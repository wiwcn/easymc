//! Upload Minecraft logs to [mclo.gs](https://mclo.gs) for shareable crash/debug links.
//! API: `POST https://api.mclo.gs/1/log` with `application/x-www-form-urlencoded` body `content=…`.

use serde::{Deserialize, Serialize};

const MCLOGS_ENDPOINT: &str = "https://api.mclo.gs/1/log";
/// Soft cap so oversized latest.log does not blow the request (mclo.gs ~10 MiB).
const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MclogsUploadResult {
    pub id: String,
    pub url: String,
    pub raw: Option<String>,
    pub message_zh: String,
}

#[derive(Debug, Deserialize)]
struct MclogsApiResponse {
    success: bool,
    id: Option<String>,
    url: Option<String>,
    raw: Option<String>,
    error: Option<String>,
}

/// Trim + optionally truncate log text for upload.
pub fn prepare_log_content(raw: &str) -> String {
    let t = raw.trim();
    if t.len() <= MAX_UPLOAD_BYTES {
        return t.to_string();
    }
    let keep = MAX_UPLOAD_BYTES.saturating_sub(80);
    let start = t.len().saturating_sub(keep);
    let mut slice = &t[start..];
    while !slice.is_char_boundary(0) && !slice.is_empty() {
        slice = &slice[1..];
    }
    format!("…（日志过长，已截取末尾）\n{slice}")
}

fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Upload prepared log body; returns share URL.
pub async fn upload_log_content(content: &str) -> Result<MclogsUploadResult, String> {
    let prepared = prepare_log_content(content);
    if prepared.trim().is_empty() {
        return Err("日志是空的，没法上传".into());
    }
    let body = format!("content={}", form_encode(&prepared));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;
    let res = client
        .post(MCLOGS_ENDPOINT)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("User-Agent", concat!("EasyMC/", env!("CARGO_PKG_VERSION")))
        .body(body)
        .send()
        .await
        .map_err(|e| format!("上传到 mclo.gs 失败: {e}"))?;
    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|e| format!("读 mclo.gs 响应失败: {e}"))?;
    if !status.is_success() {
        return Err(format!("mclo.gs 返回 {status}: {text}"));
    }
    let parsed: MclogsApiResponse =
        serde_json::from_str(&text).map_err(|e| format!("解析 mclo.gs 响应失败: {e} / {text}"))?;
    if !parsed.success {
        return Err(parsed
            .error
            .unwrap_or_else(|| "mclo.gs 拒绝了这份日志".into()));
    }
    let id = parsed.id.unwrap_or_default();
    let url = parsed
        .url
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| format!("https://mclo.gs/{id}"));
    if id.is_empty() && !url.contains("mclo.gs") {
        return Err("mclo.gs 没有返回链接".into());
    }
    Ok(MclogsUploadResult {
        id,
        url: url.clone(),
        raw: parsed.raw,
        message_zh: format!("日志已上传：{url}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_keeps_short() {
        assert_eq!(prepare_log_content("  hello\n"), "hello");
    }

    #[test]
    fn prepare_truncates_long() {
        let big = "x".repeat(MAX_UPLOAD_BYTES + 1000);
        let out = prepare_log_content(&big);
        assert!(out.len() <= MAX_UPLOAD_BYTES + 40);
        assert!(out.contains("截取末尾"));
    }

    #[test]
    fn form_encode_space_and_cjk() {
        assert_eq!(form_encode("a b"), "a+b");
        assert!(form_encode("中").contains('%'));
    }
}
