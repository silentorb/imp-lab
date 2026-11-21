# Imp Lab — Agent Instructions

## Repository purpose

**imp-lab** manages **scaling networks of Imp graphs** and provides editor/GUI building blocks for them. Language-neutral Imp specs live in **[imp-spec](https://github.com/silentorb/imp-spec)**; the Rust Imp core lives in **[imp-rs](https://github.com/silentorb/imp-rs)**.

This repo must not depend on Aura (`aura_*`) crates. Product hosts compose lab crates with their own chrome, registries, and domain extensions.

### Crate / app layers

- **Backend** — no Iced; session/network/document management and non-UI helpers.
- **Frontend (Iced)** — Iced UI building blocks; may depend on backend crates directly.
- **Frontend (Tauri, experimental)** — Tauri 2 + React + Mantine desktop shell under `apps/tauri/`; reversible; may be rolled back.
- **Shared interfaces** — optional; not a required third layer.

| Path | Layer | Role |
| --- | --- | --- |
| `crates/imp_documents/` | Backend | Multi-document session store with host-defined payloads |
| `crates/imp_graph_editor/` | Frontend (Iced) | Iced Imp graph-dag building blocks (canvas, layout, edit helpers, chrome widgets) |
| `crates/imp_lab_extensions/` | Backend | Tome-shaped optional `extensions.json` parse + dynamically linked extension loading (`imp-catalog`, `command`, `menu-item`, `service`) |
| `apps/tauri/` | Frontend (Tauri) | Experimental Imp desktop shell; Open/Save bridged; editable Imp graphs via [imp-graph-view](https://github.com/silentorb/imp-graph-view) `ImpGraphView` (Precise ports, Add palette, selected-node Title + input literals in Properties, on-node literal fields); Aura JSON `notation` documents with piano-roll editor (tools/snap/playhead); Edit Undo/Redo for Imp + notation history; host ELK layered LR layout; configurable input bindings (JSON → commands; menus derive native accelerators); persisted UI chrome (open tabs, piano prefs, window geometry) under the OS app config dir; optional Aura catalogs + playback (Play menu / Space toggle; graphs and notation tips) via Lab pack / `extensions.json` (no Aura crate dep); Simple mode deferred |

### Input bindings (Tauri)

Lab follows the **VS Code menubar + keybinding** model: a central bindings table owns chords; native menus derive `.accelerator()` from that table; webview keydown resolves the same table. Detail: [`docs/features/input-bindings.md`](docs/features/input-bindings.md).

**Tauri/muda gap vs Electron (compromise today):** Electron can `registerAccelerator: false` (show shortcut without OS registration) and expose `triggeredByAccelerator` so accelerator activations replay into the keybinding service. muda/Tauri do not yet ([muda#393](https://github.com/tauri-apps/muda/issues/393), [tauri#15909](https://github.com/tauri-apps/tauri/issues/15909)). When those land, adopt them; do **not** redesign around label-suffix fake accelerators.

## Commands

From the repo root (Iced crates; default members):

```bash
cargo test
cargo clippy -- -D warnings
```

Experimental Tauri host (needs Bun + WebKit; in aura-workbench use the **editor-tauri** service):

```bash
cd apps/tauri
bun install
bun run build
cargo check -p imp_lab_tauri
```

`bun install` / `bun run build` run `scripts/link-graph-view-peers.mjs` so TypeScript can see a single React / `@xyflow` type root when typechecking workspace `imp-graph-view` sources (Bun hoists peers under `apps/tauri/node_modules`, which is not an ancestor of that package).

## Naming

Repos and projects use **kebab-case** (`imp-lab`). Rust crates use **snake_case** (`imp_graph_editor`, `imp_lab_tauri`).

## Working conventions

- Follow [`docs/rust-style.md`](docs/rust-style.md) (links to imp-rs shared Rust conventions).
- Prefer the nearest `AGENTS.md` and this repo’s `docs/` when working in this tree.

## Where to look

- [`docs/rust-style.md`](docs/rust-style.md) — Rust conventions (links to imp-rs)
- [`docs/features/extensions.md`](docs/features/extensions.md) — optional Tome-shaped extensions (`libraryPath` + `clientModule`)
- [`docs/features/input-bindings.md`](docs/features/input-bindings.md) — configurable keyboard → command bindings (Tauri host)
- [`docs/features/user-state.md`](docs/features/user-state.md) — persisted UI chrome (tabs, piano prefs, window) for the Tauri host
- [imp-rs](https://github.com/silentorb/imp-rs) — Imp core types and runtime crates
- [imp-spec](https://github.com/silentorb/imp-spec) — language-neutral Imp specs
- [imp-graph-view](https://github.com/silentorb/imp-graph-view) — Imp ↔ React Flow converters (Tauri shell)
