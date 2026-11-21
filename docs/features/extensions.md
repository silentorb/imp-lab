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

**Current slice:** host loads `libraryPath` for `imp-catalog` and `service` components, contributes `command` / `menu-item` chrome from config, and dispatches extension commands to registered services. `clientModule` is parsed and reserved; dynamic client import/register lands when a kind needs UI chrome.

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
      "libraryPath": "extensions/aura_imp_lab_catalogs",
      "clientModule": null,
      "params": {}
    },
    {
      "id": "aura-playback",
      "enabled": true,
      "libraryPath": "extensions/aura_imp_lab_playback",
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
    },
    {
      "id": "aura-playback",
      "extensionId": "aura-playback",
      "kind": "service",
      "implementationId": "playback",
      "enabled": true,
      "params": {}
    },
    {
      "id": "play.start",
      "extensionId": "aura-playback",
      "kind": "command",
      "implementationId": "play.start",
      "enabled": true,
      "params": { "serviceId": "aura-playback", "action": "start" }
    },
    {
      "id": "play.stop",
      "extensionId": "aura-playback",
      "kind": "command",
      "implementationId": "play.stop",
      "enabled": true,
      "params": { "serviceId": "aura-playback", "action": "stop" }
    },
    {
      "id": "play.toggle",
      "extensionId": "aura-playback",
      "kind": "command",
      "implementationId": "play.toggle",
      "enabled": true,
      "params": { "serviceId": "aura-playback", "action": "toggle" }
    },
    {
      "id": "menu.play.start",
      "extensionId": "aura-playback",
      "kind": "menu-item",
      "implementationId": "play.start",
      "label": "Start",
      "enabled": true,
      "params": { "menu": "Play", "order": 10 }
    },
    {
      "id": "menu.play.stop",
      "extensionId": "aura-playback",
      "kind": "menu-item",
      "implementationId": "play.stop",
      "label": "Stop",
      "enabled": true,
      "params": { "menu": "Play", "order": 20 }
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

### `command`

Declares a host command id (Aura Iced-style, e.g. `play.start`). Params:

| Param | Role |
| --- | --- |
| `serviceId` | Id of the enabled `service` component that handles the command |
| `action` | Service action: `start`, `stop`, `toggle`, or `is_playing` |

The Tauri shell keeps built-in File/Edit/View commands local; extension command ids are forwarded via `run_extension_command` with the active graph JSON and optional document path.

### `menu-item`

Adds a native menu entry. `implementationId` is the command id to emit. Params:

| Param | Role |
| --- | --- |
| `menu` | Submenu title (e.g. `Play`) |
| `order` | Sort order within that submenu (number; default `0`) |

`label` is the menu text. Core File / Edit / View menus stay host-owned; extension items append additional submenus.

### `service`

Long-lived host backend. The extension cdylib registers an `ImpLabServiceV0` vtable during `imp_lab_extension_register` under `implementationId` (e.g. `playback`). The host retains the service for the process lifetime and invokes it when extension commands run.

## Loading (host)

1. Resolve and parse `extensions.json` (optional) during app setup (before building menus).
2. For each enabled extension with `libraryPath`, load the cdylib and call `imp_lab_extension_register`.
3. Enable components whose `extensionId` is enabled; for `imp-catalog`, apply registered catalogs; collect registered services; build menus from `menu-item` rows.

Extensions **must not** depend on the Tauri app crate (`imp_lab_tauri`). They depend on `imp_lab_extensions` (register contract) and domain crates as needed.

## Product packs

Aura ships an audio Lab pack (`extensions.json` + catalog/playback cdylibs + data) beside `imp-lab-tauri` on Windows export. Lab itself never depends on Aura crates or packages.

## Non-goals (for now)

- Full client-side TS extension runtime / dynamic UI registration
- Marketplace / config UI
- Dynamic unsigned remote plugins
- Iced Aura Editor extension runtime (deprecated; Tauri Lab + packs are the editor path)

## See also

- Tome: [extensions.md](https://github.com/silentorb/tome/blob/main/docs/features/extensions.md)
- Code: `crates/imp_lab_extensions/`
