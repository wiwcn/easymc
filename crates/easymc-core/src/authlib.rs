//! Authlib-Injector / Yggdrasil skin-station login (no password persistence).
//!
//! Downloads the AGPL authlib-injector jar at runtime — do not vendor its source.

use crate::{Account, AccountKind, EasyMcError, Paths, Result};
use chrono::{Duration as ChronoDuration, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

/// Pinned authlib-injector release (runtime download only).
pub const AUTHLIB_INJECTOR_VERSION: &str = "1.2.8";
const AUTHLIB_INJECTOR_JAR_NAME: &str = "authlib-injector.jar";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("EasyMC/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
        .unwrap_or_default()
}

fn http(e: reqwest::Error) -> EasyMcError {
    EasyMcError::msg(format!("网络连接失败，请检查网络后重试（{e}）"))
}

fn trim_slash(s: &str) -> &str {
    s.trim().trim_end_matches('/')
}

/// Resolved skin-station API endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct AuthlibServerInfo {
    pub api_root: String,
    pub server_name: Option<String>,
}

fn looks_like_yggdrasil_meta(v: &Value) -> bool {
    v.get("meta").is_some()
        || v.get("signaturePublickey").is_some()
        || v.get("skinDomains").is_some()
}

fn server_name_from_meta(v: &Value) -> Option<String> {
    v.pointer("/meta/serverName")
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

async fn fetch_meta(c: &reqwest::Client, url: &str) -> Result<(reqwest::header::HeaderMap, Value, String)> {
    let resp = c.get(url).send().await.map_err(http)?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let final_url = resp.url().to_string();
    let text = resp.text().await.map_err(http)?;
    if !status.is_success() {
        return Err(EasyMcError::msg(format!(
            "无法访问皮肤站（HTTP {}），请检查地址后重试。",
            status.as_u16()
        )));
    }
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok((headers, v, final_url))
}

/// Resolve API root from a homepage or full Yggdrasil URL.
///
/// Honors `x-authlib-injector-api-location`; else uses body if it is Yggdrasil meta;
/// else tries `{base}/api/yggdrasil`.
pub async fn resolve_api_root(server: &str) -> Result<AuthlibServerInfo> {
    let raw = server.trim();
    if raw.is_empty() {
        return Err(EasyMcError::msg("请填写皮肤站地址，例如 https://littleskin.cn"));
    }
    let mut url = raw.to_string();
    if !url.contains("://") {
        url = format!("https://{url}");
    }
    url = trim_slash(&url).to_string();

    let c = client();
    let (headers, body, final_url) = fetch_meta(&c, &url).await?;

    if let Some(loc) = headers
        .get("x-authlib-injector-api-location")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let api = if loc.starts_with("http://") || loc.starts_with("https://") {
            trim_slash(loc).to_string()
        } else {
            // Relative to the probed URL.
            let base = reqwest::Url::parse(&final_url)
                .or_else(|_| reqwest::Url::parse(&url))
                .map_err(|_| EasyMcError::msg("皮肤站地址无效，请检查后重试。"))?;
            let joined = base
                .join(loc)
                .map_err(|_| EasyMcError::msg("皮肤站返回的 API 地址无效。"))?;
            trim_slash(joined.as_str()).to_string()
        };
        let name = if looks_like_yggdrasil_meta(&body) {
            server_name_from_meta(&body)
        } else {
            None
        };
        let name = match name {
            Some(n) => Some(n),
            None => {
                // Fetch meta from resolved API for serverName.
                match fetch_meta(&c, &api).await {
                    Ok((_, meta, _)) => server_name_from_meta(&meta),
                    Err(_) => None,
                }
            }
        };
        return Ok(AuthlibServerInfo {
            api_root: api,
            server_name: name,
        });
    }

    if looks_like_yggdrasil_meta(&body) {
        return Ok(AuthlibServerInfo {
            api_root: trim_slash(&final_url).to_string(),
            server_name: server_name_from_meta(&body),
        });
    }

    // Try conventional /api/yggdrasil under the same origin.
    let base = reqwest::Url::parse(&url)
        .map_err(|_| EasyMcError::msg("皮肤站地址无效，请检查后重试。"))?;
    let candidate = base
        .join("/api/yggdrasil")
        .map_err(|_| EasyMcError::msg("皮肤站地址无效，请检查后重试。"))?;
    let cand = trim_slash(candidate.as_str()).to_string();
    if cand != trim_slash(&final_url) {
        let (_, meta, meta_url) = fetch_meta(&c, &cand).await?;
        if looks_like_yggdrasil_meta(&meta) {
            return Ok(AuthlibServerInfo {
                api_root: trim_slash(&meta_url).to_string(),
                server_name: server_name_from_meta(&meta),
            });
        }
    }

    Err(EasyMcError::msg(
        "无法识别皮肤站接口。请填写带 Authlib-Injector 支持的地址（如 https://littleskin.cn）。",
    ))
}

