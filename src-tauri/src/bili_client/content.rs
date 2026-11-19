use super::{BiliClient, read_api_response};
use crate::types::{
    bangumi_follow_info::BangumiFollowInfo, bangumi_info::BangumiInfo,
    bangumi_media_url::BangumiMediaUrl, bangumi_media_url_v2::BangumiMediaUrlV2,
    cheese_info::CheeseInfo, cheese_media_url::CheeseMediaUrl, fav_folders::FavFolders,
    fav_info::FavInfo, get_bangumi_follow_info_params::GetBangumiFollowInfoParams,
    get_bangumi_info_params::GetBangumiInfoParams, get_cheese_info_params::GetCheeseInfoParams,
    get_fav_info_params::GetFavInfoParams, get_history_info_params::GetHistoryInfoParams,
    get_normal_info_params::GetNormalInfoParams,
    get_user_video_info_params::GetUserVideoInfoParams, history_info::HistoryInfo,
    normal_info::NormalInfo, normal_media_url::NormalMediaUrl, player_info::PlayerInfo,
    skip_segments::SkipSegments, tags::Tags, user_video_info::UserVideoInfo,
    watch_later_info::WatchLaterInfo,
};
use base64::{Engine, engine::general_purpose};
use eyre::{WrapErr, eyre};
use reqwest::StatusCode;
use serde_json::json;
use tracing::instrument;

