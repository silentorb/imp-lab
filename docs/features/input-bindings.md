# Input bindings

## Summary

Imp Lab maps user keyboard input to **commands** through a configurable bindings layer (ported from the deprecated Iced Aura Editor schema). Native menus **derive accelerators** from the same table so the OS can show the right-aligned shortcut column (VS Code–style menubar + keybinding ownership).

## Ownership (VS Code–aligned)

- The **bindings table** is the semantic owner of chords (bundled defaults + optional user JSON).
- **Webview `keydown`** resolves the table and calls `dispatchCommand`.
- **Menu clicks** emit `editor://command` by command id (unchanged).
- **Menu accelerators** are set from the first binding for each menu command id so shortcuts appear natively. Until muda/Tauri gain Electron-parity hooks (see [Upstream / later](#upstream--later)), accelerator firings also arrive as menu events → the same command path.

Menus are a view/delivery surface over the bindings table—not a second competing keymap design.

## Persistence

- Load chain: optional **user** `bindings.json` **or** bundled default **or** empty (**replace, not merge**).
- User path: platform config dir for project `com` / `Silent Orb` / `imp-lab` → `bindings.json` (via `directories`).
- Bundled file: `apps/tauri/src-tauri/assets/default-bindings.json`.
- Unknown command ids (not in builtin host ids ∪ loaded extension `command` ids) are skipped.
- Unsupported `version` ≠ 1 fails that file; the load chain continues.
- No bindings settings UI; users may hand-edit the JSON. Writing the file from the app is out of scope.
- The same config dir also holds [UI-state buckets](user-state.md) (`documents.json`, `ui.json`, `window.json`); those are a separate concern and do not merge with bindings.

## Schema (version 1)

Same shape as the Iced Aura Editor design:

```json
{
  "version": 1,
  "bindings": [
    {
      "command": "document.save",
      "when": { "type": "key", "key": "s", "modifiers": ["ctrl"] }
    }
  ]
}
```

- Key triggers: `type` `key`, `key` (character or named: `space`, `escape`, …), `modifiers` subset of `ctrl`, `alt`, `shift`, `logo`.
- Exact modifier match on resolve. JSON `ctrl` matches Ctrl **or** Meta (Cmd) in the webview so macOS Cmd+S works with `CmdOrCtrl` menu accelerators.
- Menu accelerator formatting maps JSON `ctrl` → Tauri `CmdOrCtrl`.
- Last duplicate trigger wins; multiple bindings may target one command.

## Defaults

| Trigger | Command |
| --- | --- |
| Ctrl/Cmd+O | `project.open` |
| Ctrl/Cmd+P | `records.focus-filter` (Record Explorer filter) |
| Ctrl/Cmd+W | `document.close_active` |
| Ctrl/Cmd+N | `document.new.notation` |
| Ctrl/Cmd+S | `document.save` |
| Ctrl/Cmd+Z | `edit.undo` |
| Ctrl/Cmd+Shift+Z | `edit.redo` |
| D / S / E / N | notation tool draw / select / erase / snap toggle |
| Space | `play.toggle` (only when the Aura playback pack contributes that command) |
| R | `notation.record.toggle` (start / stop a take; shown when a capture service is present) |
| Ctrl/Cmd+, | `settings.open` (Edit → Settings…) |

## Runtime

- Rust: `apps/tauri/src-tauri/src/bindings.rs` (load, accelerator strings, `get_input_bindings`).
- React: `apps/tauri/src/bindings.ts` (`resolveKeyEvent`); `App.tsx` keydown listener.
- Focused text fields skip keydown resolve so browser typing / undo win; menu-delivered Undo/Redo still go through `dispatchCommand`’s text-editing branch.

## Non-goals

- Bindings settings UI / in-app write of the user file
- Mouse / pointer triggers
- Entry-by-entry merge of user + default tables
- Full VS Code `when`-clause engine
- Claiming OS clipboard / window-manager shortcuts as editor commands

## Upstream / later

Ideal delivery matches **VS Code on Electron**:

- Win/Linux: `registerAccelerator: false` — show the accelerator without OS registration so the window keybinding path owns delivery.
- Menu events: `triggered_by_accelerator` — replay the chord into the bindings resolver (especially macOS) instead of a blind command run.

**muda/Tauri do not expose these yet:**

- [muda#393](https://github.com/tauri-apps/muda/issues/393)
- [tauri#15909](https://github.com/tauri-apps/tauri/issues/15909)

When they land, adopt them without changing the bindings file model. Do **not** redesign around permanent label-suffix “fake” accelerators.

This slice is a **compromise constrained by that gap**—real native accelerators + keydown resolve—not the finished Electron-parity form, and not a hacky alternative architecture.