fn map_auth_error(status: reqwest::StatusCode, v: &Value) -> EasyMcError {
    let err = v["error"].as_str().unwrap_or("");
    let msg = v["errorMessage"].as_str().unwrap_or("");
    let lower = format!("{err} {msg}").to_lowercase();
    if status.as_u16() == 403
        || lower.contains("invalid credentials")
        || lower.contains("forbidden")
        || lower.contains("password")
    {
        return EasyMcError::msg("账号或密码不对，请再试一次。");
    }
    if lower.contains("user migrated") {
        return EasyMcError::msg("该账号已迁移，请用新的登录方式。");
    }
    if !msg.is_empty() {
        return EasyMcError::msg(format!("皮肤站登录失败：{msg}"));
    }
    if !err.is_empty() {
        return EasyMcError::msg(format!("皮肤站登录失败（{err}）。"));
    }
    EasyMcError::msg(format!(
        "皮肤站登录失败（HTTP {}），请稍后重试。",
        status.as_u16()
    ))
}

fn hyphenate_uuid(raw: &str) -> String {
    let compact: String = raw.chars().filter(|c| *c != '-').collect();
    if compact.len() == 32 && compact.chars().all(|c| c.is_ascii_hexdigit()) {
        format!(
            "{}-{}-{}-{}-{}",
            &compact[0..8],
            &compact[8..12],
            &compact[12..16],
            &compact[16..20],
            &compact[20..32]
        )
    } else {
        Uuid::parse_str(raw)
            .map(|u| u.as_hyphenated().to_string())
            .unwrap_or_else(|_| raw.to_string())
    }
}

fn account_from_auth_response(
    v: &Value,
    api_root: &str,
    server_name: Option<String>,
    fallback_client_token: Option<&str>,
    keep_created: Option<chrono::DateTime<Utc>>,
) -> Result<Account> {
    let access = v["accessToken"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| EasyMcError::msg("皮肤站没有返回有效令牌，请稍后重试。"))?
        .to_string();
    let client_token = v["clientToken"]
        .as_str()
        .map(str::to_string)
        .or_else(|| fallback_client_token.map(str::to_string))
        .filter(|s| !s.is_empty());

    let profile = if v.get("selectedProfile").is_some() && !v["selectedProfile"].is_null() {
        &v["selectedProfile"]
    } else if let Some(arr) = v["availableProfiles"].as_array() {
        arr.first()
            .ok_or_else(|| EasyMcError::msg("这个皮肤站账号还没有角色，请先在皮肤站网站创建角色。"))?
    } else {
        return Err(EasyMcError::msg(
            "这个皮肤站账号还没有角色，请先在皮肤站网站创建角色。",
        ));
    };

    let uuid_raw = profile["id"].as_str().unwrap_or_default();
    let name = profile["name"].as_str().unwrap_or_default();
    if uuid_raw.is_empty() || name.is_empty() {
        return Err(EasyMcError::msg(
            "这个皮肤站账号还没有角色，请先在皮肤站网站创建角色。",
        ));
    }
    let uuid = hyphenate_uuid(uuid_raw);
    let expires = v["expiresIn"]
        .as_i64()
        .or_else(|| v["expires_in"].as_i64())
        .unwrap_or(86400)
        .max(60);

    Ok(Account {
        id: format!("skin-{uuid}"),
        kind: AccountKind::SkinStation,
        username: name.to_string(),
        uuid,
        access_token: access,
        refresh_token: None,
        expires_at: Some(Utc::now() + ChronoDuration::seconds(expires)),
        created_at: keep_created.unwrap_or_else(Utc::now),
        authlib_api_root: Some(trim_slash(api_root).to_string()),
        authlib_server_name: server_name.filter(|s| !s.is_empty()),
        client_token,
    })
}

