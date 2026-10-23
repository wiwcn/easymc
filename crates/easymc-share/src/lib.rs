//! EasyMC instance share: export / import metadata JSON or pack zip.
//!
//! Format id: `easymc.share` v1 — see README.

mod export;
mod format;
mod import;
mod index;

pub use export::{
    build_manifest, ensure_instance, export_instance, path_for_clipboard, shares_dir, ExportOptions,
    ExportResult,
};
pub use format::{
    InstanceSpec, JvmSpec, ModEntry, ShareManifest, ShareMode, FORMAT_ID, FORMAT_VERSION,
    MANIFEST_NAME,
};
pub use import::{import_instance, read_manifest_from_path, ImportOptions, ImportResult};
pub use index::{index_path, load_index, save_index, upsert_index_entry, ModsIndex};
