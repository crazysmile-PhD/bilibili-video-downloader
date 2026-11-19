mod auth;
mod content;
mod media;
mod response;

use std::time::Duration;

use crate::{
    config::ProxyMode,
    extensions::{AppHandleExt, EyreReportToMessage},
};
use parking_lot::RwLock;
use reqwest::Client;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{Jitter, RetryTransientMiddleware, policies::ExponentialBackoff};
use serde::de::DeserializeOwned;
use tauri::{
    AppHandle,
    http::{HeaderMap, HeaderValue},
};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";
const REFERRER: &str = "https://www.bilibili.com/";

pub struct BiliClient {
    app: AppHandle,
    api_client: RwLock<ClientWithMiddleware>,
    media_client: RwLock<ClientWithMiddleware>,
    content_length_client: RwLock<Client>,
}

impl BiliClient {
    pub fn new(app: AppHandle) -> eyre::Result<Self> {
        let api_client = create_api_client(&app)?;
        let api_client = RwLock::new(api_client);

        let media_client = create_media_client(&app)?;
        let media_client = RwLock::new(media_client);

        let content_length_client = create_content_length_client(&app)?;
        let content_length_client = RwLock::new(content_length_client);

        Ok(Self {
            app,
            api_client,
            media_client,
            content_length_client,
        })
    }

    pub fn reload_client(&self) -> eyre::Result<()> {
        // Build all replacements before changing any live client.
        let api_client = create_api_client(&self.app)?;
        let media_client = create_media_client(&self.app)?;
        let content_length_client = create_content_length_client(&self.app)?;
        *self.api_client.write() = api_client;
        *self.media_client.write() = media_client;
        *self.content_length_client.write() = content_length_client;
        Ok(())
    }

    pub(crate) fn api_request(&self, url: &str) -> reqwest_middleware::RequestBuilder {
        self.api_client.read().get(url)
    }
}

fn create_api_client(app: &AppHandle) -> eyre::Result<ClientWithMiddleware> {
    let retry_policy = ExponentialBackoff::builder()
        .base(1)
        .jitter(Jitter::Bounded)
        .build_with_total_retry_duration(Duration::from_secs(5));

    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static(USER_AGENT));
    headers.insert("referer", HeaderValue::from_static(REFERRER));

    let client = reqwest::ClientBuilder::new()
        .set_proxy(app, "api_client")
        .timeout(Duration::from_secs(3))
        .default_headers(headers)
        .build()?;

    Ok(reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build())
}

fn create_media_client(app: &AppHandle) -> eyre::Result<ClientWithMiddleware> {
    let retry_policy = ExponentialBackoff::builder()
        .base(1)
        .jitter(Jitter::Bounded)
        .build_with_max_retries(3);

    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static(USER_AGENT));
    headers.insert("referer", HeaderValue::from_static(REFERRER));

    let client = reqwest::ClientBuilder::new()
        .set_proxy(app, "media_client")
        .default_headers(headers)
        .build()?;

    Ok(reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build())
}

fn create_content_length_client(app: &AppHandle) -> eyre::Result<Client> {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static(USER_AGENT));
    headers.insert("referer", HeaderValue::from_static(REFERRER));

    Ok(reqwest::ClientBuilder::new()
        .set_proxy(app, "content_length_client")
        .timeout(Duration::from_secs(5))
        .default_headers(headers)
        .build()?)
}

trait ClientBuilderExt {
    fn set_proxy(self, app: &AppHandle, client_name: &str) -> Self;
}

impl ClientBuilderExt for reqwest::ClientBuilder {
    fn set_proxy(self, app: &AppHandle, client_name: &str) -> reqwest::ClientBuilder {
        let proxy_mode = app.get_config().read().proxy_mode;
        match proxy_mode {
            ProxyMode::NoProxy => self.no_proxy(),
            ProxyMode::System => self,
            ProxyMode::Custom => {
                let config = app.get_config().inner().read();
                let proxy_host = &config.proxy_host;
                let proxy_port = &config.proxy_port;
                let proxy_url = format!("http://{proxy_host}:{proxy_port}");

                match reqwest::Proxy::all(&proxy_url).map_err(eyre::Report::from) {
                    Ok(proxy) => self.proxy(proxy),
                    Err(err) => {
                        let err_title = format!("{client_name}将`{proxy_url}`设为代理失败，将直连");
                        let message = err.to_message();
                        tracing::error!(err_title, message);
                        self.no_proxy()
                    }
                }
            }
        }
    }
}

pub(crate) async fn read_api_response<T: DeserializeOwned>(
    response: reqwest::Response,
) -> eyre::Result<T> {
    let status = response.status().as_u16();
    let body = response.text().await?;
    response::decode_api_response(status, &body)
}
