# Projects and records

Imp Lab has no file open / save. You open a **project**, which configures one or more **data stores**, and every document is a **record** (a Tome node) in one of them. Storage, sync and queries come from [tome-rust-server](../../../tome-rust-server) (`tome_db`), so a project is also a valid TS Tome configuration.

## Project

A project is a Tome `tome-server` v2 config, written as `imp-project.json`:

```json
{
  "version": 2,
  "dataStores": {
    "main":  { "id": "main", "module": "tome-flatfile", "export": "createFlatfileModule",
               "options": { "contentPath": "content", "access": "readwrite" } },
    "cache": { "id": "cache", "module": "tome-sqlite", "export": "createSqliteModule",
               "options": { "dbPath": ".tome/cache.sqlite" } }
  },
  "sync": { "queryStoreId": "cache" },
  "services": []
}
```

- **Data stores** are the `tome-flatfile` entries, in config order. Each one is a Tome content folder (`data/`, `archive/`, `model/`), either `readwrite` or `readonly`.
- **The primary store** is the first flatfile store. New projects put it at `./content`. It holds the shared model (type tables), and it is the default save target.
- **Relative `contentPath`s** resolve against the project folder.
- **The query cache** is derived data. Lab ignores the configured `dbPath` and keeps the cache in the OS cache directory (`IMP_CACHE_DIR` overrides it), keyed by project path. `.tome/` is git-ignored for other tools.
- **File menu:**
  - **New Project…** picks a folder, writes the config, bootstraps the primary store and opens it.
  - **Open Project…** opens any v2 config, including an existing Tome corpus.
- **Manage Data Stores…** adds stores (an id, a folder and read/write or read-only) and removes non-primary ones. It rewrites the config and reopens the session. A removed store's files stay on disk.

## Records

A record is one Tome node, and what it is comes from its **Tome node type**: membership in a type table. There are no document types on top of Tome. Lab opens a node when it has an editor for one of its node types:

| Node type | Type table (id) | Value column | Editor |
| --- | --- | --- | --- |
| Imp graph | `01M80M9GRAPH00000000000001` "Imp Graphs" | `graph`, `imp_graph`, node storage: bare `{ nodes, edges }` | built in |
| any other | named by an extension's `node-type-editor` | named by that component (`json` by default) | the extension's UI library ([extensions](extensions.md#node-type-editor)) |

- **On disk**, the title, timestamps and value sit in the node's YAML frontmatter. The node type comes from set membership in the type table.
- **Opening a project** adds the Imp Graphs table to a writable primary store when it is missing (`lab_model::ensure_lab_model`). An extension node type's table (schema and hub node) is added to the primary store the first time a record of it is created or saved. A new project also gets a set-trait relationship type and home and archive hubs.
- **Identity** is the node id, which is unique across every store in the project. A record lives in exactly one store. It may reference type tables in another store, in which case the membership edge is written to both stores, as TS Tome does.
- **Without an editor** for its node type (an extension not loaded), a record is listed but can't be opened.

## Record Explorer

The left panel lists the project's stores, each with its access, a primary badge and a record count, and the records in each store.

- **Filter.** A title filter (Ctrl+P) and a node-type switch: Lab (every node type Lab can open), one entry per node type (Graphs, then each extension editor's type title), or All. All also lists other Tome nodes, but you can't open them.
- **Opening.** Clicking a record opens it, or switches to its tab if it is already open.
- **Row menu.** Rename, Duplicate to store… and Archive. Archiving moves the files under the store's `archive/`.
- **The + button** on a writable store creates an empty record of any openable node type directly in that store.

## Go to Record

View → Go to Record… (Ctrl+K) opens a search over the project's openable records across every store, so you can reach a record without browsing the explorer. With an empty query it lists the most recently modified records. Typing filters by title on the host (`records_list` text filter) and ranks prefix and word-start matches first. Enter or a click opens the record, or switches to its tab if it is already open. Up to 50 results are shown; narrow the query to reach the rest.

## Saving

- **Save on a record** overwrites its value (`record_save`).
- **Save on a new document** (File → New graph, or File → New *label* for an extension editor) opens **Save new record**. You give a title and choose a store. Read-only stores are listed but disabled. The default is the last store you used, otherwise the primary.
- **Duplicate to Store…** copies the active document, or an explorer row, into any writable store as a new record.
- **Closing a dirty tab** asks whether to save, discard or cancel.

## Sample projects

Lab has no import of standalone `.json` documents. Products that ship samples build a project ahead of time with the seed tool:

```bash
cd apps/tauri/src-tauri
cargo run --example seed_project -- [--extensions <extensions.json>] <project-dir> <name> <doc.json>...
```

Each document becomes a record in the new project's primary store, titled after its file name. Its `type` tag picks the node type: `imp-graph` for Imp graphs, or the `documentType` of a `node-type-editor` in the `--extensions` config. A document's `id` is kept as the node id when it is a ULID, so records that reference each other by id still line up (`src-tauri/src/seed.rs`).

## Live changes

The host watches every store folder, polling roughly every 400 ms through `TomeSession::pump_external_changes`. It emits `records://changed` after local writes and after external edits such as a git pull, another editor or TS Tome.

- The explorer refreshes on every event.
- A clean tab whose record changed outside Lab reloads.
- A dirty tab keeps your edits and shows a status warning.
- If a record was deleted outside Lab, its tab becomes an unsaved document.

## Host commands

| Command | Purpose |
| --- | --- |
| `project_current`, `project_open`, `project_pick_and_open`, `project_pick_and_create`, `project_close` | Project lifecycle |
| `project_add_store`, `project_remove_store`, `project_pick_store_folder` | Manage data stores |
| `records_list` | Explorer rows `{ nodeId, title, nodeType, corpusId, modifiedAt }` (`nodeType` null when Lab can't open it); filter `{ nodeType: <id> \| "openable" \| "all", corpusId, text }` |
| `record_open` | `{ id, title, nodeType, record: { nodeId, corpusId, contentDir }, value }` |
| `record_create`, `record_save`, `record_rename`, `record_archive` | Writes (payload `{ nodeType, value }`) |

Errors are `code: message` strings, for example `corpus_readonly: …`, `not_found: …` and `no_project: …`.
