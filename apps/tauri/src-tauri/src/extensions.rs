//! Load optional Lab extensions and expose catalogs to the React host.

use imp_lab_extensions::{load_extensions_default, load_extensions_from_path, ExtensionsLoadResult};
use std::path::PathBuf;
use std::sync::OnceLock;

static EXTENSIONS: OnceLock<ExtensionsLoadResult> = OnceLock::new();

fn cached_extensions() -> &'static ExtensionsLoadResult {
    EXTENSIONS.get_or_init(load_extensions_default)
}

/// Return the loaded extensions / catalogs snapshot (loads once per process).
#[tauri::command]
pub fn get_extensions_load() -> ExtensionsLoadResult {
    cached_extensions().clone()
}

/// Test/tooling: load extensions from an explicit config path (does not replace cache).
#[tauri::command]
pub fn load_extensions_file(path: String) -> ExtensionsLoadResult {
    load_extensions_from_path(&PathBuf::from(path))
}
