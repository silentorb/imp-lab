# Imp Lab — Agent Instructions

## Repository purpose

**imp-lab** manages **scaling networks of Imp graphs**. Language-neutral Imp specs live in **[imp-spec](https://github.com/silentorb/imp-spec)**; the Rust Imp core lives in **[imp-rs](https://github.com/silentorb/imp-rs)**.

Imp Lab is **domain-neutral**. It must not depend on Aura (`aura_*`) crates or packages, and it must not contain product-domain concepts — no audio, music, notation, piano roll, audition or recording UI, even behind generic names. Product hosts add their domain through extensions: components in `extensions.json` that choose implementations from extension libraries. Aura's audio-specific UI belongs in **[aura-editor](https://github.com/silentorb/aura-editor)**, not here.

There are no document types. Everything Lab opens is a Tome node; what it is comes from its **Tome node type** (type-table membership). Imp graphs are the one built-in node type; extensions add editors for other node types (`node-type-editor` components).

| Path | Role |
| --- | --- |
| `crates/imp_lab_extensions/` | Tome-shaped optional `extensions.json` parse + dynamically linked extension loading; component declarations (`imp-catalog`, `command`, `menu-item`, `service`, settings, `status-item`, node UI, `node-type-editor`, `binding`) |
| `packages/imp-lab-ui/` | Contract with extension UI libraries (`uiLibraryPath`): implementation types (`NodeTypeEditorImplementation`, `EditorHost`) and the modules Lab shares (`react`, `react/jsx-runtime`, `@mantine/core` on `globalThis.__impLabShared`) |
| `apps/tauri/` | Imp Lab; projects of Tome data stores via [tome-rust-server](https://github.com/silentorb/tome-rust-server) `tome_db` (sibling checkout `../tome-rust-server`), Record Explorer (filter by node type), Go to Record quick-open (Ctrl+K), records saved to a chosen store, no filesystem open/save ([`docs/features/projects-and-records.md`](docs/features/projects-and-records.md)); editable Imp graphs via [imp-graph-view](https://github.com/silentorb/imp-graph-view) `ImpGraphView` (Precise ports, add-node picker (Shift+A / Edit → Add Node…), selected-node Title + input literals in Properties, on-node literal fields); extension editors for other node types (UI libraries loaded into the webview; Lab keeps their values and undo history, offers File → New per editor, routes commands to the active editor first); Edit Undo/Redo; host ELK layered LR layout; configurable input bindings (JSON → commands, extension `binding` components join the defaults; menus derive native accelerators); persisted UI chrome (open tabs, explorer prefs, extension component prefs, window geometry) under the OS app config dir; Edit → Settings… draws extension-declared settings (one page per settings category, Other fallback); extension status items; extension node UI in Properties (literal dropdowns from topics, node action buttons, service edits of node literals as one undo step per session); extensions push state to Lab on topics; Simple mode deferred |
### Input bindings

Lab follows the **VS Code menubar + keybinding** model: a central bindings table owns chords; native menus derive `.accelerator()` from that table; webview keydown resolves the same table. Detail: [`docs/features/input-bindings.md`](docs/features/input-bindings.md).

**Tauri/muda gap vs Electron (compromise today):** Electron can `registerAccelerator: false` (show shortcut without OS registration) and expose `triggeredByAccelerator` so accelerator activations replay into the keybinding service. muda/Tauri do not yet ([muda#393](https://github.com/tauri-apps/muda/issues/393), [tauri#15909](https://github.com/tauri-apps/tauri/issues/15909)). When those land, adopt them; do **not** redesign around label-suffix fake accelerators.

## Commands

From the repo root (library crates; default members):

```bash
cargo test
cargo clippy -- -D warnings
```

Desktop app (needs Bun + WebKit; in aura-workbench use the **imp-lab** service):

```bash
cd apps/tauri
bun install
bun run test
bun run build
cargo check -p imp_lab
```

`bun install` / `bun run build` run `scripts/link-graph-view-peers.mjs` so TypeScript can see a single React / `@xyflow` type root when typechecking workspace `imp-graph-view` sources (Bun hoists peers under `apps/tauri/node_modules`, which is not an ancestor of that package).

## Naming

Repos and projects use **kebab-case** (`imp-lab`). Rust crates use **snake_case** (`imp_lab_extensions`, `imp_lab`).

## Working conventions

- Follow [`docs/rust-style.md`](docs/rust-style.md) (links to imp-rs shared Rust conventions).
- Prefer the nearest `AGENTS.md` and this repo’s `docs/` when working in this tree.

## Where to look

- [`docs/rust-style.md`](docs/rust-style.md) — Rust conventions (links to imp-rs)
- [`docs/features/extensions.md`](docs/features/extensions.md) — optional Tome-shaped extensions (`libraryPath` cdylibs + `uiLibraryPath` UI libraries)
- [`docs/features/input-bindings.md`](docs/features/input-bindings.md) — configurable keyboard → command bindings
- [`docs/features/user-state.md`](docs/features/user-state.md) — persisted UI chrome (tabs, explorer and component prefs, window)
- [`docs/features/settings.md`](docs/features/settings.md) — extension settings, categories, associations; the Settings dialog
- [imp-rs](https://github.com/silentorb/imp-rs) — Imp core types and runtime crates
- [imp-spec](https://github.com/silentorb/imp-spec) — language-neutral Imp specs
- [imp-graph-view](https://github.com/silentorb/imp-graph-view) — Imp ↔ React Flow converters
