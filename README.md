# Imp Lab

Crates and a desktop app for **managing scaling networks of Imp graphs**. Language-neutral Imp specs live in [imp-spec](https://github.com/silentorb/imp-spec); the Rust Imp core lives in [imp-rs](https://github.com/silentorb/imp-rs).

Imp Lab is **domain-neutral**: it knows Imp graphs and Tome records, and nothing about any product's domain (no audio, no music notation). Products add their domain through Lab's optional [extensions system](docs/features/extensions.md): a Tome-shaped `extensions.json` that wires components from extension libraries (Rust cdylibs for backends, ES-module UI libraries for webview editors). This repo must not depend on Aura (`aura_*`) crates or packages. Aura's editor UI lives in [aura-editor](https://github.com/silentorb/aura-editor) and ships to Lab as part of the Aura pack.

## Layout

| Path | Role |
| --- | --- |
| `crates/imp_lab_extensions` | `extensions.json` parse, dynamic library loading, component declarations (catalogs, commands, menus, services, settings, node UI, node-type editors, bindings) |
| `packages/imp-lab-ui` | Contract between Lab and extension UI libraries: implementation types and the modules Lab shares with them |
| `apps/tauri` | Imp Lab (`imp_lab` / `imp-lab`, Tauri 2 + React + Mantine): projects of Tome data stores, records opened by Tome node type, editable Imp graphs via [imp-graph-view](https://github.com/silentorb/imp-graph-view), extension editors for other node types, configurable input bindings, extension settings / status items / node UI |

## Development

From repo root (library crates; default members):

```bash
cargo test
cargo clippy -- -D warnings
```

Desktop app (Bun + WebKit; in aura-workbench use the **imp-lab** Compose service):

```bash
cd apps/tauri
bun install
bun run test
bun run build
# or: bun run tauri dev
cargo check -p imp_lab
```

See [AGENTS.md](./AGENTS.md) and [docs/rust-style.md](./docs/rust-style.md).
