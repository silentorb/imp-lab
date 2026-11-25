//! Load optional Lab extensions and expose catalogs / commands to the React host.

use imp_lab_extensions::{
    binding_decls, load_extensions_default, load_extensions_from_path, node_type_editors, node_ui,
    settings_layout, status_items, topic_hub, BindingDecl, ExtensionsLoadResult,
    ExtensionsRuntime, NodeTypeEditorDecl, NodeUi, SettingsLayout, StatusItemDecl,
};

use crate::lab_model::{self, NodeType};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use tauri::{AppHandle, Emitter, Runtime};
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
        .run_command(&id, graph_json.as_deref(), neighbor_path.as_deref())
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatusDto {
    pub playing: bool,
    pub position_secs: f64,
}

/// Query the Aura Lab playback service clock (when the pack is loaded).
#[tauri::command]
pub fn get_playback_status() -> Result<PlaybackStatusDto, String> {
    let (playing, position_secs) = cached_runtime()
        .playback_status("playback")
        .map_err(|e| e.to_string())?;
    Ok(PlaybackStatusDto {
        playing,
        position_secs,
    })
}

/// Deliver a service-defined JSON event (for example a live note on / off for the playback
/// service) to a registered service implementation id. Lab does not interpret the event.
#[tauri::command]
pub fn send_service_event(service_id: String, event_json: String) -> Result<(), String> {
    cached_runtime()
        .send_event(&service_id, &event_json)
        .map_err(|e| e.to_string())
}

/// Ensure extensions are loaded (call during app setup before building menus).
pub fn ensure_loaded() -> &'static ExtensionsRuntime {
    cached_runtime()
}

/// Send a service-defined JSON request and return the service's JSON reply. Lab does not
/// interpret either.
#[tauri::command]
pub fn service_request(service_id: String, request_json: String) -> Result<String, String> {
    cached_runtime()
        .request(&service_id, &request_json)
        .map_err(|e| e.to_string())
}

/// Latest value of every topic extensions have published (the webview's starting snapshot;
/// later changes arrive as [`TOPIC_EVENT`] events).
#[tauri::command]
pub fn get_topics() -> BTreeMap<String, Value> {
    topic_hub().snapshot()
}

/// Settings pages built from the enabled `setting` / `settings-category` /
/// `settings-category-item` components.
#[tauri::command]
pub fn get_settings_layout() -> SettingsLayout {
    cached_runtime()
        .config
        .as_ref()
        .map(settings_layout)
        .unwrap_or_default()
}

/// Enabled `status-item` components.
#[tauri::command]
pub fn get_status_items() -> Vec<StatusItemDecl> {
    cached_runtime()
        .config
        .as_ref()
        .map(status_items)
        .unwrap_or_default()
}

/// Enabled `node-literal-options` and `node-action` components.
#[tauri::command]
pub fn get_node_ui() -> NodeUi {
    cached_runtime()
        .config
        .as_ref()
        .map(node_ui)
        .unwrap_or_default()
}

/// Webview event carrying one topic change: `{topic, value}`.
pub const TOPIC_EVENT: &str = "extension://topic";

#[derive(Clone, Serialize)]
struct TopicPayload {
    topic: String,
    value: Value,
}

/// Forward every published topic change to the webview.
pub fn install_topic_sink<R: Runtime>(app: AppHandle<R>) {
    topic_hub().set_sink(move |topic, value| {
        let payload = TopicPayload {
            topic: topic.to_string(),
            value: value.clone(),
        };
        if let Err(err) = app.emit(TOPIC_EVENT, payload) {
            eprintln!("failed to emit {TOPIC_EVENT}: {err}");
        }
    });
}

/// Command id that opens a new, unsaved record in the editor component `editor_id`.
pub fn new_record_command(editor_id: &str) -> String {
    format!("document.new:{editor_id}")
}

/// Enabled `node-type-editor` components (warnings go to stderr once per call site).
pub fn editors() -> Vec<NodeTypeEditorDecl> {
    cached_runtime()
        .config
        .as_ref()
        .map(|c| node_type_editors(c).0)
        .unwrap_or_default()
}

/// Enabled `binding` components.
pub fn bindings() -> Vec<BindingDecl> {
    cached_runtime()
        .config
        .as_ref()
        .map(|c| binding_decls(c).0)
        .unwrap_or_default()
}

/// Node types Lab can open: Imp graphs plus every extension editor's node type.
pub fn node_types() -> Vec<NodeType> {
    lab_model::node_types_with(&editors())
}

/// An extension's webview UI library.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiLibraryDto {
    pub extension_id: String,
}

/// UI components for the webview: libraries to load and the editors they implement.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiExtensionsDto {
    pub libraries: Vec<UiLibraryDto>,
    pub editors: Vec<NodeTypeEditorDecl>,
    pub warnings: Vec<String>,
}

/// Editors and the UI libraries that implement them.
#[tauri::command]
pub fn get_ui_extensions() -> UiExtensionsDto {
    let Some(config) = cached_runtime().config.as_ref() else {
        return UiExtensionsDto::default();
    };
    let (editors, mut warnings) = node_type_editors(config);
    warnings.extend(binding_decls(config).1);
    let libraries = config
        .extensions
        .iter()
        .filter(|e| e.enabled && editors.iter().any(|ed| ed.extension_id == e.id))
        .map(|e| UiLibraryDto {
            extension_id: e.id.clone(),
        })
        .collect();
    UiExtensionsDto {
        libraries,
        editors,
        warnings,
    }
}

/// Source of an extension's UI library (an ES module), read from the pack. The path must stay
/// inside the pack root.
#[tauri::command]
pub fn read_ui_library(extension_id: String) -> Result<String, String> {
    let runtime = cached_runtime();
    let config = runtime.config.as_ref().ok_or("no_extensions: no pack loaded")?;
    let extension = config
        .extensions
        .iter()
        .find(|e| e.id == extension_id && e.enabled)
        .ok_or_else(|| format!("not_found: extension {extension_id}"))?;
    let relative = extension
        .ui_library_path
        .as_deref()
        .ok_or_else(|| format!("no_ui_library: extension {extension_id} has no uiLibraryPath"))?;
    let root = runtime
        .result
        .pack_root
        .as_deref()
        .map(PathBuf::from)
        .ok_or("no_extensions: no pack root")?;
    let path = ui_library_path(&root, relative)?;
    std::fs::read_to_string(&path).map_err(|e| format!("io: {}: {e}", path.display()))
}

fn ui_library_path(root: &std::path::Path, relative: &str) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("io: {}: {e}", root.display()))?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("io: {relative}: {e}"))?;
    if !path.starts_with(&root) {
        return Err(format!("invalid_path: {relative} is outside the pack"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_library_paths_stay_inside_the_pack() {
        let tmp = tempfile::tempdir().unwrap();
        let pack = tmp.path().join("pack");
        std::fs::create_dir_all(pack.join("extensions")).unwrap();
        std::fs::write(pack.join("extensions/ui.js"), "export const implementations = {};").unwrap();
        std::fs::write(tmp.path().join("secret.js"), "").unwrap();
        assert!(ui_library_path(&pack, "extensions/ui.js").is_ok());
        assert!(ui_library_path(&pack, "../secret.js")
            .unwrap_err()
            .starts_with("invalid_path"));
        assert!(ui_library_path(&pack, "extensions/missing.js").is_err());
    }
}
