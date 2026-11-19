//! Bilibili's response envelope and error policy, independent of the HTTP client.

use eyre::{WrapErr, eyre};
use serde::{Deserialize, de::DeserializeOwned};

#[derive(Deserialize)]
struct ApiResponse {
    code: i64,
    #[serde(default, alias = "message")]
    msg: String,
    #[serde(alias = "result")]
    data: Option<serde_json::Value>,
}

pub fn decode_api_response<T: DeserializeOwned>(status: u16, body: &str) -> eyre::Result<T> {
    if status != 200 {
        return Err(eyre!("Bilibili HTTP 状态码异常: {status}"));
    }
    let response: ApiResponse = serde_json::from_str(body).wrap_err("解析 Bilibili 响应失败")?;
    match response.code {
        0 => {}
        -101 => return Err(eyre!("cookie错误或已过期，请重新登录")),
        -10403 => return Err(eyre!("地区限制，请使用代理或切换线路后重试")),
        -403 => return Err(eyre!("没有观看权限，请先购买")),
        code => {
            return Err(eyre!(
                "Bilibili API 错误: code={code}, message={}",
                response.msg
            ));
        }
    }
    let data = response
        .data
        .ok_or_else(|| eyre!("Bilibili 响应缺少 data/result"))?;
    // Consume the Value directly. Serializing it to a String and parsing again
    // duplicates traversal, allocation and JSON parsing for every API call.
    serde_json::from_value(data)
        .wrap_err_with(|| format!("解析 Bilibili 数据为 {} 失败", std::any::type_name::<T>()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    #[allow(
        dead_code,
        reason = "This test-only payload is also included by the harness-free benchmark's shared decoder module."
    )]
    struct Payload {
        title: String,
    }

    #[test]
    fn supports_data_and_result_envelopes_without_changing_payloads() {
        for field in ["data", "result"] {
            let body = format!(r#"{{"code":0,"{field}":{{"title":"标题 & 👋"}}}}"#);
            assert_eq!(
                decode_api_response::<Payload>(200, &body).unwrap(),
                Payload {
                    title: "标题 & 👋".into()
                }
            );
        }
    }

    #[test]
    fn rejects_http_errors_missing_data_and_invalid_payloads() {
        assert!(
            decode_api_response::<Payload>(503, "not json")
                .unwrap_err()
                .to_string()
                .contains("503")
        );
        for body in [
            r#"{"code":0}"#,
            r#"{"code":0,"data":null}"#,
            r#"{"code":0,"data":{"title":4}}"#,
            "{truncated",
        ] {
            assert!(decode_api_response::<Payload>(200, body).is_err(), "{body}");
        }
    }

    #[test]
    fn checks_api_errors_before_decoding_error_payloads() {
        for (code, message) in [(-101, "重新登录"), (-10403, "地区限制"), (-403, "购买")]
        {
            let body = format!(r#"{{"code":{code},"data":{{}}}}"#);
            assert!(
                decode_api_response::<Payload>(200, &body)
                    .unwrap_err()
                    .to_string()
                    .contains(message)
            );
        }
        let error =
            decode_api_response::<Payload>(200, r#"{"code":-1,"message":"请求受限"}"#).unwrap_err();
        assert!(error.to_string().contains("请求受限"));
    }
}
