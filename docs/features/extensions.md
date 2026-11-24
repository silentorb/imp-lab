# Extension system

## Summary

Imp Lab’s optional **extension system** lets a host (or shipped pack) register external capabilities via `extensions.json`. Extensions are **libraries of components** integrated by configuration—not unconstrained global hooks.

Modeled after [Tome’s extension system](https://github.com/silentorb/tome/blob/main/docs/features/extensions.md) where stacks are compatible.

## Dual-stack modules

Imp Lab is a Rust host + React/TypeScript client. Each extension row may declare:

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

The Tauri shell keeps built-in File/Edit/View commands local; extension command ids are forwarded via `run_extension_command` with the active graph JSON and an optional neighbor path. Documents are Tome records (see [projects and records](projects-and-records.md)), so the neighbor path is the open project's config file (`imp-project.json`). An extension that follows references between records (a graph using a notation by node id) reads them from the project's data stores. With no project open it is null. The ABI is unchanged.

### `menu-item`

Adds a native menu entry. `implementationId` is the command id to emit. Params:

| Param | Role |
| --- | --- |
| `menu` | Submenu title (e.g. `Play`) |
| `order` | Sort order within that submenu (number; default `0`) |

`label` is the menu text. Core File / Edit / View menus stay host-owned; extension items append additional submenus.

### `service`

Long-lived host backend. The extension cdylib registers an `ImpLabServiceV0` vtable during `imp_lab_extension_register` under `implementationId` (e.g. `playback`). The host retains the service for the process lifetime and invokes it when extension commands run.

The vtable grew two optional entries, `event` and `request`, appended in lockstep with packs.

Besides the command actions (`start`, `stop`, `toggle`, `is_playing`, `position_secs`), a service may publish `event`: a UTF-8 JSON message the host passes through **without interpreting it**. Lab sends events with the `send_service_event` Tauri command (`serviceId`, `eventJson`); a missing `event` entry reports an unsupported action. The piano roll uses this for **note audition**: while a note is placed, pressed, or dragged it sends `{"type":"note_on","id","semitone","velocity"}`, retriggers on a pitch change, and sends `{"type":"note_off","id"}` on pointer up (`{"type":"all_notes_off"}` on tab switch) to the `playback` service. How the note sounds is the service's business; the Aura pack renders it in its streaming mode. Audition is offered when the pack's play commands are present and goes quiet after the first failed send (for example, no audio device).

A service may also publish `request`: UTF-8 JSON in, a UTF-8 JSON **reply** out, neither interpreted by the host. The host passes a reply buffer; the service writes the reply and its length, or returns `SERVICE_REPLY_TOO_SMALL` with the needed length, and the host retries once (`write_service_reply` is the extension-side helper). Lab sends requests with the `service_request` Tauri command (`serviceId`, `requestJson`) and gets the reply string back. Use `request` only for calls that need an answer; state that changes on its own goes on a [topic](#topics).

#### Live note capture protocol (optional)

The piano roll records takes through any service that answers these requests. It doesn't know where the notes come from (the Aura pack records a MIDI input):

| Request | Reply |
| --- | --- |
| `{"type":"capture_info"}` | `{"topic"}`: the take topic. Any answer means capture is offered, and the piano roll shows **Record**. |
| `{"type":"record_start","notation":{…},"startBeats":n,"click":bool}` | `{"topic"}`. The service plays the notation from the playhead (with a click and a count-in when `click` is set) and starts the take. |
| `{"type":"record_stop"}` | `{"notes":[{semitone,startBeats,durationBeats,velocity}…]}`, the take as notation notes |

While a take runs, the service publishes `{recording, countIn, notes}` on the take topic, with notes growing as keys are held. The piano roll draws them as an overlay. If the topic turns `recording: false` (the transport was stopped some other way), Lab sends `record_stop` to collect the take. Lab adds the notes to the notation it was recording into as **one undoable edit** (overdub). See the piano-roll [recording](#piano-roll-recording) behavior below.

### Topics

The extension → host channel. `ImpLabHostV0` carries `publish(topic, json)` with its own `publisher_ctx`, which, unlike `ctx`, lives for the whole process. An extension keeps both (`ImpLabHostV0::publisher()` gives a `Publisher`) and publishes the **latest value** of a named topic whenever it changes, from any thread.

- A topic id is a string; by convention it starts with the publishing extension's id (`aura.midi.status`). Its value is opaque JSON.
- Publishing replaces the value. There is no history or queue, and the extension chooses how often to publish (throttling fast-changing values).
- The callback only stores the value and hands it to the UI; it never blocks on the UI or calls back into the extension. Invalid UTF-8 or JSON returns non-zero.
- The desktop host forwards each change to the webview as an `extension://topic` event (`{topic, value}`); `get_topics` returns every topic's latest value, for the webview's starting snapshot.
- Settings options (`optionsTopic`), status items, and the piano-roll take read topics. Nothing in Lab polls an extension for them.

### Settings components

Settings, categories, and their associations are separate kinds, so any extension can place any setting on any page. The dialog is described in [settings](settings.md).

| Kind | Fields |
| --- | --- |
| `setting` | `id` is the setting id; `label`; `params`: `valueType` (`choice` \| `multi-choice` \| `boolean`), `default`, `description`, `options` (`[{value,label}]`) or `optionsTopic`, `minSelected`, `serviceId` (the service component that receives the value) |
| `settings-category` | `id`; `label` (or `params.title`) is the page title |
| `settings-category-item` | `params`: `settingId`, `categoryId`, `order` |
| `status-item` | `label` (tooltip); `params`: `topic` (value `{text, level}`, `level` = `ok` \| `warn` \| `error` \| `off`), `openCategory` (a settings category to open on click), `order` |

`implementationId` is optional for these kinds. Status items show in the status bar and the piano-roll toolbar, hidden until their topic has a value.

### Piano-roll recording

With a capture service present, the piano-roll toolbar shows **Record** (`R`) and **Click** (metronome and one-bar count-in, persisted).

- Record starts a take from the parked playhead in the active notation. The playhead follows the transport, and the editing tools are inert (panning still works).
- Stop (Record again, Space, or Play › Stop) adds the take as one edit. Switching tabs or closing the notation ends the take first.
- An empty take adds nothing to history.

## Loading (host)

1. Resolve and parse `extensions.json` (optional) during app setup (before building menus).
2. For each enabled extension with `libraryPath`, load the cdylib and call `imp_lab_extension_register`.
3. Enable components whose `extensionId` is enabled; for `imp-catalog`, apply registered catalogs; collect registered services; build menus from `menu-item` rows; lay out settings pages and status items (`get_settings_layout`, `get_status_items`).

Extensions **must not** depend on the Imp Lab app crate (`imp_lab`). They depend on `imp_lab_extensions` (register contract) and domain crates as needed.

## Product packs

Aura ships an audio Lab pack (`extensions.json` + catalog/playback cdylibs + data) beside `imp-lab` on Windows export. Lab itself never depends on Aura crates or packages.

## Non-goals (for now)

- Full client-side TS extension runtime / dynamic UI registration
- Marketplace
- Extension-defined settings controls beyond the declared value types (settings are drawn by Lab from declarations; see [settings](settings.md))
- Dynamic unsigned remote plugins
- Iced Aura Editor extension runtime (deprecated; Imp Lab + packs are the editor path)

## See also

- Tome: [extensions.md](https://github.com/silentorb/tome/blob/main/docs/features/extensions.md)
- Code: `crates/imp_lab_extensions/`
