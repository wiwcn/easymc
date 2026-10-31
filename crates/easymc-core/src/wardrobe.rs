//! Local skin wardrobe: upload / replace / import / active selection.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::skin::{
    download_texture_bytes, validate_cape_png, validate_skin_png, ResolvedSkin, SkinModel,
    SkinSourceKind,
};
use crate::{EasyMcError, Paths, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WardrobeEntry {
    pub id: String,
    pub name: String,
    pub model: SkinModel,
    pub source: String,
    pub source_label: String,
    pub has_cape: bool,
    pub account_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WardrobeStore {
    pub entries: Vec<WardrobeEntry>,
    pub active_id: Option<String>,
}

fn load_store(paths: &Paths) -> Result<WardrobeStore> {
    let f = paths.wardrobe_file();
    if !f.exists() {
        return Ok(WardrobeStore::default());
    }
    let raw = std::fs::read_to_string(&f)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn save_store(paths: &Paths, store: &WardrobeStore) -> Result<()> {
    paths.ensure()?;
    std::fs::create_dir_all(paths.skins_files_dir())?;
    let f = paths.wardrobe_file();
    let tmp = f.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(store)?)?;
    std::fs::rename(&tmp, &f)?;
    Ok(())
}

fn skin_path(paths: &Paths, id: &str) -> std::path::PathBuf {
    paths.skins_files_dir().join(format!("{id}.png"))
}
fn cape_path(paths: &Paths, id: &str) -> std::path::PathBuf {
    paths.skins_files_dir().join(format!("{id}_cape.png"))
}
fn to_data_url(bytes: &[u8]) -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[derive(Debug, Clone, Serialize)]
pub struct WardrobeEntryDto {
    pub id: String,
    pub name: String,
    pub model: String,
    pub source: String,
    pub source_label: String,
    pub has_cape: bool,
    pub skin_data_url: String,
    pub cape_data_url: Option<String>,
    pub account_id: Option<String>,
    pub created_at: String,
}

pub fn entry_to_dto(paths: &Paths, e: &WardrobeEntry) -> Result<WardrobeEntryDto> {
    let skin_bytes = std::fs::read(skin_path(paths, &e.id))
        .map_err(|_| EasyMcError::msg("衣柜皮肤文件丢失，请重新导入"))?;
    let cape_data_url = if e.has_cape {
        std::fs::read(cape_path(paths, &e.id)).ok().map(|b| to_data_url(&b))
    } else {
        None
    };
    Ok(WardrobeEntryDto {
        id: e.id.clone(),
        name: e.name.clone(),
        model: e.model.as_str().into(),
        source: e.source.clone(),
        source_label: e.source_label.clone(),
        has_cape: e.has_cape,
        skin_data_url: to_data_url(&skin_bytes),
        cape_data_url,
        account_id: e.account_id.clone(),
        created_at: e.created_at.to_rfc3339(),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct WardrobeStateDto {
    pub entries: Vec<WardrobeEntryDto>,
    pub active_id: Option<String>,
}

pub fn wardrobe_state(paths: &Paths) -> Result<WardrobeStateDto> {
    paths.ensure()?;
    let store = load_store(paths)?;
    let mut entries = Vec::with_capacity(store.entries.len());
    for e in &store.entries {
        if let Ok(d) = entry_to_dto(paths, e) {
            entries.push(d);
        }
    }
    Ok(WardrobeStateDto { entries, active_id: store.active_id })
}

pub fn set_active_wardrobe(paths: &Paths, id: Option<&str>) -> Result<WardrobeStateDto> {
    let mut store = load_store(paths)?;
    if let Some(id) = id {
        if !store.entries.iter().any(|e| e.id == id) {
            return Err(EasyMcError::msg("衣柜里找不到这件皮肤"));
        }
        store.active_id = Some(id.to_string());
    } else {
        store.active_id = None;
    }
    save_store(paths, &store)?;
    wardrobe_state(paths)
}

pub fn wardrobe_upload(
    paths: &Paths,
    skin: &[u8],
    cape: Option<&[u8]>,
    name: Option<&str>,
    model: Option<SkinModel>,
) -> Result<WardrobeEntryDto> {
    validate_skin_png(skin)?;
    if let Some(c) = cape {
        validate_cape_png(c)?;
    }
    paths.ensure()?;
    std::fs::create_dir_all(paths.skins_files_dir())?;
    let id = Uuid::new_v4().as_hyphenated().to_string();
    std::fs::write(skin_path(paths, &id), skin)?;
    let has_cape = if let Some(c) = cape {
        std::fs::write(cape_path(paths, &id), c)?;
        true
    } else {
        false
    };
    let entry = WardrobeEntry {
        id: id.clone(),
        name: name.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("我的皮肤").to_string(),
        model: model.unwrap_or(SkinModel::Default),
        source: SkinSourceKind::Upload.as_str().into(),
        source_label: SkinSourceKind::Upload.label_zh().into(),
        has_cape,
        account_id: None,
        created_at: Utc::now(),
    };
    let mut store = load_store(paths)?;
    store.entries.insert(0, entry.clone());
    if store.active_id.is_none() {
        store.active_id = Some(id);
    }
    save_store(paths, &store)?;
    entry_to_dto(paths, &entry)
}

pub fn wardrobe_replace(
    paths: &Paths,
    id: &str,
    skin: Option<&[u8]>,
    cape: Option<&[u8]>,
    name: Option<&str>,
    model: Option<SkinModel>,
    clear_cape: bool,
) -> Result<WardrobeEntryDto> {
    let mut store = load_store(paths)?;
    let entry = store.entries.iter_mut().find(|e| e.id == id)
        .ok_or_else(|| EasyMcError::msg("衣柜里找不到这件皮肤"))?;
    if let Some(s) = skin {
        validate_skin_png(s)?;
        std::fs::write(skin_path(paths, id), s)?;
    }
    if clear_cape {
        let _ = std::fs::remove_file(cape_path(paths, id));
        entry.has_cape = false;
    } else if let Some(c) = cape {
        validate_cape_png(c)?;
        std::fs::write(cape_path(paths, id), c)?;
        entry.has_cape = true;
    }
    if let Some(n) = name.map(str::trim).filter(|s| !s.is_empty()) {
        entry.name = n.to_string();
    }
    if let Some(m) = model {
        entry.model = m;
    }
    let snapshot = entry.clone();
    save_store(paths, &store)?;
    entry_to_dto(paths, &snapshot)
}

pub fn wardrobe_remove(paths: &Paths, id: &str) -> Result<WardrobeStateDto> {
    let mut store = load_store(paths)?;
    let before = store.entries.len();
    store.entries.retain(|e| e.id != id);
    if store.entries.len() == before {
        return Err(EasyMcError::msg("衣柜里找不到这件皮肤"));
    }
    if store.active_id.as_deref() == Some(id) {
        store.active_id = store.entries.first().map(|e| e.id.clone());
    }
    let _ = std::fs::remove_file(skin_path(paths, id));
    let _ = std::fs::remove_file(cape_path(paths, id));
    save_store(paths, &store)?;
    wardrobe_state(paths)
}

pub async fn wardrobe_import_resolved(
    paths: &Paths,
    resolved: &ResolvedSkin,
    name: Option<&str>,
) -> Result<WardrobeEntryDto> {
    let skin = download_texture_bytes(&resolved.skin_url).await?;
    validate_skin_png(&skin)?;
    let cape = if let Some(ref u) = resolved.cape_url {
        match download_texture_bytes(u).await {
            Ok(b) => { let _ = validate_cape_png(&b); Some(b) }
            Err(_) => None,
        }
    } else {
        None
    };
    paths.ensure()?;
    std::fs::create_dir_all(paths.skins_files_dir())?;
    let id = Uuid::new_v4().as_hyphenated().to_string();
    std::fs::write(skin_path(paths, &id), &skin)?;
    let has_cape = if let Some(ref c) = cape {
        std::fs::write(cape_path(paths, &id), c)?;
        true
    } else {
        false
    };
    let entry = WardrobeEntry {
        id: id.clone(),
        name: name.map(str::trim).filter(|s| !s.is_empty()).unwrap_or(resolved.name.as_str()).to_string(),
        model: resolved.model,
        source: resolved.source.as_str().into(),
        source_label: resolved.source.label_zh().into(),
        has_cape,
        account_id: None,
        created_at: Utc::now(),
    };
    let mut store = load_store(paths)?;
    store.entries.insert(0, entry.clone());
    store.active_id = Some(id);
    save_store(paths, &store)?;
    entry_to_dto(paths, &entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_64() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        v.extend_from_slice(&13u32.to_be_bytes());
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&64u32.to_be_bytes());
        v.extend_from_slice(&64u32.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0]);
        v.extend_from_slice(&[0, 0, 0, 0]);
        v.extend_from_slice(&[0u8; 64]);
        v
    }

    #[test]
    fn upload_and_list_and_remove() {
        let dir = std::env::temp_dir().join(format!("easymc-wardrobe-{}", Uuid::new_v4()));
        let paths = Paths::from_root(&dir);
        paths.ensure().unwrap();
        let dto = wardrobe_upload(&paths, &png_64(), None, Some("测试"), Some(SkinModel::Slim)).unwrap();
        assert_eq!(dto.name, "测试");
        assert_eq!(dto.model, "slim");
        assert!(dto.skin_data_url.starts_with("data:image/png;base64,"));
        let st = wardrobe_state(&paths).unwrap();
        assert_eq!(st.entries.len(), 1);
        let st2 = wardrobe_remove(&paths, &dto.id).unwrap();
        assert!(st2.entries.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replace_name_and_model() {
        let dir = std::env::temp_dir().join(format!("easymc-wardrobe2-{}", Uuid::new_v4()));
        let paths = Paths::from_root(&dir);
        paths.ensure().unwrap();
        let dto = wardrobe_upload(&paths, &png_64(), None, Some("A"), None).unwrap();
        let dto2 = wardrobe_replace(&paths, &dto.id, None, None, Some("B"), Some(SkinModel::Slim), false).unwrap();
        assert_eq!(dto2.name, "B");
        assert_eq!(dto2.model, "slim");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
