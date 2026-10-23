//! Microsoft → Xbox Live → XSTS → Minecraft Services sign-in (device code flow).
//!
//! Client id: see [`crate::MicrosoftAuthConfig`] (TEMPORARY PrismLauncher public id fallback).

use crate::{Account, AccountKind, EasyMcError, MicrosoftAuthConfig, Result};
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin offline_access";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("EasyMC/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

fn http(e: reqwest::Error) -> EasyMcError {
    EasyMcError::msg(format!("网络连接失败，请检查网络后重试（{e}）"))
}

fn client_id(cfg: &MicrosoftAuthConfig) -> Result<&str> {
    cfg.client_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| EasyMcError::msg("正版登录需要登录客户端，可先离线游玩。"))
}

/// Step 1: ask Microsoft for a device code the player types at microsoft.com/link.
pub async fn request_device_code(cfg: &MicrosoftAuthConfig) -> Result<DeviceCodeInfo> {
    let id = client_id(cfg)?;
    let resp = client()
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", id), ("scope", SCOPE)])
        .send()
        .await
        .map_err(http)?;
    let status = resp.status();
    let v: Value = resp.json().await.map_err(http)?;
    if !status.is_success() {
        return Err(EasyMcError::msg(format!(
            "微软登录暂时不可用：{}",
            v["error_description"].as_str().unwrap_or("未知错误")
        )));
    }
    serde_json::from_value(v).map_err(Into::into)
}

struct MsTokens {
    access: String,
    refresh: Option<String>,
}

/// Step 2: poll until the player finishes in the browser, then run the whole chain.
pub async fn poll_device_code(cfg: &MicrosoftAuthConfig, info: &DeviceCodeInfo) -> Result<Account> {
    let id = client_id(cfg)?.to_string();
    let c = client();
    let mut interval = info.interval.max(1);
    let deadline = std::time::Instant::now() + Duration::from_secs(info.expires_in.max(60));
    let ms = loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if std::time::Instant::now() > deadline {
            return Err(EasyMcError::msg("登录代码已过期，请重新开始正版登录。"));
        }
        let resp = c
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", id.as_str()),
                ("device_code", info.device_code.as_str()),
            ])
            .send()
            .await
            .map_err(http)?;
        let v: Value = resp.json().await.map_err(http)?;
        if let Some(tok) = v["access_token"].as_str() {
            break MsTokens {
                access: tok.to_string(),
                refresh: v["refresh_token"].as_str().map(str::to_string),
            };
        }
        match v["error"].as_str().unwrap_or("") {
            "authorization_pending" => continue,
            "slow_down" => interval += 5,
            "authorization_declined" => return Err(EasyMcError::msg("你在浏览器里取消了登录。")),
            "expired_token" => return Err(EasyMcError::msg("登录代码已过期，请重新开始正版登录。")),
            other => {
                return Err(EasyMcError::msg(format!(
                    "微软登录失败（{other}），请重试。"
                )))
            }
        }
    };
    finish_chain(&c, ms).await
}

/// Refresh an MSA account (refresh token → full chain). Used before launch.
pub async fn refresh_account(cfg: &MicrosoftAuthConfig, acc: &Account) -> Result<Account> {
    let id = client_id(cfg)?;
    let rt = acc
        .refresh_token
        .as_deref()
        .ok_or_else(|| EasyMcError::msg("正版登录已失效，请在设置里重新登录微软账户。"))?;
    let c = client();
    let resp = c
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", id),
            ("refresh_token", rt),
            ("scope", SCOPE),
        ])
        .send()
        .await
        .map_err(http)?;
    let v: Value = resp.json().await.map_err(http)?;
    let access = v["access_token"]
        .as_str()
        .ok_or_else(|| EasyMcError::msg("正版登录已失效，请在设置里重新登录微软账户。"))?;
    let mut out = finish_chain(
        &c,
        MsTokens {
            access: access.to_string(),
            refresh: v["refresh_token"].as_str().map(str::to_string).or(Some(rt.to_string())),
        },
    )
    .await?;
    out.created_at = acc.created_at;
    Ok(out)
}

/// True when the stored Minecraft token is missing or expires within 5 minutes.
pub fn needs_refresh(acc: &Account) -> bool {
    acc.kind == AccountKind::Microsoft
        && acc
            .expires_at
            .map_or(true, |t| t - ChronoDuration::minutes(5) < Utc::now())
}

