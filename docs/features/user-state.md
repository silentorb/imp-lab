# UI state persistence

## Summary

Imp Lab persists **editor UI chrome** across launches: open saved tabs, explorer prefs, extension UI components' own prefs, and main-window geometry. This is ordinary durable local UI state — not document content, not a start/finish “session.” It also stores extension setting values for the [Settings](settings.md) dialog.

## Location

- Platform config dir: `directories::ProjectDirs::from("com", "Silent Orb", "imp-lab")` → `config_dir/`
- Same root as optional hand-edited [`bindings.json`](input-bindings.md); UI state uses separate bucket files
- Override for tests/tooling: env `IMP_USER_DIR` (absolute path)
- Directory is created on first successful write

## Buckets

Each bucket is `{config_dir}/{bucket}.json` with envelope:

```json
{
  "version": 1,
  "entries": {
    "<entry-id>": { }
  }
}
```

| Entry id | Bucket file | Shape |
| --- | --- | --- |
| `open-documents` | `documents.json` | optional `projectPath`, `openNodeIds` (string[]), optional `activeNodeId` |
| `component:<componentId>` | `ui.json` | an extension UI component's own prefs (an editor's tool settings, say), opaque to Lab |
| `extension-settings` | `settings.json` | `{settingId: value}`: extension setting values, opaque to Lab ([settings](settings.md)) |
| `records-ui` | `ui.json` | optional `lastNewRecordStore`, optional `explorerNodeType` (a node type id, `openable` or `all`) |
| `window` | `window.json` | `width`, `height`, optional `x`/`y`, `maximized` |

Missing file, missing entry, unsupported `version`, or invalid JSON → defaults for that bucket/entry; the app still starts (with logging).

### Open documents

- The open project's config path plus the node ids of open record tabs; unsaved tabs are omitted (see [projects and records](projects-and-records.md))
- Collect after open / activate / close / first save / project switch
- Apply once at startup: reopen `projectPath` via `project_open`, reopen each node id via `record_open`, then select `activeNodeId` when still open
- Records that fail to open are skipped with a warning
- Entries from before projects (`openPaths` / `activePath`) are ignored; old file paths are not reopened

### Records UI

- `lastNewRecordStore`: default store in the "Save new record" dialog (falls back to the primary store when it is gone or read-only)
- `explorerNodeType`: the Record Explorer's node-type filter (a saved type with no editor any more falls back to `openable`)

### Component prefs

- One entry per extension UI component (`component:` + component id), read and written by the component through `EditorHost.loadPrefs` / `savePrefs`; Lab doesn't interpret it
- Prefs that are not per document (shared by every open document of that editor)

### Window

- Outer size and position in physical pixels, plus maximized
- Captured on move/resize from the Rust host; applied in `.setup` before the UI restores tabs
- Multi-monitor / DPI edge cases are best-effort

## Mechanics

- Updates apply in memory immediately and mark the bucket dirty
- Disk writes are **debounced** (500 ms idle) with a generation token so bursts coalesce
- Writes use **atomic** temp sibling + rename
- **Flush on window close** so the last geometry/tabs are not lost if exit beats the debounce
- Do not serialize React widget trees; only the purpose-built entry shapes above

## Runtime

- Rust: `apps/tauri/src-tauri/src/user_state.rs` (store, window capture/apply, commands)
- React: `apps/tauri/src/userState.ts` + restore/sync in `App.tsx`
- Commands: `get_user_state_entry`, `set_user_state_entry`, `flush_user_state`

## Relationship to bindings

Keyboard bindings remain a separate replace-loaded `bindings.json` in the same config dir. UI-state buckets do not merge with or rewrite bindings.

## Color scheme

Mantine color scheme stays on webview `localStorage` (`imp-lab-color-scheme`) for this slice.

## Non-goals

- General app/settings configuration or layered install/project/user config
- Shell pane trees, pan/zoom camera, selection, recent-files list
- Graph connection-mode / add-node-type prefs (not in the desktop UI yet)
- Extension-defined buckets (components get one opaque `component:` entry each)
- In-app settings UI for these entries
- Persisting untitled dirty buffers
