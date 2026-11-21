//! Load optional Lab extensions and expose catalogs / commands to the React host.

use imp_lab_extensions::{
    load_extensions_default, load_extensions_from_path, ExtensionsLoadResult, ExtensionsRuntime,
};
use std::path::PathBuf;
use std::sync::OnceLock;

static EXTENSIONS: OnceLock<ExtensionsRuntime> = OnceLock::new();

fn cached_runtime() -> &'static ExtensionsRuntime {
    EXTENSIONS.get_or_init(load_extensions_default)
}

/// Return the loaded extensions / catalogs snapshot (loads once per process).
#[tauri::command]
pub fn get_extensions_load() -> ExtensionsLoadResult {
    cached_runtime().result.clone()
}

/// Test/tooling: load extensions from an explicit config path (does not replace cache).
#[tauri::command]
pub fn load_extensions_file(path: String) -> ExtensionsLoadResult {
    load_extensions_from_path(&PathBuf::from(path)).result
}

/// Run an extension-contributed command against a registered service.
#[tauri::command]
pub fn run_extension_command(
    id: String,
    graph_json: Option<String>,
    neighbor_path: Option<String>,
) -> Result<Option<bool>, String> {
    cached_runtime()
        .run_command(
            &id,
            graph_json.as_deref(),
            neighbor_path.as_deref(),
        )
        .map_err(|e| e.to_string())
}

/// Ensure extensions are loaded (call during app setup before building menus).
pub fn ensure_loaded() -> &'static ExtensionsRuntime {
    cached_runtime()
}
