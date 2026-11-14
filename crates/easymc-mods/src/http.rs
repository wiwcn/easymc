use std::time::Duration;

use easymc_core::{EasyMcError, Result};

/// Connect timeout for marketplace APIs. A black-holed DNS/TCP handshake must
/// fail in seconds, not hang the whole agent turn.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Whole-request timeout for JSON endpoints (search / project / version).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub const USER_AGENT: &str = concat!(
    "EasyMC/",
    env!("CARGO_PKG_VERSION"),
    " (github.com/easymc/easymc; launcher)"
);

pub fn modrinth_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| EasyMcError::msg(e.to_string()))
}

pub fn curseforge_client(api_key: &str) -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::HeaderName::from_static("x-api-key"),
        reqwest::header::HeaderValue::from_str(api_key)
            .map_err(|e| EasyMcError::msg(format!("无效的 CurseForge API Key: {e}")))?,
    );
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| EasyMcError::msg(e.to_string()))
}
