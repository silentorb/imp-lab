//! Native application menu bar (File / Edit / View + extension contributions).
//!
//! Clicks emit `editor://command` for the React shell to handle.
//! Accelerators are derived from the host bindings table (VS Code–style).

use crate::bindings::{self, Bindings};
use crate::extensions;
use imp_lab_extensions::menu_contributions_from_config;
use std::collections::BTreeMap;
use tauri::menu::{Menu, MenuBuilder, MenuItem, MenuItemBuilder, SubmenuBuilder};
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
    let runtime = extensions::ensure_loaded();
    let bindings = bindings::ensure_loaded(&runtime.result.contributions.command_ids);

    let open = item(app, CMD_DOCUMENT_OPEN_FILE, "Open File…", bindings)?;
    let new_graph = item(app, CMD_DOCUMENT_NEW_GRAPH, "New graph", bindings)?;
    let new_notation = item(app, CMD_DOCUMENT_NEW_NOTATION, "New notation", bindings)?;
    let save = item(app, CMD_DOCUMENT_SAVE, "Save", bindings)?;
    let save_as = item(app, CMD_DOCUMENT_SAVE_AS, "Save As…", bindings)?;
    let close = item(app, CMD_DOCUMENT_CLOSE_ACTIVE, "Close", bindings)?;
    let file = SubmenuBuilder::new(app, "File")
        .item(&open)
        .item(&new_graph)
        .item(&new_notation)
        .item(&save)
        .item(&save_as)
        .item(&close)
        .build()?;

    let undo = item(app, CMD_EDIT_UNDO, "Undo", bindings)?;
    let redo = item(app, CMD_EDIT_REDO, "Redo", bindings)?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&undo)
        .item(&redo)
        .build()?;

    let toggle_theme = item(app, CMD_VIEW_TOGGLE_COLOR_SCHEME, "Toggle light/dark", bindings)?;
    let view = SubmenuBuilder::new(app, "View")
        .item(&toggle_theme)
        .build()?;

    let mut builder = MenuBuilder::new(app)
        .item(&file)
        .item(&edit)
        .item(&view);

    if let Some(config) = runtime.config.as_ref() {
        let items = menu_contributions_from_config(config);
        let mut by_menu: BTreeMap<&str, Vec<_>> = BTreeMap::new();
        for contrib in &items {
            by_menu.entry(contrib.menu.as_str()).or_default().push(contrib);
        }
        for (menu_title, menu_items) in by_menu {
            let mut sub = SubmenuBuilder::new(app, menu_title);
            for contrib in menu_items {
                let menu_item =
                    item(app, &contrib.command_id, &contrib.label, bindings)?;
                sub = sub.item(&menu_item);
            }
            let submenu = sub.build()?;
            builder = builder.item(&submenu);
        }
    }

    builder.build()
}

fn item<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    label: &str,
    bindings: &Bindings,
) -> tauri::Result<MenuItem<R>> {
    let mut builder = MenuItemBuilder::with_id(id, label);
    if let Some(accel) = bindings.accelerator_for_command(id) {
        builder = builder.accelerator(accel);
    }
    builder.build(app)
}
