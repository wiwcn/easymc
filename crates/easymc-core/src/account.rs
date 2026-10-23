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

pub fn create_offline_account(username: &str) -> Account {
    let uuid = offline_player_uuid(username);
    Account {
        id: format!("offline-{uuid}"),
        kind: AccountKind::Offline,
        username: username.to_string(),
        uuid: uuid.as_hyphenated().to_string(),
        access_token: "0".into(),
        refresh_token: None,
        expires_at: None,
        created_at: Utc::now(),
    }
}

/// Microsoft auth scaffolding. Needs an Azure AD application (public client)
/// registered for Minecraft launcher-style OAuth before this can complete.
///
/// TODO (user / maintainer):
/// 1. Register an Azure app: https://portal.azure.com → App registrations
/// 2. Set public client redirect (e.g. `http://127.0.0.1:PORT/callback`) or device code
/// 3. Put client id in env `EASYMC_MSA_CLIENT_ID` or settings
/// 4. Implement device-code or localhost redirect + Xbox Live + XSTS + MC services
///
/// Until then, offline accounts remain the supported play path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrosoftAuthConfig {
    /// Azure application (client) id. None → MSA login disabled.
    pub client_id: Option<String>,
    pub redirect_uri: String,
}

impl Default for MicrosoftAuthConfig {
    fn default() -> Self {
        Self {
            client_id: std::env::var("EASYMC_MSA_CLIENT_ID").ok().filter(|s| !s.is_empty()),
            redirect_uri: "http://127.0.0.1:0/callback".into(),
        }
    }
}

impl MicrosoftAuthConfig {
    pub fn is_configured(&self) -> bool {
        self.client_id.as_ref().is_some_and(|c| !c.is_empty())
    }
}

/// Placeholder for the multi-step MSA → Xbox → Minecraft services flow.
pub async fn begin_microsoft_login(cfg: &MicrosoftAuthConfig) -> crate::Result<Account> {
    if !cfg.is_configured() {
        return Err(crate::EasyMcError::msg(
            "Microsoft 登录未配置：请设置 EASYMC_MSA_CLIENT_ID（Azure 应用客户端 ID），或使用离线账户",
        ));
    }
    Err(crate::EasyMcError::NotImplemented(
        "Microsoft OAuth + Xbox Live + Minecraft services token exchange",
    ))
}
