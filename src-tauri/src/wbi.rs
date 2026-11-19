use std::time::{SystemTime, UNIX_EPOCH};

use eyre::{OptionExt, WrapErr};
use md5::{Digest, Md5};
use serde::Deserialize;
use tracing::instrument;

use crate::bili_client::{BiliClient, read_api_response};

const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

#[derive(Deserialize)]
struct WbiImgRespData {
    img_url: String,
    sub_url: String,
}

#[derive(Deserialize)]
struct WeiRespData {
    wbi_img: WbiImgRespData,
}

impl BiliClient {
    // 为请求参数进行 wbi 签名
    #[instrument(level = "error", skip_all)]
    pub async fn wbi(&self, params: &mut Vec<(&str, String)>) -> eyre::Result<()> {
        let (img_key, sub_key) = self.get_wbi_keys().await.wrap_err("获取wbi keys失败")?;
        let mixin_key = get_mixin_key((img_key + &sub_key).as_bytes());

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        // 添加当前时间戳
        params.push(("wts", timestamp.to_string()));
        // 重新排序
        params.sort_by(|a, b| a.0.cmp(b.0));
        // 拼接参数
        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", get_url_encoded(k), get_url_encoded(v)))
            .collect::<Vec<_>>()
            .join("&");
        // 计算签名
        let web_sign = md5_hex(query + &mixin_key);
        params.push(("w_rid", web_sign));
        Ok(())
    }

    #[instrument(level = "error", skip_all)]
    async fn get_wbi_keys(&self) -> eyre::Result<(String, String)> {
        let request = self
            .api_request("https://api.bilibili.com/x/web-interface/nav")
            .header("Cookie", &self.get_cookie());

        let wei_resp_data: WeiRespData = read_api_response(request.send().await?).await?;

        let img_url = wei_resp_data.wbi_img.img_url;
        let sub_url = wei_resp_data.wbi_img.sub_url;

        let img_filename =
            take_filename(&img_url).ok_or_eyre(format!("从img_url中提取文件名失败: {img_url}"))?;
        let sub_filename =
            take_filename(&sub_url).ok_or_eyre(format!("从sub_url中提取文件名失败: {sub_url}"))?;

        Ok((img_filename, sub_filename))
    }
}

// 对 imgKey 和 subKey 进行字符顺序打乱编码
fn get_mixin_key(orig: &[u8]) -> String {
    MIXIN_KEY_ENC_TAB
        .iter()
        .take(32)
        .map(|&i| orig[i] as char)
        .collect::<String>()
}

fn get_url_encoded(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() || "-_.~".contains(c) {
                Some(c.to_string())
            } else {
                // 过滤 value 中的 "!'()*" 字符
                if "!'()*".contains(c) {
                    return None;
                }
                let encoded = c
                    .encode_utf8(&mut [0; 4])
                    .bytes()
                    .fold(String::new(), |acc, b| acc + &format!("%{b:02X}"));
                Some(encoded)
            }
        })
        .collect::<String>()
}

fn take_filename(url: &str) -> Option<String> {
    url.rsplit_once('/')
        .and_then(|(_, s)| s.rsplit_once('.'))
        .map(|(s, _)| s.to_string())
}

fn md5_hex(value: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(32);
    for byte in Md5::digest(value) {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::md5_hex;

    #[test]
    fn signature_digest_preserves_lowercase_hex_and_leading_zeroes() {
        for (input, expected) in [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
        ] {
            assert_eq!(md5_hex(input), expected);
        }
    }
}
