mod bindings;
mod extensions;
mod lab_model;
mod menu;
mod project;
mod records;
pub mod seed;
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
        .manage(project::ProjectState::default())
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
            project::spawn_change_poller(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            user_state::on_window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            project::project_current,
            project::project_open,
            project::project_pick_and_open,
            project::project_pick_and_create,
            project::project_close,
            project::project_pick_store_folder,
            project::project_add_store,
            project::project_remove_store,
            records::records_list,
            records::record_open,
            records::record_create,
            records::record_save,
            records::record_rename,
            records::record_archive,
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
