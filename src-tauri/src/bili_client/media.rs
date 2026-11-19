use super::BiliClient;
use crate::{extensions::AppHandleExt, protobuf::DmSegMobileReply, types::subtitle::Subtitle};
use bytes::Bytes;
use eyre::{OptionExt, WrapErr, eyre};
use prost::Message;
use reqwest::StatusCode;
use serde_json::json;
use tauri::http::HeaderMap;
use tokio::task::JoinSet;
use tracing::{Instrument, instrument};

impl BiliClient {
    #[instrument(level = "error", skip_all)]
    pub async fn get_media_chunk(
        &self,
        media_url: &str,
        start: u64,
        end: u64,
    ) -> eyre::Result<Bytes> {
        let request = self
            .media_client
            .read()
            .get(media_url)
            .header("range", format!("bytes={start}-{end}"));
        let http_resp = request.send().await?;
        // 检查http响应状态码
        let status = http_resp.status();
        if status != StatusCode::PARTIAL_CONTENT {
            return Err(eyre!("预料之外的状态码({status})"));
        }

        let content_range = http_resp
            .headers()
            .get("Content-Range")
            .ok_or_eyre("缺少 Content-Range 响应头")?
            .to_str()?;
        let expected_length =
            crate::downloader::http_range::validate_content_range(start, end, content_range)?;
        let bytes = http_resp.bytes().await?;
        crate::downloader::http_range::validate_chunk_length(expected_length, bytes.len())?;

        Ok(bytes)
    }

    #[instrument(level = "error", skip_all, fields(media_url = media_url))]
    pub async fn get_content_length(&self, media_url: &str) -> eyre::Result<u64> {
        #[instrument(level = "error", skip_all)]
        fn parse_content_length(headers: &HeaderMap) -> eyre::Result<u64> {
            headers
                .get("Content-Length")
                .ok_or_eyre("缺少 Content-Length 响应头")?
                .to_str()
                .wrap_err("Content-Length 响应头无法转换为字符串")?
                .parse::<u64>()
                .wrap_err("Content-Length 响应头无法转换为整数")
        }

        #[instrument(level = "error", skip_all)]
        fn parse_total_from_content_range(headers: &HeaderMap) -> eyre::Result<u64> {
            // Example: "bytes 0-0/12345"
            let content_range = headers
                .get("Content-Range")
                .ok_or_eyre("缺少 Content-Range 响应头")?
                .to_str()
                .wrap_err("Content-Range 响应头无法转换为字符串")?;

            let Some((_, total)) = content_range.split_once('/') else {
                return Err(eyre!("预料之外的 Content-Range 格式: {content_range}"));
            };

            total
                .parse::<u64>()
                .wrap_err("Content-Range 总大小无法转换为整数")
        }

        // 优先使用 HEAD 获取 Content-Length
        let request = self.content_length_client.read().head(media_url);
        let http_resp = request.send().await?;
        let status = http_resp.status();
        if status == StatusCode::OK
            && let Ok(content_length) = parse_content_length(http_resp.headers())
        {
            return Ok(content_length);
        }

        // 部分 upos/CDN 对 HEAD 支持不完整（尤其是高码率/高帧率流），这里降级用 Range 请求探测总大小
        let request = self
            .content_length_client
            .read()
            .get(media_url)
            .header("range", "bytes=0-0");
        let http_resp = request.send().await?;
        let status = http_resp.status();

        if status == StatusCode::PARTIAL_CONTENT {
            return parse_total_from_content_range(http_resp.headers());
        }

        if status == StatusCode::OK {
            return parse_content_length(http_resp.headers());
        }

        Err(eyre!("预料之外的状态码({status})"))
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_url_with_content_length(&self, urls: Vec<String>) -> Vec<(String, u64)> {
        let mut url_with_content_length = Vec::new();
        let mut join_set = JoinSet::new();

        for url in urls {
            let app = self.app.clone();
            let get_content_length_task = async move {
                let bili_client = app.get_bili_client();
                let Ok(content_length) = bili_client.get_content_length(&url).await else {
                    return None;
                };
                Some((url, content_length))
            };
            join_set.spawn(get_content_length_task.in_current_span());
        }

        while let Some(join_result) = join_set.join_next().await {
            let Ok(Some((url, content_length))) = join_result else {
                continue;
            };

            url_with_content_length.push((url, content_length));
        }

        url_with_content_length
    }

    #[instrument(level = "error", skip_all, fields(aid = aid, cid = cid, duration = duration))]
    pub async fn get_danmaku(
        &self,
        aid: i64,
        cid: i64,
        duration: u64,
    ) -> eyre::Result<Vec<DmSegMobileReply>> {
        let client = self.api_client.read().clone();
        // 以6分钟为单位分段
        let segment_count = duration.div_ceil(360);

        let mut join_set = JoinSet::new();
        let mut segments = 1..=segment_count;
        let concurrency = self.app.get_config().read().chunk_concurrency.max(1);
        let mut replies = Vec::new();
        loop {
            while join_set.len() < concurrency {
                let Some(segment_index) = segments.next() else {
                    break;
                };
                let client = client.clone();
                let cookie = self.get_cookie();

                let segment_span =
                    tracing::error_span!("get_danmaku_segment", segment_index = segment_index);
                let segment_task = async move {
                    // 发送获取分段弹幕的请求
                    let params = json!({
                        "type": 1,
                        "oid": cid,
                        "pid": aid,
                        "segment_index": segment_index,
                    });
                    let http_resp = client
                        .get("https://api.bilibili.com/x/v2/dm/web/seg.so")
                        .query(&params)
                        .header("cookie", cookie)
                        .send()
                        .await?;
                    let status = http_resp.status();
                    if status != StatusCode::OK {
                        let body = http_resp.text().await?;
                        return Err(eyre!("预料之外的状态码({status}): {body}"));
                    }
                    let body = http_resp.bytes().await?;
                    let reply = DmSegMobileReply::decode(body)
                        .wrap_err("将body解析为DmSegMobileReply失败")?;

                    Ok(reply)
                };
                join_set.spawn(segment_task.instrument(segment_span));
            }

            match join_set.join_next().await {
                Some(result) => replies.push(result.wrap_err("弹幕分段工作异常退出")??),
                None => return Ok(replies),
            }
        }
    }

    #[instrument(level = "error", skip_all, fields(url = url))]
    pub async fn get_subtitle(&self, url: &str) -> eyre::Result<Subtitle> {
        let request = self.api_request(url);
        let http_resp = request.send().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        // 尝试将body解析为Subtitle
        let subtitle: Subtitle = serde_json::from_str(&body)
            .wrap_err_with(|| format!("将body解析为Subtitle失败: {body}"))?;

        Ok(subtitle)
    }

    #[instrument(level = "error", skip_all, fields(url = url))]
    pub async fn get_cover_data_and_ext(&self, url: &str) -> eyre::Result<(Bytes, String)> {
        let request = self.api_request(url);
        let http_resp = request.send().await?;
        // 检查http响应状态码
        let status = http_resp.status();
        if status != StatusCode::OK {
            let body = http_resp.text().await?;
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }

        let content_type = http_resp
            .headers()
            .get("Content-Type")
            .ok_or_eyre("缺少 Content-Type 响应头")?
            .to_str()
            .wrap_err("Content-Type 响应头无法转换为字符串")?
            .to_string();

        let ext = match content_type.as_str() {
            "image/png" => "png",
            "image/webp" => "webp",
            "image/avif" => "avif",
            _ => "jpg",
        };

        let bytes = http_resp.bytes().await?;

        Ok((bytes, ext.to_string()))
    }
}
