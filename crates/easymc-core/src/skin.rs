//! Official (Mojang / BMCLAPI) and LittleSkin CSL skin fetch.

use base64::Engine;
use serde::Deserialize;
use serde_json::Value;

use crate::{EasyMcError, Result};

const MOJANG_PROFILE: &str = "https://api.mojang.com/users/profiles/minecraft/";
const SESSION_OFFICIAL: &str = "https://sessionserver.mojang.com/session/minecraft/profile/";
const SESSION_BMCL: &str = "https://bmclapi2.bangbang93.com/session/minecraft/profile/";
const LITTLESKIN_CSL: &str = "https://littleskin.cn/csl/";
const LITTLESKIN_TEX: &str = "https://littleskin.cn/textures/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinModel {
    Default,
    Slim,
}

impl SkinModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Slim => "slim",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "slim" | "alex" | "arms_slim" => Self::Slim,
            _ => Self::Default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinSourceKind {
    Official,
    LittleSkin,
    Upload,
    Wardrobe,
}

impl SkinSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::LittleSkin => "little_skin",
            Self::Upload => "upload",
            Self::Wardrobe => "wardrobe",
        }
    }
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Official => "正版皮肤",
            Self::LittleSkin => "LittleSkin",
            Self::Upload => "本地上传",
            Self::Wardrobe => "衣柜",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedSkin {
    pub name: String,
    pub uuid: Option<String>,
    pub model: SkinModel,
    pub skin_url: String,
    pub cape_url: Option<String>,
    pub source: SkinSourceKind,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("EasyMC/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn http(e: reqwest::Error) -> EasyMcError {
    EasyMcError::Http(e.to_string())
}

pub fn validate_skin_png(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err(EasyMcError::msg("不是有效的 PNG 皮肤文件"));
    }
    if &bytes[12..16] != b"IHDR" {
        return Err(EasyMcError::msg("PNG 格式异常"));
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    let ok = (w == 64 && (h == 32 || h == 64))
        || (w >= 64 && w % 64 == 0 && h >= 32 && (h == w || h == w / 2));
    if !ok {
        return Err(EasyMcError::msg(format!(
            "皮肤尺寸应为 64×64（或 64×32），当前是 {w}×{h}"
        )));
    }
    Ok((w, h))
}

pub fn validate_cape_png(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err(EasyMcError::msg("不是有效的披风 PNG"));
    }
    if &bytes[12..16] != b"IHDR" {
        return Err(EasyMcError::msg("披风 PNG 格式异常"));
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if w < 22 || h < 17 {
        return Err(EasyMcError::msg(format!("披风尺寸过小（{w}×{h}），请换一张")));
    }
    Ok((w, h))
}

pub fn read_local_file(path: &str) -> Result<Vec<u8>> {
    let p = std::path::Path::new(path);
    if !p.is_file() {
        return Err(EasyMcError::msg("找不到所选文件"));
    }
    let bytes = std::fs::read(p)?;
    if bytes.is_empty() {
        return Err(EasyMcError::msg("文件是空的"));
    }
    Ok(bytes)
}

pub async fn download_texture_bytes(url: &str) -> Result<Vec<u8>> {
    let c = client();
    let resp = c.get(url).send().await.map_err(http)?;
    if !resp.status().is_success() {
        return Err(EasyMcError::msg(format!("下载皮肤失败（HTTP {}）", resp.status())));
    }
    Ok(resp.bytes().await.map_err(http)?.to_vec())
}

fn normalize_uuid(raw: &str) -> Option<String> {
    let s: String = raw.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if s.len() != 32 {
        return None;
    }
    Some(format!(
        "{}-{}-{}-{}-{}",
        &s[0..8], &s[8..12], &s[12..16], &s[16..20], &s[20..32]
    ))
}

fn looks_like_uuid(s: &str) -> bool {
    s.chars().filter(|c| c.is_ascii_hexdigit()).count() == 32
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

async fn resolve_uuid(c: &reqwest::Client, query: &str) -> Result<(String, String)> {
    let q = query.trim();
    if looks_like_uuid(q) {
        let uuid = normalize_uuid(q).ok_or_else(|| EasyMcError::msg("UUID 格式不对"))?;
        return Ok((uuid, q.to_string()));
    }
    if q.is_empty() {
        return Err(EasyMcError::msg("请输入正版玩家名或 UUID"));
    }
    let url = format!("{MOJANG_PROFILE}{}", urlencoding_simple(q));
    let resp = c.get(&url).send().await.map_err(http)?;
    if resp.status().as_u16() == 204 || resp.status().as_u16() == 404 {
        return Err(EasyMcError::msg(format!("找不到正版玩家「{q}」")));
    }
    if !resp.status().is_success() {
        return Err(EasyMcError::msg(format!("查询正版玩家失败（HTTP {}）", resp.status())));
    }
    let v: Value = resp.json().await.map_err(http)?;
    let id = v["id"].as_str().ok_or_else(|| EasyMcError::msg("正版接口没有返回 UUID"))?;
    let name = v["name"].as_str().unwrap_or(q).to_string();
    let uuid = normalize_uuid(id).ok_or_else(|| EasyMcError::msg("UUID 格式不对"))?;
    Ok((uuid, name))
}

#[derive(Debug, Deserialize)]
struct SessionProfile {
    name: Option<String>,
    properties: Option<Vec<SessionProp>>,
}
#[derive(Debug, Deserialize)]
struct SessionProp { name: String, value: String }
#[derive(Debug, Deserialize)]
struct TexturesPayload { textures: Option<TexturesMap> }
#[derive(Debug, Deserialize)]
struct TexturesMap {
    #[serde(rename = "SKIN")]
    skin: Option<TextureEntry>,
    #[serde(rename = "CAPE")]
    cape: Option<TextureEntry>,
}
#[derive(Debug, Deserialize)]
struct TextureEntry {
    url: String,
    metadata: Option<TextureMeta>,
}
#[derive(Debug, Deserialize)]
struct TextureMeta { model: Option<String> }

fn decode_textures(value_b64: &str) -> Result<(SkinModel, String, Option<String>)> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(value_b64.trim())
        .map_err(|_| EasyMcError::msg("皮肤纹理数据损坏"))?;
    let payload: TexturesPayload =
        serde_json::from_slice(&raw).map_err(|_| EasyMcError::msg("皮肤纹理 JSON 损坏"))?;
    let textures = payload.textures.ok_or_else(|| EasyMcError::msg("该玩家没有皮肤纹理"))?;
    let skin = textures.skin.ok_or_else(|| EasyMcError::msg("该玩家没有皮肤"))?;
    let model = match skin.metadata.as_ref().and_then(|m| m.model.as_deref()).unwrap_or("default") {
        "slim" => SkinModel::Slim,
        _ => SkinModel::Default,
    };
    Ok((model, skin.url, textures.cape.map(|c| c.url)))
}

async fn fetch_session(c: &reqwest::Client, uuid_hyphen: &str) -> Result<SessionProfile> {
    let bare: String = uuid_hyphen.chars().filter(|ch| *ch != '-').collect();
    let urls = [format!("{SESSION_OFFICIAL}{bare}"), format!("{SESSION_BMCL}{bare}")];
    let mut last_err = EasyMcError::msg("无法获取正版皮肤");
    for url in urls {
        match c.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<SessionProfile>().await {
                Ok(p) => return Ok(p),
                Err(e) => last_err = http(e),
            },
            Ok(resp) => last_err = EasyMcError::msg(format!("正版皮肤接口失败（HTTP {}）", resp.status())),
            Err(e) => last_err = http(e),
        }
    }
    Err(last_err)
}

