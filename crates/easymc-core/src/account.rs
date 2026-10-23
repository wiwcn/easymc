use chrono::{DateTime, Utc};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    /// Local offline / demo play (no Microsoft login).
    Offline,
    /// Microsoft / Xbox account (MSA). Requires Azure app registration.
    Microsoft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub kind: AccountKind,
    pub username: String,
    /// UUID used as --uuid (offline: nameUUIDFromBytes; MSA: profile id).
    pub uuid: String,
    /// Access token for --accessToken (offline: dummy "0").
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Java `UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))`.
pub fn offline_player_uuid(name: &str) -> Uuid {
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{name}").as_bytes());
    let mut bytes = hasher.finalize();
    bytes[6] = (bytes[6] & 0x0f) | 0x30; // version 3
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // IETF variant
    Uuid::from_bytes(bytes.into())
}

/// Player-facing offline username checks (aligned with PCL Legacy naming tips).
/// Allows Unicode (e.g. Chinese) for offline; classic Mojang ASCII gets a soft tip via `is_classic_mojang_name`.
pub fn validate_offline_username(username: &str) -> crate::Result<()> {
    let name = username.trim();
    if name.is_empty() {
        return Err(crate::EasyMcError::msg("请输入游戏里显示的名字"));
    }
    let len = name.chars().count();
    if len < 2 {
        return Err(crate::EasyMcError::msg("名字至少 2 个字符"));
    }
    if len > 16 {
        return Err(crate::EasyMcError::msg("名字最多 16 个字符"));
    }
    if name.chars().any(|c| c.is_control() || c == '\n' || c == '\r' || c == '\t') {
        return Err(crate::EasyMcError::msg("名字不能包含控制字符或换行"));
    }
    Ok(())
}

/// Classic Mojang-style name: 3–16 chars of `[A-Za-z0-9_]`. Offline allows more; UI may tip.
pub fn is_classic_mojang_name(username: &str) -> bool {
    let name = username.trim();
    let len = name.chars().count();
    (3..=16).contains(&len) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn create_offline_account(username: &str) -> crate::Result<Account> {
    validate_offline_username(username)?;
    let username = username.trim();
    let uuid = offline_player_uuid(username);
    Ok(Account {
        id: format!("offline-{uuid}"),
        kind: AccountKind::Offline,
        username: username.to_string(),
        uuid: uuid.as_hyphenated().to_string(),
        access_token: "0".into(),
        refresh_token: None,
        expires_at: None,
        created_at: Utc::now(),
    })
}


/// Microsoft auth scaffolding. Needs an Azure AD application (public client)
/// registered for Minecraft launcher-style OAuth before this can complete.
///
/// Resolution order for client id:
/// 1. Env `EASYMC_MSA_CLIENT_ID`
/// 2. `settings.msa_client_id`
///
/// No fallback id is bundled: DESIGN §2.4 forbids inventing / borrowing an unverified UUID.
/// Until one is configured, MSA login reports 「正版登录即将可用」.
///
/// Production: register own Azure public client + Mojang Minecraft API whitelist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrosoftAuthConfig {
    /// Azure application (client) id. None → MSA login disabled.
    pub client_id: Option<String>,
    pub redirect_uri: String,
    /// Reserved: true if a temporary public client id is ever used (currently never).
    #[serde(default)]
    pub using_temporary_client_id: bool,
}

impl Default for MicrosoftAuthConfig {
    fn default() -> Self {
        Self::resolve(None)
    }
}

impl MicrosoftAuthConfig {
    pub fn resolve(settings_client_id: Option<&str>) -> Self {
        let from_env = std::env::var("EASYMC_MSA_CLIENT_ID")
            .ok()
            .filter(|s| !s.is_empty());
        let from_settings = settings_client_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let (client_id, using_temporary_client_id) = if let Some(id) = from_env.or(from_settings) {
            (Some(id), false)
        } else {
            (None, false)
        };
        Self {
            client_id,
            redirect_uri: "http://127.0.0.1:0/callback".into(),
            using_temporary_client_id,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.client_id.as_ref().is_some_and(|c| !c.is_empty())
    }
}

/// Placeholder for the multi-step MSA → Xbox → Minecraft services flow.
pub async fn begin_microsoft_login(cfg: &MicrosoftAuthConfig) -> crate::Result<Account> {
    if !cfg.is_configured() {
        return Err(crate::EasyMcError::msg(
            "正版登录即将可用，可先离线游玩。",
        ));
    }
    // Client id present (env / settings), but Device Code →
    // Xbox Live → XSTS → Minecraft Services exchange is not wired yet.
    Err(crate::EasyMcError::msg(
        "正版登录即将可用。微软登录流程尚未接通，请先用离线名字游玩，或稍后再试。",
    ))
}
