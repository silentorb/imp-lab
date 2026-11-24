# Projects and records

Imp Lab has no file open / save. You open a **project**, which configures one or more **data stores**, and every graph or notation is a **record** in one of them. Storage, sync and queries come from [tome-rust-server](../../../tome-rust-server) (`tome_db`), so a project is also a valid TS Tome configuration.

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

A record is one Tome node:

| Kind | Type table (well-known id) | Body column |
| --- | --- | --- |
| Imp graph | `01M80M9GRAPH00000000000001` "Imp Graphs" | `graph`, `imp_graph`, node storage: bare `{ nodes, edges }` |
| Notation | `01M80M9N0TAT00000000000001` "Notations" | `notation`, `json`, node storage: the notation tip |

- **On disk**, the title, timestamps and body sit in the node's YAML frontmatter. The kind comes from set membership in the type table.
- **Opening a project** adds the type tables to a writable primary store when they are missing (`lab_model::ensure_lab_model`). A new project also gets a set-trait relationship type and home and archive hubs.
- **Identity** is the node id, which is unique across every store in the project. A record lives in exactly one store. It may reference type tables in another store, in which case the membership edge is written to both stores, as TS Tome does.

## Record Explorer

The left panel lists the project's stores, each with its access, a primary badge and a record count, and the records in each store.

- **Filter.** A title filter (Ctrl+P) and a kind switch: Lab, Graphs, Notes, or All. All also lists other Tome nodes, but you can't open them.
- **Opening.** Clicking a record opens it, or switches to its tab if it is already open.
- **Row menu.** Rename, Duplicate to store… and Archive. Archiving moves the files under the store's `archive/`.
- **The + button** on a writable store creates an empty graph or notation directly in that store.

## Go to Record

View → Go to Record… (Ctrl+K) opens a search over the project's graphs and notations across every store, so you can reach a record without browsing the explorer. With an empty query it lists the most recently modified records. Typing filters by title on the host (`records_list` text filter) and ranks prefix and word-start matches first. Enter or a click opens the record, or switches to its tab if it is already open. Up to 50 results are shown; narrow the query to reach the rest.

## Saving

- **Save on a record** overwrites its body (`record_save`).
- **Save on a new document** (File → New graph / New notation) opens **Save new record**. You give a title and choose a store. Read-only stores are listed but disabled. The default is the last store you used, otherwise the primary.
- **Duplicate to Store…** copies the active document, or an explorer row, into any writable store as a new record.
- **Closing a dirty tab** asks whether to save, discard or cancel.

## Sample projects

Lab has no import of standalone `.json` documents. Products that ship samples build a project ahead of time with the seed tool:

```bash
cd apps/tauri/src-tauri
cargo run --example seed_project -- <project-dir> <name> <doc.json>...
```

Each `imp-graph` or `notation` document becomes a record in the new project's primary store, titled after its file name. A document's `id` is kept as the node id when it is a ULID, so records that reference each other by id still line up (`src-tauri/src/seed.rs`). Aura's Lab pack ships its demos this way as `lab-pack/demo-project/`.

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
| `records_list` | Explorer rows `{ nodeId, title, kind, corpusId, modifiedAt }` |
| `record_open` | Document DTO with `record: { nodeId, corpusId, contentDir }` |
| `record_create`, `record_save`, `record_rename`, `record_archive` | Writes |

Errors are `code: message` strings, for example `corpus_readonly: …`, `not_found: …` and `no_project: …`.