/// Authenticate against a Yggdrasil authserver. Does not store the password.
pub async fn authenticate(api_root: &str, username: &str, password: &str) -> Result<Account> {
    let user = username.trim();
    let pass = password; // passwords may intentionally have edge whitespace — keep as typed
    if user.is_empty() {
        return Err(EasyMcError::msg("请填写皮肤站账号（邮箱或用户名）。"));
    }
    if pass.is_empty() {
        return Err(EasyMcError::msg("请填写密码。"));
    }
    let root = trim_slash(api_root);
    if root.is_empty() {
        return Err(EasyMcError::msg("皮肤站 API 地址无效。"));
    }

    // Optional: pull serverName for storage.
    let c = client();
    let server_name = match fetch_meta(&c, root).await {
        Ok((_, meta, _)) => server_name_from_meta(&meta),
        Err(_) => None,
    };

    let client_token = Uuid::new_v4().as_hyphenated().to_string();
    let url = format!("{root}/authserver/authenticate");
    let resp = c
        .post(&url)
        .json(&json!({
            "agent": { "name": "Minecraft", "version": 1 },
            "username": user,
            "password": pass,
            "clientToken": client_token,
            "requestUser": true
        }))
        .send()
        .await
        .map_err(http)?;
    let status = resp.status();
    let v: Value = resp.json().await.map_err(http)?;
    if !status.is_success() {
        return Err(map_auth_error(status, &v));
    }
    account_from_auth_response(&v, root, server_name, Some(&client_token), None)
}

/// Refresh Yggdrasil access token via `/authserver/refresh`.
pub async fn refresh_account(acc: &Account) -> Result<Account> {
    if acc.kind != AccountKind::SkinStation {
        return Err(EasyMcError::msg("不是皮肤站账户，无法刷新。"));
    }
    let root = acc
        .authlib_api_root
        .as_deref()
        .map(trim_slash)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| EasyMcError::msg("皮肤站账户信息不完整，请重新登录。"))?;
    let access = acc.access_token.trim();
    if access.is_empty() || access == "0" {
        return Err(EasyMcError::msg("皮肤站登录已失效，请重新登录。"));
    }
    let client_token = acc
        .client_token
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| EasyMcError::msg("皮肤站登录已失效，请重新登录。"))?;

    let c = client();
    let url = format!("{root}/authserver/refresh");
    let resp = c
        .post(&url)
        .json(&json!({
            "accessToken": access,
            "clientToken": client_token,
            "requestUser": true
        }))
        .send()
        .await
        .map_err(http)?;
    let status = resp.status();
    let v: Value = resp.json().await.map_err(http)?;
    if !status.is_success() {
        return Err(map_auth_error(status, &v));
    }
    let mut out = account_from_auth_response(
        &v,
        root,
        acc.authlib_server_name.clone(),
        Some(client_token),
        Some(acc.created_at),
    )?;
    // Keep stable account id if uuid unchanged.
    if out.uuid == acc.uuid {
        out.id = acc.id.clone();
    }
    Ok(out)
}

/// True when SkinStation token is missing or expires within 5 minutes.
pub fn needs_refresh(acc: &Account) -> bool {
    acc.kind == AccountKind::SkinStation
        && acc
            .expires_at
            .map_or(true, |t| t - ChronoDuration::minutes(5) < Utc::now())
}

fn authlib_injector_jar_path(paths: &Paths) -> PathBuf {
    paths
        .libraries_dir()
        .join("authlib-injector")
        .join(AUTHLIB_INJECTOR_JAR_NAME)
}

/// Build number for AUTHLIB_INJECTOR_VERSION on authlib-injector.yushi.moe / BMCLAPI mirror.
const AUTHLIB_INJECTOR_BUILD: u32 = 56;

fn jar_download_urls() -> Vec<String> {
    let ver = AUTHLIB_INJECTOR_VERSION;
    let build = AUTHLIB_INJECTOR_BUILD;
    let asset = format!("authlib-injector-{ver}.jar");
    let gh = format!(
        "https://github.com/yushijinhun/authlib-injector/releases/download/v{ver}/{asset}"
    );
    let yushi = format!("https://authlib-injector.yushi.moe/artifact/{build}/{asset}");
    vec![
        // BMCLAPI mirror of yushi.moe (CN-friendly).
        format!(
            "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/{build}/{asset}"
        ),
        yushi,
        format!("https://mirror.ghproxy.com/{gh}"),
        format!("https://ghproxy.net/{gh}"),
        gh,
    ]
}