fn xsts_error(xerr: u64) -> EasyMcError {
    EasyMcError::msg(match xerr {
        2148916233 => "这个微软账户还没有 Xbox 档案。请先在 xbox.com 登录一次创建档案，再回来登录。".to_string(),
        2148916235 => "你所在的地区暂不支持 Xbox Live，无法正版登录。".to_string(),
        2148916236 | 2148916237 => "这个账户需要在 Xbox 网站完成成人验证后才能登录。".to_string(),
        2148916238 => "这是未满 18 岁的儿童账户，需要家长在微软家庭组中把它加入并允许后才能登录。".to_string(),
        other => format!("Xbox 登录被拒绝（代码 {other}），请稍后重试。"),
    })
}

async fn finish_chain(c: &reqwest::Client, ms: MsTokens) -> Result<Account> {
    // Xbox Live user token
    let xbl: Value = c
        .post(XBL_URL)
        .json(&json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={}", ms.access)},
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .map_err(http)?
        .json()
        .await
        .map_err(|_| EasyMcError::msg("Xbox 登录失败：这个微软账户可能还没有 Xbox 档案。"))?;
    let xbl_token = xbl["Token"]
        .as_str()
        .ok_or_else(|| EasyMcError::msg("Xbox 登录失败，请稍后重试。"))?;

    // XSTS
    let resp = c
        .post(XSTS_URL)
        .json(&json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .map_err(http)?;
    let st = resp.status();
    let xsts: Value = resp.json().await.unwrap_or(Value::Null);
    if st.as_u16() == 401 || xsts.get("XErr").is_some() {
        return Err(xsts_error(xsts["XErr"].as_u64().unwrap_or(0)));
    }
    let xsts_token = xsts["Token"]
        .as_str()
        .ok_or_else(|| EasyMcError::msg("Xbox 授权失败，请稍后重试。"))?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or_else(|| EasyMcError::msg("Xbox 授权失败，请稍后重试。"))?;

    // Minecraft services
    let resp = c
        .post(MC_LOGIN_URL)
        .json(&json!({"identityToken": format!("XBL3.0 x={uhs};{xsts_token}")}))
        .send()
        .await
        .map_err(http)?;
    if resp.status().as_u16() == 403 {
        return Err(EasyMcError::msg(
            "Minecraft 服务拒绝了这个登录客户端（尚未获得 Mojang 授权）。请稍后再试，或先离线游玩。",
        ));
    }
    let mc: Value = resp.json().await.map_err(http)?;
    let mc_token = mc["access_token"]
        .as_str()
        .ok_or_else(|| EasyMcError::msg("Minecraft 登录失败，请稍后重试。"))?
        .to_string();
    let expires = mc["expires_in"].as_i64().unwrap_or(86400);

    // Ownership + profile
    let ent: Value = c
        .get(MC_ENTITLEMENTS_URL)
        .bearer_auth(&mc_token)
        .send()
        .await
        .map_err(http)?
        .json()
        .await
        .unwrap_or(Value::Null);
    let owns = ent["items"].as_array().is_some_and(|a| !a.is_empty());
    let resp = c
        .get(MC_PROFILE_URL)
        .bearer_auth(&mc_token)
        .send()
        .await
        .map_err(http)?;
    if resp.status().as_u16() == 404 || !owns {
        return Err(EasyMcError::msg(
            "这个微软账户还没有购买 Minecraft Java 版（或尚未设置游戏名）。可以先离线游玩。",
        ));
    }
    let prof: Value = resp.json().await.map_err(http)?;
    let uuid_raw = prof["id"].as_str().unwrap_or_default();
    let name = prof["name"].as_str().unwrap_or_default();
    if uuid_raw.is_empty() || name.is_empty() {
        return Err(EasyMcError::msg("还没有设置 Minecraft 游戏名，请先在 minecraft.net 设置后再登录。"));
    }
    let uuid = uuid::Uuid::parse_str(uuid_raw)
        .map(|u| u.as_hyphenated().to_string())
        .unwrap_or_else(|_| uuid_raw.to_string());
    Ok(Account {
        id: format!("msa-{uuid}"),
        kind: AccountKind::Microsoft,
        username: name.to_string(),
        uuid,
        access_token: mc_token,
        refresh_token: ms.refresh,
        expires_at: Some(Utc::now() + ChronoDuration::seconds(expires)),
        created_at: Utc::now(),
    })
}
