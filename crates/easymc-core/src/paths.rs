use std::path::{Path, PathBuf};

use crate::{EasyMcError, Result};

/// Root data directory for EasyMC (versions, libraries, assets, instances, runtimes).
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Prefer `EASYMC_DATA_DIR`, else platform data dir / EasyMC.
    pub fn discover() -> Result<Self> {
        if let Ok(p) = std::env::var("EASYMC_DATA_DIR") {
            return Ok(Self::from_root(p));
        }
        let base = dirs::data_dir().ok_or_else(|| EasyMcError::msg("no data_dir"))?;
        Ok(Self::from_root(base.join("EasyMC")))
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