pub async fn fetch_official(query: &str) -> Result<ResolvedSkin> {
    let c = client();
    let (uuid, name) = resolve_uuid(&c, query).await?;
    let profile = fetch_session(&c, &uuid).await?;
    let display = profile.name.unwrap_or(name);
    let props = profile.properties.unwrap_or_default();
    let textures = props.into_iter().find(|p| p.name == "textures")
        .ok_or_else(|| EasyMcError::msg("该正版账户没有公开皮肤"))?;
    let (model, skin_url, cape_url) = decode_textures(&textures.value)?;
    Ok(ResolvedSkin {
        name: display, uuid: Some(uuid), model, skin_url, cape_url, source: SkinSourceKind::Official,
    })
}

#[derive(Debug, Deserialize)]
struct CslProfile {
    username: Option<String>,
    textures: Option<CslTextures>,
    skin: Option<String>,
    cape: Option<String>,
}
#[derive(Debug, Deserialize)]
struct CslTextures {
    #[serde(rename = "SKIN")]
    skin: Option<CslTex>,
    #[serde(rename = "CAPE")]
    cape: Option<CslTex>,
}
#[derive(Debug, Deserialize)]
struct CslTex {
    url: Option<String>,
    hash: Option<String>,
    metadata: Option<TextureMeta>,
}

