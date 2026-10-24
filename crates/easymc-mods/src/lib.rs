//! Modrinth + CurseForge search and install into an instance `mods/` folder.

mod content;
mod curseforge;
mod modpack;
mod http;
mod install;
mod modrinth;
mod types;

pub use curseforge::{
    api_key, api_key_from_env, api_key_source, require_api_key, set_stored_api_key,
    stored_api_key, sync_api_key_from_settings, ApiKeySource, API_KEY_ENV, CONSOLE_URL,
};
pub use install::{
    install_mod, list_installed_mods, list_mod_versions, mods_dir, remove_mod,
    sanitize_mod_filename, score_mod_match, search_and_install_best, search_mods, set_mod_disabled,
};
pub use types::*;
pub use content::{
    content_dest, delete_local, install_content, list_local, local_dir, ContentInstallResult,
    LocalEntry,
};
pub use modpack::{install_modpack, install_mrpack_file, ModpackInstallResult};

pub use modrinth::HashLookup;

pub async fn lookup_modrinth_by_sha1(sha1_hex: &str) -> easymc_core::Result<Option<HashLookup>> {
    modrinth::lookup_by_sha1(sha1_hex).await
}