impl BiliClient {
    #[instrument(level = "error", skip_all, fields(bvid = params.get_bvid(), aid = params.get_aid()))]
    pub async fn get_normal_info(&self, params: GetNormalInfoParams) -> eyre::Result<NormalInfo> {
        use GetNormalInfoParams::{Aid, Bvid};
        let params = match params {
            Bvid(bvid) => json!({"bvid": bvid}),
            Aid(aid) => json!({"aid": aid}),
        };
        // 发送获取普通视频信息的请求
        let request = self
            .api_request("https://api.bilibili.com/x/web-interface/view")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all, fields(ep_id = params.get_ep_id(), season_id = params.get_season_id()))]
    pub async fn get_bangumi_info(
        &self,
        params: GetBangumiInfoParams,
    ) -> eyre::Result<BangumiInfo> {
        use GetBangumiInfoParams::{EpId, SeasonId};
        let params = match params {
            EpId(ep_id) => json!({"ep_id": ep_id}),
            SeasonId(season_id) => json!({"season_id": season_id}),
        };
        // 发送获取番剧视频信息的请求
        let request = self
            .api_request("https://api.bilibili.com/pgc/view/web/season")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all, fields(ep_id = params.get_ep_id(), season_id = params.get_season_id()))]
    pub async fn get_cheese_info(&self, params: GetCheeseInfoParams) -> eyre::Result<CheeseInfo> {
        use GetCheeseInfoParams::{EpId, SeasonId};
        let params = match params {
            EpId(ep_id) => json!({"ep_id": ep_id}),
            SeasonId(season_id) => json!({"season_id": season_id}),
        };
        // 发送获取课程视频信息的请求
        let request = self
            .api_request("https://api.bilibili.com/pugv/view/web/season")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_user_video_info(
        &self,
        params: GetUserVideoInfoParams,
    ) -> eyre::Result<UserVideoInfo> {
        const DM_IMG_INTER: &str = r#"{"ds":[],"wh":[0,0,0],"of":[0,0,0]}"#;

        fn random_base64() -> String {
            let random_bytes: Vec<u8> = (0..48).map(|_| rand::random_range(32..=127)).collect();

            general_purpose::STANDARD.encode(&random_bytes)
        }

        let mut dm_img_str = random_base64();
        dm_img_str.truncate(dm_img_str.len() - 2);

        let mut dm_cover_img_str = random_base64();
        dm_cover_img_str.truncate(dm_cover_img_str.len() - 2);

        let mut params: Vec<(&str, String)> = vec![
            ("pn", params.pn.to_string()),
            ("ps", "42".to_string()),
            ("mid", params.mid.to_string()),
            ("dm_img_list", "[]".to_string()),
            ("dm_img_str", dm_img_str),
            ("dm_cover_img_str", dm_cover_img_str),
            ("dm_img_inter", DM_IMG_INTER.to_string()),
        ];
        self.wbi(&mut params).await?;

        let request = self
            .api_request("https://api.bilibili.com/x/space/wbi/arc/search")
            .query(&params)
            // 不带DedeUserID会随机触发412错误
            .header("cookie", format!("DedeUserID=1; {}", self.get_cookie()));
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all, fields(bvid = bvid, cid = cid))]
    pub async fn get_normal_url(&self, bvid: &str, cid: i64) -> eyre::Result<NormalMediaUrl> {
        let params = json!({
            "bvid": bvid,
            "cid": cid,
            "qn": 127,
            "fnval": 4048,
        });
        // 发送获取普通url的请求
        let request = self
            .api_request("https://api.bilibili.com/x/player/wbi/playurl")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all, fields(cid = cid))]
    pub async fn get_bangumi_url(&self, cid: i64) -> eyre::Result<BangumiMediaUrl> {
        let media_url_v2 = self.get_bangumi_url_v2(cid).await?;
        if media_url_v2.video_info.is_drm {
            self.get_bangumi_url_v1(cid).await
        } else {
            Ok(media_url_v2.video_info)
        }
    }

    #[instrument(level = "error", skip_all, fields(cid = cid))]
    async fn get_bangumi_url_v1(&self, cid: i64) -> eyre::Result<BangumiMediaUrl> {
        let params = json!({
            "cid": cid,
            "qn": 127,
            "fnval": 139216,
            "drm_tech_type": 2,
        });
        // 发送获取番剧url的请求
        let request = self
            .api_request("https://api.bilibili.com/pgc/player/web/playurl")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all, fields(cid = cid))]
    async fn get_bangumi_url_v2(&self, cid: i64) -> eyre::Result<BangumiMediaUrlV2> {
        let params = json!({
            "cid": cid,
            "qn": 127,
            "fnval": 139216,
            "drm_tech_type": 2,
            "from_client": "BROWSER",
        });
        // 发送获取番剧url的请求
        let request = self
            .api_request("https://api.bilibili.com/pgc/player/web/v2/playurl")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all, fields(ep_id = ep_id))]
    pub async fn get_cheese_url(&self, ep_id: i64) -> eyre::Result<CheeseMediaUrl> {
        let params = json!({
            "ep_id": ep_id,
            "qn": 127,
            "fnval": 139216,
            "drm_tech_type": 2,
        });
        // 发送获取课程url的请求
        let request = self
            .api_request("https://api.bilibili.com/pugv/player/web/playurl")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all, fields(aid = aid, cid = cid))]
    pub async fn get_player_info(&self, aid: i64, cid: i64) -> eyre::Result<PlayerInfo> {
        let params = json!({
            "aid": aid,
            "cid": cid,
        });
        // 发送获取播放器信息的请求
        let request = self
            .api_request("https://api.bilibili.com/x/player/wbi/v2")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_fav_folders(&self, uid: i64) -> eyre::Result<FavFolders> {
        let params = json!({"up_mid": uid});
        // 发送获取收藏夹信息的请求
        let request = self
            .api_request("https://api.bilibili.com/x/v3/fav/folder/created/list-all")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_fav_info(&self, params: GetFavInfoParams) -> eyre::Result<FavInfo> {
        let params = json!({
            "media_id": params.media_list_id,
            "pn": params.pn,
            "ps": 36,
            "platform": "web",
        });
        // 发送获取收藏夹信息的请求
        let request = self
            .api_request("https://api.bilibili.com/x/v3/fav/resource/list")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_watch_later_info(&self, page: i32) -> eyre::Result<WatchLaterInfo> {
        // 发送获取稍后观看信息的请求
        let params = json!({"ps": 20, "pn": page});
        let request = self
            .api_request("https://api.bilibili.com/x/v2/history/toview")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_bangumi_follow_info(
        &self,
        params: GetBangumiFollowInfoParams,
    ) -> eyre::Result<BangumiFollowInfo> {
        // 发送获取番剧追踪信息的请求
        let params = json!({
            "vmid": params.vmid,
            "type": params.type_field,
            "pn": params.pn,
            "ps": 24,
            "follow_status": params.follow_status,
        });
        let request = self
            .api_request("https://api.bilibili.com/x/space/bangumi/follow/list")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_history_info(
        &self,
        params: GetHistoryInfoParams,
    ) -> eyre::Result<HistoryInfo> {
        let device_type: i64 = params.device_type.into();
        let params = json!({
            "pn": params.pn,
            "keyword": params.keyword,
            "business": "archive",
            "add_time_start": params.add_time_start,
            "add_time_end": params.add_time_end,
            "arc_max_duration": params.arc_max_duration,
            "arc_min_duration": params.arc_min_duration,
            "device_type": device_type,
        });
        let request = self
            .api_request("https://api.bilibili.com/x/web-interface/history/search")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all, fields(aid = aid))]
    pub async fn get_tags(&self, aid: i64) -> eyre::Result<Tags> {
        // 发送获取普通视频标签的请求
        let params = json!({"aid": aid});
        let request = self
            .api_request("https://api.bilibili.com/x/web-interface/view/detail/tag")
            .query(&params)
            .header("cookie", self.get_cookie());
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all, fields(bvid = bvid, cid = cid))]
    pub async fn get_skip_segments(
        &self,
        bvid: &str,
        cid: Option<i64>,
    ) -> eyre::Result<SkipSegments> {
        // 发送获取跳过片段的请求
        let mut params = json!({
            "videoID": bvid,
            "actionType": "skip",
        });
        if let Some(cid) = cid {
            params["cid"] = cid.into();
        }

        let request = self
            .api_request("https://bsbsb.top/api/skipSegments")
            .query(&params);
        let http_resp = request.send().await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == StatusCode::NOT_FOUND {
            return Ok(SkipSegments(Vec::new()));
        } else if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        // 尝试将body解析为SkipSegments
        let skip_segments: SkipSegments = serde_json::from_str(&body)
            .wrap_err(format!("将body解析为SkipSegments失败: {body}"))?;

        Ok(skip_segments)
    }
}
