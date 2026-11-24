//! Load optional Lab extensions and expose catalogs / commands to the React host.

use imp_lab_extensions::{
    load_extensions_default, load_extensions_from_path, settings_layout, status_items, topic_hub,
    ExtensionsLoadResult, ExtensionsRuntime, SettingsLayout, StatusItemDecl,
};
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
