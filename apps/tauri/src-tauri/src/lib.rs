mod bindings;
mod extensions;
mod menu;
mod open;
mod save;
mod user_state;

use std::sync::Mutex;

use serde::Serialize;
use tauri::{Emitter, Manager};

use user_state::{UserStateStore, WindowEntry, BUCKET_WINDOW, ENTRY_WINDOW};

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
    // Bindings after extensions so extension command ids are known.
    let _ = bindings::ensure_loaded(&loaded.result.contributions.command_ids);

    let store = UserStateStore::open_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(store))
        .menu(menu::build)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            if let Err(err) = app.emit("editor://command", CommandPayload { id }) {
                eprintln!("failed to emit editor://command: {err}");
            }
        })
        .setup(|app| {
            let entry = {
                let state = app.state::<Mutex<UserStateStore>>();
                let Ok(guard) = state.lock() else {
                    return Ok(());
                };
                guard.get_entry::<WindowEntry>(BUCKET_WINDOW, ENTRY_WINDOW)
            };
            if let Some(entry) = entry {
                user_state::apply_window_entry(app.handle(), &entry);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            user_state::on_window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            open::pick_and_open_document,
            open::open_document_file,
            save::save_document_file,
            save::pick_and_save_document,
            bindings::get_input_bindings,
            extensions::get_extensions_load,
            extensions::load_extensions_file,
            extensions::run_extension_command,
            extensions::get_playback_status,
            user_state::get_user_state_entry,
            user_state::set_user_state_entry,
            user_state::flush_user_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
