use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{EasyMcError, Result};

/// Marker filenames that enable portable ("green") mode when placed next to the binary.
/// Either name works; Settings writes `portable.txt` by default.
pub const PORTABLE_MARKER_NAMES: &[&str] = &["portable.txt", "easymc-portable"];

/// Preferred marker created by the Settings toggle.
pub const PORTABLE_MARKER_PRIMARY: &str = "portable.txt";

/// Subfolder under the install/app directory used as the data root in portable mode.
pub const PORTABLE_DATA_FOLDER: &str = "data";

/// Root data directory for EasyMC (versions, libraries, assets, instances, runtimes).
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

/// Snapshot of how the data directory was / will be resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableStatus {
    /// Marker present next to the binary (ignores env override for this flag).
    pub portable_marker: bool,
    /// Absolute path of the marker file if present.
    pub marker_path: Option<PathBuf>,
    /// Directory that contains the running binary (install / unpack folder).
    pub app_dir: Option<PathBuf>,
    /// Data root currently in use by this process.
    pub active_data_dir: PathBuf,
    /// Where data would live with portable mode on (`<app_dir>/data`).
    pub portable_data_dir: Option<PathBuf>,
    /// OS-standard data dir (`…/EasyMC`), independent of portable.
    pub standard_data_dir: Option<PathBuf>,
    /// `EASYMC_DATA_DIR` is set and non-empty — always wins over marker / OS paths.
    pub env_override: bool,
    pub env_data_dir: Option<PathBuf>,
    /// Standard dir exists and looks populated (instances.json / versions / …).
    pub standard_has_data: bool,
    /// Portable data dir exists and looks populated.
    pub portable_has_data: bool,
    /// Human-readable Chinese summary for UI / Agent.
    pub message_zh: String,
}

/// Result of a safe (non-destructive) copy from standard → portable data dir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrateResult {
    pub ok: bool,
    pub copied_entries: u64,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub message_zh: String,
}

impl Paths {
    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Prefer `EASYMC_DATA_DIR`, else portable `<app>/data` when a marker is present,
    /// else the platform data dir / `EasyMC`.
    pub fn discover() -> Result<Self> {
        Self::discover_with(install_dir())
    }

    /// Same as [`discover`] but with an injectable install/app directory (for tests).
    pub fn discover_with(app_dir: Option<PathBuf>) -> Result<Self> {
        if let Some(p) = env_data_dir() {
            return Ok(Self::from_root(p));
        }
        if let Some(dir) = app_dir.as_ref() {
            if portable_marker_in(dir).is_some() {
                return Ok(Self::from_root(portable_data_dir(dir)));
            }
        }
        Ok(Self::from_root(standard_data_dir()?))
    }

    pub fn ensure(&self) -> Result<()> {
        for p in [
            self.root.clone(),
            self.meta_dir(),
            self.versions_dir(),
            self.libraries_dir(),
            self.assets_dir(),
            self.assets_indexes_dir(),
            self.assets_objects_dir(),
            self.instances_dir(),
            self.runtimes_dir(),
            self.natives_dir(),
            self.skins_dir(),
            self.skins_files_dir(),
        ] {
            std::fs::create_dir_all(p)?;
        }
        Ok(())
    }

    pub fn meta_dir(&self) -> PathBuf {
        self.root.join("meta")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions_dir().join(id)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }
    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }
    pub fn libraries_dir(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn assets_indexes_dir(&self) -> PathBuf {
        self.assets_dir().join("indexes")
    }
    pub fn assets_objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }
    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn instance_game_dir(&self, id: &str) -> PathBuf {
        self.instances_dir().join(id).join(".minecraft")
    }
    pub fn runtimes_dir(&self) -> PathBuf {
        self.root.join("runtimes")
    }
    pub fn natives_dir(&self) -> PathBuf {
        self.root.join("natives")
    }
    pub fn natives_for_version(&self, id: &str) -> PathBuf {
        self.natives_dir().join(id)
    }
    pub fn instances_file(&self) -> PathBuf {
        self.root.join("instances.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    pub fn library_path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.libraries_dir().join(relative)
    }

    pub fn skins_dir(&self) -> PathBuf {
        self.root.join("skins")
    }
    pub fn skins_files_dir(&self) -> PathBuf {
        self.skins_dir().join("files")
    }
    pub fn wardrobe_file(&self) -> PathBuf {
        self.skins_dir().join("wardrobe.json")
    }
}

/// Directory containing the running executable, if resolvable.
pub fn install_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// OS-standard EasyMC data directory (not portable).
pub fn standard_data_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| EasyMcError::msg("no data_dir"))?;
    Ok(base.join("EasyMC"))
}

/// Portable data root adjacent to the app: `<app_dir>/data`.
pub fn portable_data_dir(app_dir: &Path) -> PathBuf {
    app_dir.join(PORTABLE_DATA_FOLDER)
}

