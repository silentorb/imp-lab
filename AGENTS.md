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
| `apps/tauri/` | Frontend (Tauri) | Experimental Imp desktop shell; Open File bridged; Imp graphs view-only via [imp-graph-view](https://github.com/silentorb/imp-graph-view) with registry-backed (imp.core) + edge-inferred port handles; Aura catalogs not loaded; edit/save deferred |

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

## Naming

Repos and projects use **kebab-case** (`imp-lab`). Rust crates use **snake_case** (`imp_graph_editor`, `imp_lab_tauri`).

## Working conventions

- Follow [`docs/rust-style.md`](docs/rust-style.md) (links to imp-rs shared Rust conventions).
- Prefer the nearest `AGENTS.md` and this repo’s `docs/` when working in this tree.

## Where to look

- [`docs/rust-style.md`](docs/rust-style.md) — Rust conventions (links to imp-rs)
- [imp-rs](https://github.com/silentorb/imp-rs) — Imp core types and runtime crates
- [imp-spec](https://github.com/silentorb/imp-spec) — language-neutral Imp specs
- [imp-graph-view](https://github.com/silentorb/imp-graph-view) — Imp ↔ React Flow converters (Tauri shell)
