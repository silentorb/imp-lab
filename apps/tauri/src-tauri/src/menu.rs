//! Native application menu bar (File / Edit / View + extension contributions).
//!
//! Clicks emit `editor://command` for the React shell to handle.

use crate::extensions;
use imp_lab_extensions::menu_contributions_from_config;
use std::collections::BTreeMap;
use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Runtime};

const CMD_DOCUMENT_OPEN_FILE: &str = "document.open-file";
const CMD_DOCUMENT_NEW_GRAPH: &str = "document.new.graph";
const CMD_DOCUMENT_NEW_NOTATION: &str = "document.new.notation";
const CMD_DOCUMENT_SAVE: &str = "document.save";
const CMD_DOCUMENT_SAVE_AS: &str = "document.save-as";
const CMD_DOCUMENT_CLOSE_ACTIVE: &str = "document.close_active";
const CMD_EDIT_UNDO: &str = "edit.undo";
const CMD_EDIT_REDO: &str = "edit.redo";
const CMD_VIEW_TOGGLE_COLOR_SCHEME: &str = "view.toggle-color-scheme";

/// Builds the global File / Edit / View menu bar, plus extension submenus.
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let file = SubmenuBuilder::new(app, "File")
        .text(CMD_DOCUMENT_OPEN_FILE, "Open File…")
        .text(CMD_DOCUMENT_NEW_GRAPH, "New graph")
        .text(CMD_DOCUMENT_NEW_NOTATION, "New notation")
        .text(CMD_DOCUMENT_SAVE, "Save")
        .text(CMD_DOCUMENT_SAVE_AS, "Save As…")
        .text(CMD_DOCUMENT_CLOSE_ACTIVE, "Close")
        .build()?;

    let undo = MenuItemBuilder::with_id(CMD_EDIT_UNDO, "Undo")
        .accelerator("CmdOrCtrl+Z")
        .build(app)?;
    let redo = MenuItemBuilder::with_id(CMD_EDIT_REDO, "Redo")
        .accelerator("CmdOrCtrl+Shift+Z")
        .build(app)?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&undo)
        .item(&redo)
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .text(CMD_VIEW_TOGGLE_COLOR_SCHEME, "Toggle light/dark")
        .build()?;

    let mut builder = MenuBuilder::new(app)
        .item(&file)
        .item(&edit)
        .item(&view);

    let runtime = extensions::ensure_loaded();
    if let Some(config) = runtime.config.as_ref() {
        let items = menu_contributions_from_config(config);
        let mut by_menu: BTreeMap<&str, Vec<_>> = BTreeMap::new();
        for item in &items {
            by_menu.entry(item.menu.as_str()).or_default().push(item);
        }
        for (menu_title, menu_items) in by_menu {
            let mut sub = SubmenuBuilder::new(app, menu_title);
            for item in menu_items {
                sub = sub.text(&item.command_id, &item.label);
            }
            let submenu = sub.build()?;
            builder = builder.item(&submenu);
        }
    }

    builder.build()
}
