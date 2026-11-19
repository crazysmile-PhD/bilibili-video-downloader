use super::{BiliClient, read_api_response};
use crate::{
    extensions::AppHandleExt,
    types::{qrcode_data::QrcodeData, qrcode_status::QrcodeStatus, user_info::UserInfo},
};
use eyre::{OptionExt, eyre};
use tracing::instrument;

impl BiliClient {
    #[instrument(level = "error", skip_all)]
    pub async fn generate_qrcode(&self) -> eyre::Result<QrcodeData> {
        // 发送生成二维码请求
        let request =
            self.api_request("https://passport.bilibili.com/x/passport-login/web/qrcode/generate");
        read_api_response(request.send().await?).await
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_qrcode_status(&self, qrcode_key: &str) -> eyre::Result<QrcodeStatus> {
        // 发送获取二维码状态请求
        let params = serde_json::json!({"qrcode_key": qrcode_key});
        let request = self
            .api_request("https://passport.bilibili.com/x/passport-login/web/qrcode/poll")
            .query(&params);
        let http_resp = request.send().await?;
        let sessdata = http_resp
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|header| header.to_str().ok())
            .filter_map(|header| header.split(';').next())
            .filter_map(|cookie| cookie.split_once('='))
            .find_map(|(key, value)| (key.trim() == "SESSDATA").then(|| value.to_owned()));
        finish_qrcode_status(read_api_response(http_resp).await?, sessdata)
    }

    #[instrument(level = "error", skip_all)]
    pub async fn get_user_info(&self, sessdata: &str) -> eyre::Result<UserInfo> {
        // 发送获取用户信息的请求
        let request = self
            .api_request("https://api.bilibili.com/x/web-interface/nav")
            .header("cookie", format!("SESSDATA={sessdata}"));
        read_api_response(request.send().await?).await
    }

    pub fn get_cookie(&self) -> String {
        let sessdata = self.app.get_config().read().sessdata.clone();

        let trimmed = sessdata.trim_end_matches(';');
        let decoded = urlencoding::decode(trimmed).unwrap_or_else(|_| trimmed.into());
        let encoded = urlencoding::encode(&decoded);

        format!("SESSDATA={encoded}")
    }
}

fn finish_qrcode_status(
    mut status: QrcodeStatus,
    sessdata: Option<String>,
) -> eyre::Result<QrcodeStatus> {
    if ![0, 86101, 86090, 86038].contains(&status.code) {
        return Err(eyre!("预料之外的二维码code: {status:?}"));
    }
    if status.code == 0 {
        status.sessdata = sessdata.ok_or_eyre("扫码成功，但没有解析到SESSDATA")?;
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_session_cookie_only_after_successful_scan() {
        for code in [86101, 86090, 86038] {
            assert!(
                finish_qrcode_status(
                    QrcodeStatus {
                        code,
                        ..Default::default()
                    },
                    None
                )
                .is_ok()
            );
        }
        assert!(finish_qrcode_status(QrcodeStatus::default(), None).is_err());
        assert_eq!(
            finish_qrcode_status(QrcodeStatus::default(), Some("token".into()))
                .unwrap()
                .sessdata,
            "token"
        );
        assert!(
            finish_qrcode_status(
                QrcodeStatus {
                    code: -1,
                    ..Default::default()
                },
                Some("token".into())
            )
            .is_err()
        );
    }
}
