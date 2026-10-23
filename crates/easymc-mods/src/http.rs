use easymc_core::{EasyMcError, Result};

pub const USER_AGENT: &str = concat!(
    "EasyMC/",
    env!("CARGO_PKG_VERSION"),
    " (github.com/easymc/easymc; launcher)"
);

pub fn modrinth_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
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
        .build()
        .map_err(|e| EasyMcError::msg(e.to_string()))
}