/// Ensure authlib-injector.jar exists under libraries/; download if missing/empty.
pub async fn ensure_authlib_injector_jar(paths: &Paths) -> Result<PathBuf> {
    let dest = authlib_injector_jar_path(paths);
    if dest.is_file() {
        if let Ok(meta) = std::fs::metadata(&dest) {
            if meta.len() > 10_000 {
                return Ok(dest);
            }
        }
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let c = client();
    let mut last_err = EasyMcError::msg("下载 authlib-injector 失败。");
    for url in jar_download_urls() {
        match c.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.bytes().await {
                    Ok(bytes) if bytes.len() > 10_000 => {
                        let tmp = dest.with_extension("jar.part");
                        if let Err(e) = std::fs::write(&tmp, &bytes) {
                            last_err = EasyMcError::msg(format!("无法写入 authlib-injector：{e}"));
                            continue;
                        }
                        if let Err(e) = std::fs::rename(&tmp, &dest) {
                            let _ = std::fs::remove_file(&dest);
                            if let Err(e2) = std::fs::rename(&tmp, &dest) {
                                last_err = EasyMcError::msg(format!("无法保存 authlib-injector：{e2}（{e}）"));
                                continue;
                            }
                        }
                        return Ok(dest);
                    }
                    Ok(_) => {
                        last_err = EasyMcError::msg("下载的 authlib-injector 文件不完整，请重试。");
                    }
                    Err(e) => last_err = http(e),
                }
            }
            Ok(resp) => {
                last_err = EasyMcError::msg(format!(
                    "下载 authlib-injector 失败（HTTP {}）。",
                    resp.status().as_u16()
                ));
            }
            Err(e) => last_err = http(e),
        }
    }
    Err(last_err)
}

