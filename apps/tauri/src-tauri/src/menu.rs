//! Native application menu bar (File / Edit / View).
//!
//! Clicks emit `editor://command` for the React shell to handle.

use tauri::menu::{Menu, MenuBuilder, SubmenuBuilder};
use tauri::{AppHandle, Runtime};

const CMD_DOCUMENT_OPEN_FILE: &str = "document.open-file";
const CMD_DOCUMENT_NEW_GRAPH: &str = "document.new.graph";
const CMD_DOCUMENT_SAVE: &str = "document.save";
const CMD_DOCUMENT_CLOSE_ACTIVE: &str = "document.close_active";
const CMD_VIEW_TOGGLE_COLOR_SCHEME: &str = "view.toggle-color-scheme";

/// Builds the global File / Edit / View menu bar.
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let file = SubmenuBuilder::new(app, "File")
        .text(CMD_DOCUMENT_OPEN_FILE, "Open File…")
        .text(CMD_DOCUMENT_NEW_GRAPH, "New graph")
        .text(CMD_DOCUMENT_SAVE, "Save")
        .text(CMD_DOCUMENT_CLOSE_ACTIVE, "Close")
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit").build()?;

    let view = SubmenuBuilder::new(app, "View")
        .text(CMD_VIEW_TOGGLE_COLOR_SCHEME, "Toggle light/dark")
        .build()?;

    MenuBuilder::new(app)
        .item(&file)
        .item(&edit)
        .item(&view)
        .build()
}