fn env_data_dir() -> Option<PathBuf> {
    std::env::var_os("EASYMC_DATA_DIR").and_then(|v| {
        let s = v.to_string_lossy();
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(PathBuf::from(t))
        }
    })
}

/// First matching portable marker under `app_dir`, if any.
pub fn portable_marker_in(app_dir: &Path) -> Option<PathBuf> {
    for name in PORTABLE_MARKER_NAMES {
        let p = app_dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn dir_looks_populated(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    // Any of the well-known files/dirs means the player already has data here.
    for name in [
        "instances.json",
        "accounts.json",
        "settings.json",
        "instances",
        "versions",
        "libraries",
        "assets",
        "skins",
    ] {
        if dir.join(name).exists() {
            return true;
        }
    }
    // Non-empty directory also counts (e.g. partial migrate).
    fs::read_dir(dir)
        .map(|mut rd| rd.next().is_some())
        .unwrap_or(false)
}

/// Build a status snapshot. `active_root` is the data dir this process already opened.
pub fn portable_status(active_root: &Path) -> PortableStatus {
    let app_dir = install_dir();
    let marker_path = app_dir.as_ref().and_then(|d| portable_marker_in(d));
    let portable_marker = marker_path.is_some();
    let portable_data = app_dir.as_ref().map(|d| portable_data_dir(d));
    let standard = standard_data_dir().ok();
    let env = env_data_dir();
    let env_override = env.is_some();
    let standard_has_data = standard.as_ref().map(|p| dir_looks_populated(p)).unwrap_or(false);
    let portable_has_data = portable_data
        .as_ref()
        .map(|p| dir_looks_populated(p))
        .unwrap_or(false);

    let message_zh = if env_override {
        format!(
            "当前由环境变量 EASYMC_DATA_DIR 指定数据目录：{}",
            env.as_ref().unwrap().display()
        )
    } else if portable_marker {
        format!(
            "便携模式已开启（标记文件在程序旁）。数据目录：{}",
            portable_data
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| active_root.display().to_string())
        )
    } else {
        format!(
            "普通模式：数据在系统目录。当前：{}",
            active_root.display()
        )
    };

    PortableStatus {
        portable_marker,
        marker_path,
        app_dir,
        active_data_dir: active_root.to_path_buf(),
        portable_data_dir: portable_data,
        standard_data_dir: standard,
        env_override,
        env_data_dir: env,
        standard_has_data,
        portable_has_data,
        message_zh,
    }
}

/// Create or remove the portable marker next to the binary.
/// Does **not** move data; caller should offer migrate + ask the user to restart.
pub fn set_portable_marker(enable: bool) -> Result<PortableStatus> {
    let app_dir = install_dir().ok_or_else(|| {
        EasyMcError::msg("无法定位程序目录，无法写入便携标记。请把 portable.txt 手动放到 easymc 程序旁边。")
    })?;
    if enable {
        let marker = app_dir.join(PORTABLE_MARKER_PRIMARY);
        // Idempotent: also fine if easymc-portable already exists.
        if portable_marker_in(&app_dir).is_none() {
            fs::write(
                &marker,
                "# EasyMC portable mode\n# 此文件表示启用便携（绿色）目录：数据写在程序旁的 data/ 下。\n# 删除本文件并重启即可恢复系统默认数据目录。\n",
            )
            .map_err(|e| {
                EasyMcError::msg(format!(
                    "无法在程序目录写入 {PORTABLE_MARKER_PRIMARY}：{e}。若安装在受保护位置，请改用免安装包，或手动把该文件放到程序旁边。"
                ))
            })?;
        }
    } else {
        for name in PORTABLE_MARKER_NAMES {
            let p = app_dir.join(name);
            if p.is_file() {
                fs::remove_file(&p).map_err(|e| {
                    EasyMcError::msg(format!(
                        "无法删除便携标记 {}：{e}。可手动删除后重启。",
                        p.display()
                    ))
                })?;
            }
        }
    }
    // Status against whatever discover would pick after a restart (best-effort active).
    let preview = Paths::discover_with(Some(app_dir.clone()))?;
    Ok(portable_status(&preview.root))
}

/// Safe copy of data from the OS-standard directory into the portable `data/` folder.
/// Never deletes the source. Refuses if env override is set or paths collide.
pub fn migrate_standard_to_portable() -> Result<MigrateResult> {
    if env_data_dir().is_some() {
        return Err(EasyMcError::msg(
            "已设置 EASYMC_DATA_DIR，便携迁移不适用。请先取消该环境变量。",
        ));
    }
    let app_dir = install_dir().ok_or_else(|| EasyMcError::msg("无法定位程序目录"))?;
    let src = standard_data_dir()?;
    let dst = portable_data_dir(&app_dir);
    if src == dst {
        return Err(EasyMcError::msg("源与目标相同，无需迁移。"));
    }
    if !dir_looks_populated(&src) {
        return Ok(MigrateResult {
            ok: true,
            copied_entries: 0,
            source: src.clone(),
            destination: dst,
            message_zh: format!("系统目录没有可迁移的数据（{}）。", src.display()),
        });
    }
    fs::create_dir_all(&dst)?;
    let copied = copy_dir_merge(&src, &dst)?;
    Ok(MigrateResult {
        ok: true,
        copied_entries: copied,
        source: src.clone(),
        destination: dst.clone(),
        message_zh: format!(
            "已把 {copied} 项从系统目录复制到便携目录（未删除原文件）。源：{} → 目标：{}。请确认便携模式已开启并重启 EasyMC。",
            src.display(),
            dst.display()
        ),
    })
}

/// Recursively copy `src` into `dst`, skipping files that already exist with the same size.
fn copy_dir_merge(src: &Path, dst: &Path) -> Result<u64> {
    let mut count = 0u64;
    fs::create_dir_all(dst)?;
    for ent in fs::read_dir(src)? {
        let ent = ent?;
        let from = ent.path();
        let to = dst.join(ent.file_name());
        let ft = ent.file_type()?;
        if ft.is_dir() {
            count += copy_dir_merge(&from, &to)?;
        } else if ft.is_file() {
            let skip = to.is_file()
                && fs::metadata(&to).ok().map(|m| m.len())
                    == fs::metadata(&from).ok().map(|m| m.len());
            if !skip {
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&from, &to)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod portable_tests {
    use super::*;
    use std::sync::Mutex;

    // Env mutations must be serialized across tests in this process.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_app() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "easymc-portable-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discover_uses_env_over_marker() {
        let _g = ENV_LOCK.lock().unwrap();
        let app = temp_app();
        fs::write(app.join("portable.txt"), "x").unwrap();
        let data = app.join("env-data");
        fs::create_dir_all(&data).unwrap();
        std::env::set_var("EASYMC_DATA_DIR", &data);
        let paths = Paths::discover_with(Some(app.clone())).unwrap();
        assert_eq!(paths.root, data);
        std::env::remove_var("EASYMC_DATA_DIR");
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn discover_portable_marker_txt() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("EASYMC_DATA_DIR");
        let app = temp_app();
        fs::write(app.join("portable.txt"), "# portable\n").unwrap();
        let paths = Paths::discover_with(Some(app.clone())).unwrap();
        assert_eq!(paths.root, app.join("data"));
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn discover_portable_marker_easymc_portable() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("EASYMC_DATA_DIR");
        let app = temp_app();
        fs::write(app.join("easymc-portable"), "").unwrap();
        let paths = Paths::discover_with(Some(app.clone())).unwrap();
        assert_eq!(paths.root, portable_data_dir(&app));
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn discover_without_marker_uses_standard_layout_shape() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("EASYMC_DATA_DIR");
        let app = temp_app();
        // No marker → not portable; root should equal standard_data_dir (platform).
        let paths = Paths::discover_with(Some(app.clone())).unwrap();
        let std_dir = standard_data_dir().unwrap();
        assert_eq!(paths.root, std_dir);
        assert!(!paths.root.starts_with(&app));
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn marker_detection_helpers() {
        let app = temp_app();
        assert!(portable_marker_in(&app).is_none());
        fs::write(app.join("easymc-portable"), "1").unwrap();
        assert_eq!(
            portable_marker_in(&app).unwrap().file_name().unwrap(),
            "easymc-portable"
        );
        // portable.txt is preferred when both exist (first in list).
        fs::write(app.join("portable.txt"), "1").unwrap();
        assert_eq!(
            portable_marker_in(&app).unwrap().file_name().unwrap(),
            "portable.txt"
        );
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn migrate_copies_without_deleting_source() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("EASYMC_DATA_DIR");

        // Use a fake "standard" by copying into a sandbox then invoking merge directly.
        let sandbox = temp_app();
        let src = sandbox.join("EasyMC");
        let dst = sandbox.join("data");
        fs::create_dir_all(src.join("instances")).unwrap();
        fs::write(src.join("settings.json"), "{\"memory_mb\":2048}").unwrap();
        fs::write(src.join("instances").join("a.txt"), "hello").unwrap();

        let n = copy_dir_merge(&src, &dst).unwrap();
        assert!(n >= 2);
        assert!(dst.join("settings.json").is_file());
        assert!(dst.join("instances").join("a.txt").is_file());
        // Source intact.
        assert!(src.join("settings.json").is_file());
        assert_eq!(
            fs::read_to_string(src.join("instances").join("a.txt")).unwrap(),
            "hello"
        );
        // Second merge is idempotent (same sizes → skip).
        let n2 = copy_dir_merge(&src, &dst).unwrap();
        assert_eq!(n2, 0);

        let _ = fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn dir_looks_populated_detects_settings() {
        let dir = temp_app();
        assert!(!dir_looks_populated(&dir));
        fs::write(dir.join("settings.json"), "{}").unwrap();
        assert!(dir_looks_populated(&dir));
        let _ = fs::remove_dir_all(&dir);
    }
}
