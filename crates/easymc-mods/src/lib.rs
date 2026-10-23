//! Modrinth + CurseForge search and install into an instance `mods/` folder.

mod curseforge;
mod http;
mod install;
mod modrinth;
mod types;

pub use curseforge::{api_key_from_env, require_api_key, API_KEY_ENV};
pub use install::{
    install_mod, list_installed_mods, list_mod_versions, mods_dir, search_mods,
};
pub use types::*;

pub use modrinth::HashLookup;

pub async fn lookup_modrinth_by_sha1(sha1_hex: &str) -> easymc_core::Result<Option<HashLookup>> {
    modrinth::lookup_by_sha1(sha1_hex).await
}

