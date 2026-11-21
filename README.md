# Imp Lab

Crates for **managing scaling networks of Imp graphs**, plus editor/GUI building blocks for working with those graphs. Language-neutral Imp specs live in [imp-spec](https://github.com/silentorb/imp-spec); the Rust Imp core lives in [imp-rs](https://github.com/silentorb/imp-rs).

Product hosts (for example Aura Editor) supply domain chrome, registries, and extensions. This repo must not depend on Aura (`aura_*`) crates.

GUI and non-GUI concerns live in the same repository for now (separate crates / apps). A further split within the repo is fine later; treating lab as “GUI only” is not the intended scope.

## Layers

| Layer | Rule |
| --- | --- |
| **Backend** | No Iced. Session/network/document management and other non-UI helpers. |
| **Frontend (Iced)** | Iced UI building blocks. May depend on backend crates directly. |
| **Frontend (Tauri, experimental)** | Tauri 2 + React + Mantine desktop shell. Reversible; may be rolled back. |
| **Shared interfaces** | Not required. Add only if frontend and backend need a dependency-light shared surface. |

| Path | Layer | Role |
| --- | --- | --- |
| `imp_documents` | Backend | Multi-document session store (tabs, paths, dirty/lifecycle) with host-defined payloads |
| `imp_graph_editor` | Frontend (Iced) | Iced Imp graph-dag building blocks (canvas, edit helpers, chrome widgets) |
| `apps/tauri` | Frontend (Tauri) | Experimental Imp desktop shell (`imp-lab-tauri`); Open/Save bridged; editable graphs via [imp-graph-view](https://github.com/silentorb/imp-graph-view) `ImpGraphView` (Precise ports + Add); host ELK LR layout; Aura catalogs not loaded; Simple/retitle deferred |

Iced shell/extension-framework extract from Aura Editor remains future work. The Tauri app is a separate experimental host, not that extract.

## Development

From repo root (Iced crates):

```bash
cargo test
cargo clippy -- -D warnings
```

Experimental Tauri (Bun + WebKit; in aura-workbench use the **editor-tauri** Compose service):

```bash
cd apps/tauri
bun install
bun run build
# or: bun run tauri dev
cargo check -p imp_lab_tauri
```

See [AGENTS.md](./AGENTS.md) and [docs/rust-style.md](./docs/rust-style.md).
