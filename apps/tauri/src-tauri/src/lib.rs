mod extensions;
mod menu;
mod open;
mod save;

use serde::Serialize;
use tauri::Emitter;

#[derive(Clone, Serialize)]
struct CommandPayload {
    id: String,
}

#[tauri::command]
fn ping() -> String {
    "pong".into()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load optional pack extensions once at startup (before menu build).
    let loaded = extensions::ensure_loaded();
    for warning in &loaded.result.warnings {
        eprintln!("imp-lab extensions: {warning}");
    }
    if let Some(root) = &loaded.result.pack_root {
        eprintln!(
            "imp-lab extensions: loaded pack root {root} ({} node libraries, {} services, {} commands)",
            loaded.result.catalogs.node_libraries.len(),
            loaded
                .services
                .lock()
                .map(|s| s.len())
                .unwrap_or(0),
            loaded.result.contributions.command_ids.len()
        );
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(menu::build)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            if let Err(err) = app.emit("editor://command", CommandPayload { id }) {
                eprintln!("failed to emit editor://command: {err}");
            }
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            open::pick_and_open_document,
            open::open_document_file,
            save::save_document_file,
            save::pick_and_save_document,
            extensions::get_extensions_load,
            extensions::load_extensions_file,
            extensions::run_extension_command,
            extensions::get_playback_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
