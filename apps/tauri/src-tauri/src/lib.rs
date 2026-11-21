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
    // Load optional pack extensions once at startup (logs warnings to stderr).
    let loaded = extensions::get_extensions_load();
    for warning in &loaded.warnings {
        eprintln!("imp-lab extensions: {warning}");
    }
    if let Some(root) = &loaded.pack_root {
        eprintln!(
            "imp-lab extensions: loaded pack root {root} ({} node libraries)",
            loaded.catalogs.node_libraries.len()
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
            extensions::load_extensions_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
