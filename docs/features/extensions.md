# Extension system

## Summary

Imp Lab’s optional **extension system** lets a host (or shipped pack) register external capabilities via `extensions.json`. Extensions are **libraries of components** integrated by configuration—not unconstrained global hooks.

Modeled after [Tome’s extension system](https://github.com/silentorb/tome/blob/main/docs/features/extensions.md) where stacks are compatible.

## Dual-stack modules

Lab Tauri is Rust host + React/TypeScript client. Each extension row may declare:

| Field | Role |
| --- | --- |
| `libraryPath` | Sidecar **Rust cdylib** (host / “server”), loaded with `libloading` |
| `clientModule` | **TypeScript** entry for the webview (UI kinds; pack-relative or resolvable id) |

Both are optional (like Tome’s optional subsystem modules). Missing `extensions.json` ⇒ Imp core only; the host still starts.

**Current slice:** host loads `libraryPath` for `imp-catalog` components and pushes Imp libraries into the frontend registry. `clientModule` is parsed and reserved; dynamic client import/register lands when a kind needs UI chrome.

Lab and its extensions are developed in lockstep—there is no long-lived ABI stability campaign. Rebuild host and extension libs together as the register contract evolves.

## Configuration

`extensions.json` shape:

```json
{
  "version": 1,
  "extensions": [
    {
      "id": "aura-imp-catalogs",
      "enabled": true,
      "libraryPath": "extensions/aura_imp_catalogs.dll",
      "clientModule": null,
      "params": {}
    }
  ],
  "components": [
    {
      "id": "aura-imp-catalogs",
      "extensionId": "aura-imp-catalogs",
      "kind": "imp-catalog",
      "implementationId": "aura-imp-catalogs",
      "enabled": true,
      "params": {}
    }
  ]
}
```

Rules:

- Top-level: `version`, `extensions[]`, `components[]`.
- Host merges extension-level and component-level `params` (component wins).
- Do not encode kind in component ids. Prefer component id = extension id when there is one component per extension.
- Paths in `libraryPath` resolve relative to the config file’s directory (pack root).
- Config search (Tauri): `IMP_LAB_EXTENSIONS` → directory of the executable → current working directory.
- Unsupported version or parse failure ⇒ log and continue with core-only (do not refuse to start).

## Component kinds

### `imp-catalog`

Registers Imp `NodeLibrary`, `TypeConstraintLibrary`, and/or `GraphLibrary` catalogs into the host registry (Add palette, ports, literals). Implementations live in the extension cdylib and call the Lab register entrypoint.

## Loading (host)

1. Resolve and parse `extensions.json` (optional).
2. For each enabled extension with `libraryPath`, load the cdylib and call `imp_lab_extension_register`.
3. Enable components whose `extensionId` is enabled; for `imp-catalog`, apply registered catalogs.

Extensions **must not** depend on the Tauri app crate (`imp_lab_tauri`). They depend on `imp_lab_extensions` (register contract) and domain crates as needed.

## Product packs

Aura ships an audio Lab pack (`extensions.json` + catalog cdylib + data) beside `imp-lab-tauri` on Windows export. Lab itself never depends on Aura crates or packages.

## Non-goals (for now)

- Full client-side TS extension runtime / dynamic UI registration
- Marketplace / config UI
- Dynamic unsigned remote plugins
- Iced Aura Editor extension runtime (deprecated; Tauri Lab + packs are the editor path)

## See also

- Tome: [extensions.md](https://github.com/silentorb/tome/blob/main/docs/features/extensions.md)
- Code: `crates/imp_lab_extensions/`
