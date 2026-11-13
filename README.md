# Imp Lab

General-purpose Imp editor and GUI library crates. Product hosts (for example Aura Editor) supply chrome, registries, and domain extensions.

## Crates

| Rust crate | Role |
| --- | --- |
| `imp_graph_editor` | Iced Imp graph-dag building blocks (canvas, edit helpers, chrome widgets) |

Shell and extension-framework extract from Aura Editor is future work.

## Development

From repo root:

```bash
cargo test
cargo clippy -- -D warnings
```

See [AGENTS.md](./AGENTS.md) and [docs/rust-style.md](./docs/rust-style.md).
