# Extension system

## Summary

Imp Lab’s optional **extension system** lets a host (or shipped pack) register external capabilities via `extensions.json`. Extensions are **libraries of implementations**; **components** in `extensions.json` choose an implementation by `implementationId` and configure it with `params`. Configuration does the wiring—there are no unconstrained global hooks and no self-registering code.

Lab is domain-neutral: everything a product's domain needs (catalogs, transport, settings, editors for its own node types) comes in this way. See [projects and records](projects-and-records.md) for how records and node types relate.

Modeled after [Tome’s extension system](https://github.com/silentorb/tome/blob/main/docs/features/extensions.md) where stacks are compatible.

## Libraries

Imp Lab is a Rust host + React/TypeScript webview. Each extension row may name two libraries:

| Field | Role |
| --- | --- |
| `libraryPath` | **Rust cdylib** (backend), loaded with `libloading`. Registers backend implementations (catalogs, services) by id during `imp_lab_extension_register`. |
| `uiLibraryPath` | **UI library** for the webview: one ES module exporting `implementations`, an object of UI implementations keyed by implementation id (see [UI libraries](#ui-libraries)). |

Both are optional (like Tome’s optional subsystem modules). Missing `extensions.json` ⇒ Imp core only; the host still starts.

Lab and its extensions are developed in lockstep—there is no long-lived ABI stability campaign. Rebuild host and extension libs together as the register contract evolves.

## Configuration

`extensions.json` shape (an illustrative pack with a playback service and an editor for a `sketch` node type):

```json
{
  "version": 1,
  "extensions": [
    { "id": "acme-playback", "enabled": true, "libraryPath": "extensions/acme_playback", "params": {} },
    { "id": "acme-editor", "enabled": true, "uiLibraryPath": "extensions/acme_editor.js", "params": {} }
  ],
  "components": [
    {
      "id": "acme-playback",
      "extensionId": "acme-playback",
      "kind": "service",
      "implementationId": "playback",
      "params": {}
    },
    {
      "id": "play.toggle",
      "extensionId": "acme-playback",
      "kind": "command",
      "implementationId": "play.toggle",
      "params": { "serviceId": "acme-playback", "action": "toggle" }
    },
    {
      "id": "menu.play.toggle",
      "extensionId": "acme-playback",
      "kind": "menu-item",
      "implementationId": "play.toggle",
      "label": "Play / Stop",
      "params": { "menu": "Play", "order": 10 }
    },
    {
      "id": "acme.sketch-editor",
      "extensionId": "acme-editor",
      "kind": "node-type-editor",
      "implementationId": "sketch-editor",
      "label": "Sketch",
      "params": {
        "nodeTypeId": "01M80M9SKETCH0000000000001",
        "typeTitle": "Sketches",
        "column": "sketch",
        "documentType": "sketch",
        "serviceId": "acme-playback"
      }
    },
    {
      "id": "acme.key.play",
      "extensionId": "acme-playback",
      "kind": "binding",
      "params": { "command": "play.toggle", "key": "space" }
    }
  ]
}
```

Rules:

- Top-level: `version`, `extensions[]`, `components[]`.
- Host merges extension-level and component-level `params` (component wins).
- Do not encode kind in component ids. Prefer component id = extension id when there is one component per extension.
- Paths in `libraryPath` / `uiLibraryPath` resolve relative to the config file’s directory (pack root); a UI library path must stay inside the pack root.
- Config search (Tauri): `IMP_LAB_EXTENSIONS` → directory of the executable → current working directory.
- Unsupported version or parse failure ⇒ log and continue with core-only (do not refuse to start).

## Component kinds

### `imp-catalog`

Registers Imp `NodeLibrary`, `TypeConstraintLibrary`, and/or `GraphLibrary` catalogs into the host registry (add-node picker, ports, literals). Implementations live in the extension cdylib and call the Lab register entrypoint.

### `command`

Declares a host command id (`implementationId`, e.g. `play.start`) dispatched to a service. Params:

| Param | Role |
| --- | --- |
| `serviceId` | Id of the enabled `service` component that handles the command |
| `action` | Service action: `start`, `stop`, `toggle`, or `is_playing` |

The Tauri shell keeps built-in File/Edit/View commands local. A command goes first to the active document's extension editor (if it handles it), then to the service via `run_extension_command`, with the active document's value as JSON (the Imp graph, or an extension node type's stored value) and an optional neighbor path. Documents are Tome records (see [projects and records](projects-and-records.md)), so the neighbor path is the open project's config file (`imp-project.json`). An extension that follows references between records (a graph using another record by node id) reads them from the project's data stores. With no project open it is null.

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

Besides the command actions (`start`, `stop`, `toggle`, `is_playing`, `position_secs`), a service may publish `event`: a UTF-8 JSON message the host passes through **without interpreting it**. The webview sends events with the `send_service_event` Tauri command (`serviceId`, `eventJson`); a missing `event` entry reports an unsupported action. Extension editors use events for fire-and-forget messages to their services.

A service may also publish `request`: UTF-8 JSON in, a UTF-8 JSON **reply** out, neither interpreted by the host. The host passes a reply buffer; the service writes the reply and its length, or returns `SERVICE_REPLY_TOO_SMALL` with the needed length, and the host retries once (`write_service_reply` is the extension-side helper). The webview sends requests with the `service_request` Tauri command (`serviceId`, `requestJson`) and gets the reply string back. Use `request` only for calls that need an answer; state that changes on its own goes on a [topic](#topics).

### Topics

The extension → host channel. `ImpLabHostV0` carries `publish(topic, json)` with its own `publisher_ctx`, which, unlike `ctx`, lives for the whole process. An extension keeps both (`ImpLabHostV0::publisher()` gives a `Publisher`) and publishes the **latest value** of a named topic whenever it changes, from any thread.

- A topic id is a string; by convention it starts with the publishing extension's id (`acme.midi.status`). Its value is opaque JSON.
- Publishing replaces the value. There is no history or queue, and the extension chooses how often to publish (throttling fast-changing values).
- The callback only stores the value and hands it to the UI; it never blocks on the UI or calls back into the extension. Invalid UTF-8 or JSON returns non-zero.
- The desktop host forwards each change to the webview as an `extension://topic` event (`{topic, value}`); `get_topics` returns every topic's latest value, for the webview's starting snapshot.
- Settings options (`optionsTopic`), status items, and extension editors (`EditorHost.useTopic`) read topics. Nothing in Lab polls an extension for them.

### Settings components

Settings, categories, and their associations are separate kinds, so any extension can place any setting on any page. The dialog is described in [settings](settings.md).

| Kind | Fields |
| --- | --- |
| `setting` | `id` is the setting id; `label`; `params`: `valueType` (`choice` \| `multi-choice` \| `boolean`), `default`, `description`, `options` (`[{value,label}]`) or `optionsTopic`, `minSelected`, `serviceId` (the service component that receives the value) |
| `settings-category` | `id`; `label` (or `params.title`) is the page title |
| `settings-category-item` | `params`: `settingId`, `categoryId`, `order` |
| `status-item` | `label` (tooltip); `params`: `topic` (value `{text, level}`, `level` = `ok` \| `warn` \| `error` \| `off`), `openCategory` (a settings category to open on click), `order` |

`implementationId` is optional for these kinds. Status items show in the status bar, and extension editors may show them in their toolbars (`EditorHost.statusItems`), hidden until their topic has a value.

### Node UI components

An extension can declare UI for node types it knows (its own catalog types, say). Lab draws it in **Properties** without knowing what the node types mean. `get_node_ui` returns the declarations.

| Kind | Fields |
| --- | --- |
| `node-literal-options` | `params`: `nodeType`, `port` (a literal input), `optionsTopic` (value `[{value, label}]`). Properties draws that literal as a searchable dropdown of the topic's options (a current value not among them stays listed, marked not found). A wired port keeps its plain field. |
| `node-action` | `label` (button text); `params`: `nodeType`, `serviceId` (the service component that receives it), `action`, optional `editsTopic`, `order`. Properties shows a button for a selected node of that type. |

Clicking an action sends `service_request` `{"type":"node_action","action","node":{id,type,inputs}}` to the service. A failed request (or a `{"error"}` reply) shows its message under the buttons.

A service may then **edit the node** through the action's `editsTopic` (an extension opening a plugin editor saves the plugin's state into the node, say). The topic value is `{seq, session, node, inputs}`: Lab applies each new `seq` to the open graph holding node `node` (preferring the document the action was run from), setting each literal in `inputs` and removing those that are `null`. Edits that share a `session` are **one undo step**. An edit published before Lab subscribed is not replayed.

### `node-type-editor`

An editor for records of one **Tome node type**, implemented in the extension's UI library. Lab has no document types: a record's node type is its type-table membership, and this component says which editor opens it.

| Field | Role |
| --- | --- |
| `implementationId` | A `node-type-editor` implementation exported by the extension's UI library |
| `label` | One record ("Sketch"): File → New *label*, explorer badges |
| `params.nodeTypeId` | Tome type-table id; records that are members of it open in this editor |
| `params.column` | Node-stored column holding the record's value |
| `params.columnType` | Tome column type of `column` (default `json`) |
| `params.typeTitle` | Title of the type-table hub ("Sketches"), and the explorer filter label |
| `params.documentType` | Optional: `type` tag of standalone JSON documents of this node type (the seed tool) |
| `params.serviceId` | Optional: a `service` component the editor talks to; the implementation receives that service's implementation id |

All params reach the implementation (`EditorHost.params`). One editor per node type; later duplicates are reported and ignored. `get_ui_extensions` returns the declarations and the libraries to load.

Lab, for each editor:

- opens its node type's records (the column value goes through the implementation's `parse`), keeps the value, dirty state and **undo history** (edits sharing a coalesce key are one step), and saves it back through `serialize`;
- adds **File → New *label*** (command `document.new:<component id>`) and the type to the explorer's **+** menu and node-type filter;
- adds the node type's table (schema and hub node) to the primary store the first time a record of it is created or saved there (read-only primaries are left alone);
- offers every command to the active document's editor first (`EditorHost.onCommand`), so an editor can own commands its `binding` components name.

Without the editor (pack not loaded), records of that node type are listed but not openable.

### `binding`

A default key binding. Params: `command`, `key`, optional `modifiers` (as in [input bindings](input-bindings.md)). Extension bindings join Lab's bundled defaults (a user bindings file replaces both), and the commands they name become known commands, so an extension editor can own commands Lab has never heard of.

## UI libraries

A UI library (`uiLibraryPath`) is one ES module. It exports `implementations`, an object keyed by implementation id; it contains no wiring. The contract is the `imp-lab-ui` package (`packages/imp-lab-ui`):

- `NodeTypeEditorImplementation<V>`: `kind: "node-type-editor"`, `empty()`, `parse(stored)` (throw when invalid), optional `serialize`, `summarize`, optional `properties` (Properties rows) and `equals` (default JSON equality), and `Panel`, a React component receiving `{ value, onChange(next, coalesceKey?), host }`.
- `EditorHost`: the component's `params` and id, whether its document is the active tab, `setStatus`, `statusItems`, `hasCommand` / `runCommand` (extension commands with a document payload), `playbackStatus`, `sendServiceEvent`, `requestService`, `useTopic`, `loadPrefs` / `savePrefs` (the component's own [user state](user-state.md) entry), and `onCommand`.

Lab reads each library with `read_ui_library` and imports it as a module. **Libraries don't bundle React or Mantine**: Lab shares its instances of `react`, `react/jsx-runtime` and `@mantine/core` (`SHARED_MODULE_IDS`) on `globalThis.__impLabShared`, and a library's build maps those imports onto that object. A library that fails to load only drops its own editors.

## Loading (host)

1. Resolve and parse `extensions.json` (optional) during app setup (before building menus).
2. For each enabled extension with `libraryPath`, load the cdylib and call `imp_lab_extension_register`.
3. Enable components whose `extensionId` is enabled; for `imp-catalog`, apply registered catalogs; collect registered services; build menus from `menu-item` rows and one New item per `node-type-editor`; merge `binding` components into the default bindings; lay out settings pages and status items (`get_settings_layout`, `get_status_items`).
4. The webview loads the UI library of every extension with an enabled UI component and resolves each `node-type-editor` to its implementation before restoring tabs.

Extensions **must not** depend on the Imp Lab app crate (`imp_lab`). Backend libraries depend on `imp_lab_extensions` (register contract) and domain crates as needed; UI libraries on `imp-lab-ui`.

## Product packs

A product ships a **pack**: `extensions.json` plus the libraries and data it names, placed beside `imp-lab` (or pointed to by `IMP_LAB_EXTENSIONS`). Aura's pack adds audio catalogs, playback, MIDI input, plugin node UI and Aura's editors; its configuration is owned by [aura-editor](https://github.com/silentorb/aura-editor). Lab itself never depends on any product's crates or packages.

## Non-goals (for now)

- Marketplace
- Extension-defined settings controls beyond the declared value types (settings are drawn by Lab from declarations; see [settings](settings.md))
- Dynamic unsigned remote plugins
- UI component kinds beyond `node-type-editor` (add them as product needs arrive)

## See also

- Tome: [extensions.md](https://github.com/silentorb/tome/blob/main/docs/features/extensions.md)
- Code: `crates/imp_lab_extensions/`, `packages/imp-lab-ui/`, `apps/tauri/src/extensionUi.ts`
