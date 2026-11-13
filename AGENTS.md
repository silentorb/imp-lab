# Imp Lab — Agent Instructions

## Repository purpose

**imp-lab** manages **scaling networks of Imp graphs** and provides editor/GUI building blocks for them. Language-neutral Imp specs live in **[imp-spec](https://github.com/silentorb/imp-spec)**; the Rust Imp core lives in **[imp-rs](https://github.com/silentorb/imp-rs)**.

This repo must not depend on Aura (`aura_*`) crates. Product hosts compose lab crates with their own chrome, registries, and domain extensions.

### Crate layers

- **Backend** — no Iced; session/network/document management and non-UI helpers.
- **Frontend** — Iced UI; may depend on backend crates directly.
- **Shared interfaces** — optional; not a required third layer.

| Rust crate | Layer | Role |
| --- | --- | --- |
| `crates/imp_documents/` | Backend | Multi-document session store with host-defined payloads |
| `crates/imp_graph_editor/` | Frontend | Iced Imp graph-dag building blocks (canvas, layout, edit helpers, chrome widgets) |

## Commands

From the repo root:

```bash
cargo test
cargo clippy -- -D warnings
```

## Naming

Repos and projects use **kebab-case** (`imp-lab`). Rust crates use **snake_case** (`imp_graph_editor`).

## Working conventions

- Follow [`docs/rust-style.md`](docs/rust-style.md) (links to imp-rs shared Rust conventions).
- Prefer the nearest `AGENTS.md` and this repo’s `docs/` when working in this tree.

## Where to look

- [`docs/rust-style.md`](docs/rust-style.md) — Rust conventions (links to imp-rs)
- [imp-rs](https://github.com/silentorb/imp-rs) — Imp core types and runtime crates
- [imp-spec](https://github.com/silentorb/imp-spec) — language-neutral Imp specs