fn csl_texture_url(hash_or_url: &str) -> String {
    if hash_or_url.starts_with("http://") || hash_or_url.starts_with("https://") {
        hash_or_url.to_string()
    } else {
        format!("{LITTLESKIN_TEX}{hash_or_url}")
    }
}

pub async fn fetch_littleskin_by_name(name: &str) -> Result<ResolvedSkin> {
    let name = name.trim();
    if name.is_empty() {
        return Err(EasyMcError::msg("请输入 LittleSkin 玩家名"));
    }
    let c = client();
    let url = format!("{LITTLESKIN_CSL}{}.json", urlencoding_simple(name));
    let resp = c.get(&url).send().await.map_err(http)?;
    if resp.status().as_u16() == 404 {
        return Err(EasyMcError::msg(format!("LittleSkin 上找不到「{name}」")));
    }
    if !resp.status().is_success() {
        return Err(EasyMcError::msg(format!("LittleSkin 请求失败（HTTP {}）", resp.status())));
    }
    let profile: CslProfile = resp.json().await.map_err(http)?;
    let display = profile.username.unwrap_or_else(|| name.to_string());
    let (model, skin_url, cape_url) = if let Some(tex) = profile.textures {
        let skin = tex.skin.ok_or_else(|| EasyMcError::msg("该 LittleSkin 账户没有皮肤"))?;
        let model = match skin.metadata.as_ref().and_then(|m| m.model.as_deref()).unwrap_or("default") {
            "slim" => SkinModel::Slim,
            _ => SkinModel::Default,
        };
        let skin_url = skin.url.or(skin.hash).map(|s| csl_texture_url(&s))
            .ok_or_else(|| EasyMcError::msg("LittleSkin 皮肤地址缺失"))?;
        let cape_url = tex.cape.and_then(|c| c.url.or(c.hash)).map(|s| csl_texture_url(&s));
        (model, skin_url, cape_url)
    } else if let Some(hash) = profile.skin {
        (SkinModel::Default, csl_texture_url(&hash), profile.cape.map(|h| csl_texture_url(&h)))
    } else {
        return Err(EasyMcError::msg("该 LittleSkin 账户没有皮肤"));
    };
    Ok(ResolvedSkin {
        name: display, uuid: None, model, skin_url, cape_url, source: SkinSourceKind::LittleSkin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png(w: u32, h: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        v.extend_from_slice(&13u32.to_be_bytes());
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0]);
        v.extend_from_slice(&[0, 0, 0, 0]);
        v
    }

    #[test]
    fn validate_skin_accepts_64x64() {
        assert_eq!(validate_skin_png(&tiny_png(64, 64)).unwrap(), (64, 64));
    }
    #[test]
    fn validate_skin_accepts_legacy_64x32() {
        assert!(validate_skin_png(&tiny_png(64, 32)).is_ok());
    }
    #[test]
    fn validate_skin_rejects_garbage() {
        assert!(validate_skin_png(b"not a png").is_err());
    }
    #[test]
    fn validate_skin_rejects_bad_size() {
        assert!(validate_skin_png(&tiny_png(32, 32)).is_err());
    }
    #[test]
    fn normalize_uuid_hyphenates() {
        assert_eq!(
            normalize_uuid("069a79f444e94726a5befca90e38aaf5").unwrap(),
            "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        );
    }
    #[test]
    fn decode_textures_skin_and_cape() {
        let json = r#"{"textures":{"SKIN":{"url":"https://textures.minecraft.net/texture/abc","metadata":{"model":"slim"}},"CAPE":{"url":"https://textures.minecraft.net/texture/cape"}}}"#;
        let b64 = base64::engine::general_purpose::STANDARD.encode(json.as_bytes());
        let (model, skin, cape) = decode_textures(&b64).unwrap();
        assert_eq!(model, SkinModel::Slim);
        assert!(skin.contains("abc"));
        assert!(cape.unwrap().contains("cape"));
    }
    #[test]
    fn skin_model_parse() {
        assert_eq!(SkinModel::parse("slim"), SkinModel::Slim);
        assert_eq!(SkinModel::parse("default"), SkinModel::Default);
    }
}