/// JVM `-javaagent:jar=api_root` for SkinStation accounts.
pub fn javaagent_arg(account: &Account, jar: &std::path::Path) -> Result<String> {
    if account.kind != AccountKind::SkinStation {
        return Err(EasyMcError::msg("仅皮肤站账户需要 authlib-injector。"));
    }
    let root = account
        .authlib_api_root
        .as_deref()
        .map(trim_slash)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| EasyMcError::msg("皮肤站账户缺少 API 地址，请重新登录。"))?;
    Ok(format!("-javaagent:{}={}", jar.display(), root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;

    fn spawn_stub<F>(handler: F) -> (String, thread::JoinHandle<()>)
    where
        F: Fn(&str, &str, &str) -> (u16, &'static str, String) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        let handler = Arc::new(handler);
        let handle = thread::spawn(move || {
            // Handle a handful of requests then exit.
            for _ in 0..8 {
                let Ok((mut stream, _)) = listener.accept() else { break };
                let mut buf = [0u8; 65536];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let mut lines = req.lines();
                let start = lines.next().unwrap_or("");
                let mut parts = start.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let path = parts.next().unwrap_or("/").to_string();
                let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                let (code, ctype, resp_body) = handler(&method, &path, &body);
                let resp = format!(
                    "HTTP/1.1 {code} OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp_body}",
                    resp_body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        (format!("http://127.0.0.1:{port}"), handle)
    }

    #[tokio::test]
    async fn resolve_littleskin_live() {
        let info = resolve_api_root("https://littleskin.cn").await.expect("resolve");
        assert!(
            info.api_root.contains("yggdrasil"),
            "api_root={}",
            info.api_root
        );
        assert_eq!(info.server_name.as_deref(), Some("LittleSkin"));
    }

    #[tokio::test]
    async fn resolve_direct_yggdrasil_meta() {
        let info = resolve_api_root("https://littleskin.cn/api/yggdrasil")
            .await
            .expect("resolve");
        assert!(info.api_root.contains("yggdrasil"));
        assert_eq!(info.server_name.as_deref(), Some("LittleSkin"));
    }

    #[tokio::test]
    async fn authenticate_and_refresh_via_stub() {
        let (base, _join) = spawn_stub(|method, path, body| {
            if method == "GET" && (path == "/" || path.starts_with("/api/yggdrasil")) {
                return (
                    200,
                    "application/json",
                    r#"{"meta":{"serverName":"StubSkin"},"skinDomains":["example.com"]}"#.into(),
                );
            }
            if method == "POST" && path.ends_with("/authserver/authenticate") {
                assert!(body.contains("\"username\""));
                assert!(body.contains("\"password\""));
                return (
                    200,
                    "application/json",
                    r#"{
                      "accessToken":"access-1",
                      "clientToken":"client-1",
                      "selectedProfile":{"id":"00112233445566778899aabbccddeeff","name":"StubPlayer"}
                    }"#
                    .into(),
                );
            }
            if method == "POST" && path.ends_with("/authserver/refresh") {
                return (
                    200,
                    "application/json",
                    r#"{
                      "accessToken":"access-2",
                      "clientToken":"client-1",
                      "selectedProfile":{"id":"00112233445566778899aabbccddeeff","name":"StubPlayer"}
                    }"#
                    .into(),
                );
            }
            if method == "POST" && path.ends_with("/authserver/authenticate") {
                return (
                    403,
                    "application/json",
                    r#"{"error":"ForbiddenOperationException","errorMessage":"Invalid credentials."}"#.into(),
                );
            }
            (404, "text/plain", "no".into())
        });

        // Point api root at stub /api/yggdrasil
        let api = format!("{base}/api/yggdrasil");
        let acc = authenticate(&api, "user@example.com", "secret")
            .await
            .expect("auth");
        assert_eq!(acc.kind, AccountKind::SkinStation);
        assert_eq!(acc.username, "StubPlayer");
        assert_eq!(acc.access_token, "access-1");
        assert_eq!(acc.client_token.as_deref(), Some("client-1"));
        assert_eq!(acc.authlib_api_root.as_deref(), Some(api.as_str()));
        assert!(acc.uuid.contains('-'));

        let refreshed = refresh_account(&acc).await.expect("refresh");
        assert_eq!(refreshed.access_token, "access-2");
        assert_eq!(refreshed.username, "StubPlayer");
    }

    #[tokio::test]
    async fn authenticate_wrong_password_message() {
        let (base, _join) = spawn_stub(|method, path, _body| {
            if method == "GET" {
                return (
                    200,
                    "application/json",
                    r#"{"meta":{"serverName":"Stub"},"skinDomains":[]}"#.into(),
                );
            }
            if path.ends_with("/authserver/authenticate") {
                return (
                    403,
                    "application/json",
                    r#"{"error":"ForbiddenOperationException","errorMessage":"Invalid credentials. Invalid username or password."}"#.into(),
                );
            }
            (404, "text/plain", "x".into())
        });
        let err = authenticate(&format!("{base}/api/yggdrasil"), "a", "b")
            .await
            .expect_err("should fail");
        let msg = err.to_string();
        assert!(msg.contains("账号或密码"), "msg={msg}");
    }

    #[tokio::test]
    async fn ensure_jar_download_or_skip() {
        let tmp = tempfile_dir();
        let paths = Paths::from_root(&tmp);
        paths.ensure().expect("ensure dirs");
        match ensure_authlib_injector_jar(&paths).await {
            Ok(p) => {
                assert!(p.is_file());
                assert!(std::fs::metadata(&p).unwrap().len() > 10_000);
                // Second call should hit cache.
                let p2 = ensure_authlib_injector_jar(&paths).await.unwrap();
                assert_eq!(p, p2);
            }
            Err(e) => {
                let msg = e.to_string();
                // Offline / blocked CI: allow skip rather than hard-fail the suite.
                eprintln!("skip jar download (offline?): {msg}");
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    fn tempfile_dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("easymc-authlib-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn javaagent_format() {
        let acc = Account {
            id: "skin-x".into(),
            kind: AccountKind::SkinStation,
            username: "A".into(),
            uuid: "00000000-0000-0000-0000-000000000001".into(),
            access_token: "t".into(),
            refresh_token: None,
            expires_at: None,
            created_at: Utc::now(),
            authlib_api_root: Some("https://littleskin.cn/api/yggdrasil".into()),
            authlib_server_name: Some("LittleSkin".into()),
            client_token: Some("c".into()),
        };
        let arg = javaagent_arg(&acc, std::path::Path::new("/tmp/authlib-injector.jar")).unwrap();
        assert_eq!(
            arg,
            "-javaagent:/tmp/authlib-injector.jar=https://littleskin.cn/api/yggdrasil"
        );
    }
}
