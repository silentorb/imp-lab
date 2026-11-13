# Imp Lab

Crates for **managing scaling networks of Imp graphs**, plus editor/GUI building blocks for working with those graphs. Language-neutral Imp specs live in [imp-spec](https://github.com/silentorb/imp-spec); the Rust Imp core lives in [imp-rs](https://github.com/silentorb/imp-rs).

Product hosts (for example Aura Editor) supply chrome, registries, and domain extensions. This repo must not depend on Aura (`aura_*`) crates.

GUI and non-GUI concerns live in the same repository for now (separate crates). A further split within the repo is fine later; treating lab as “GUI only” is not the intended scope.

## Crate layers

| Layer | Rule |
| --- | --- |
| **Backend** | No Iced. Session/network/document management and other non-UI helpers. |
| **Frontend** | Iced UI. May depend on backend crates directly. |
| **Shared interfaces** | Not required. Add only if frontend and backend need a dependency-light shared surface. |

| Rust crate | Layer | Role |
| --- | --- | --- |
| `imp_documents` | Backend | Multi-document session store (tabs, paths, dirty/lifecycle) with host-defined payloads |
| `imp_graph_editor` | Frontend | Iced Imp graph-dag building blocks (canvas, edit helpers, chrome widgets) |

Shell and extension-framework extract from Aura Editor remains future work.

## Development

From repo root:

```bash
cargo test
cargo clippy -- -D warnings
```

See [AGENTS.md](./AGENTS.md) and [docs/rust-style.md](./docs/rust-style.md).
