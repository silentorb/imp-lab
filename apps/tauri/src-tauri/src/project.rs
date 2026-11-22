//! Projects: a Tome v2 config (`dataStores` + `sync`) that lists the data sources Lab works
//! with. Opening a project opens a [`TomeSession`] over every configured store; there is no
//! "open file" any more.
//!
//! The config file is the same shape TS Tome reads, so a project can be served by
//! `tome-server` as is. Lab overrides only the query cache location at open time (into the OS
//! cache dir, keyed by project path) so caches stay out of the project and out of git.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tome_db::{
    normalize_server_config, open_session, parse_server_config, ChangeEvent, ChangeOrigin,
    ConfigEnv, DbError, NodeFilter, OpenSessionOptions, StoreKind, TomeSession,
};
use tome_graph_interfaces::{js_json_file, CorpusAccess};

use crate::lab_model;

/// Default project config file name.
pub const PROJECT_FILE_NAME: &str = "imp-project.json";
/// Event emitted to the frontend whenever records change (local writes or external edits).
pub const RECORDS_CHANGED_EVENT: &str = "records://changed";
/// Environment variable overriding the cache directory (tests / tooling).
pub const CACHE_DIR_ENV: &str = "IMP_CACHE_DIR";

const POLL_INTERVAL: Duration = Duration::from_millis(400);

/// `code: message` error string for the frontend.
pub fn db_error(err: DbError) -> String {
    format!("{}: {err}", err.code())
}

/// An open project.
pub struct ProjectSession {
    pub config_path: PathBuf,
    pub session: Arc<TomeSession>,
}

/// Managed Tauri state: the open project, if any.
#[derive(Default)]
pub struct ProjectState {
    inner: Mutex<Option<ProjectSession>>,
}

impl ProjectState {
    /// The open session (cloned handle; does not hold the lock).
    pub fn session(&self) -> Result<Arc<TomeSession>, String> {
        self.inner
            .lock()
            .map_err(|_| "project lock poisoned".to_string())?
            .as_ref()
            .map(|p| p.session.clone())
            .ok_or_else(|| "no_project: open or create a project first".to_string())
    }

    fn replace(&self, next: Option<ProjectSession>) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "project lock poisoned".to_string())?;
        if let Some(old) = guard.as_ref() {
            old.session.stop_watching();
        }
        *guard = next;
        Ok(())
    }

    fn config_path(&self) -> Result<PathBuf, String> {
        self.inner
            .lock()
            .map_err(|_| "project lock poisoned".to_string())?
            .as_ref()
            .map(|p| p.config_path.clone())
            .ok_or_else(|| "no_project: open or create a project first".to_string())
    }

    fn info(&self) -> Result<Option<ProjectInfo>, String> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| "project lock poisoned".to_string())?;
        guard.as_ref().map(project_info).transpose()
    }
}

/// One configured data store, for the explorer and the save dialog.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoreInfo {
    pub id: String,
    pub label: String,
    /// `readwrite` | `readonly`.
    pub access: String,
    pub content_path: String,
    /// First configured flatfile store: default target, holds the shared model.
    pub primary: bool,
    pub record_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub config_path: String,
    pub name: String,
    pub stores: Vec<StoreInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordsChangedDto {
    /// `local` | `external`.
    pub origin: &'static str,
    pub node_ids: Vec<String>,
    pub deleted_node_ids: Vec<String>,
    pub model_changed: bool,
}

impl From<&ChangeEvent> for RecordsChangedDto {
    fn from(e: &ChangeEvent) -> Self {
        Self {
            origin: match e.origin {
                ChangeOrigin::Local => "local",
                ChangeOrigin::External => "external",
            },
            node_ids: e.node_ids.clone(),
            deleted_node_ids: e.deleted_node_ids.clone(),
            model_changed: e.model_changed,
        }
    }
}

fn access_str(access: CorpusAccess) -> &'static str {
    match access {
        CorpusAccess::Readwrite => "readwrite",
        CorpusAccess::Readonly => "readonly",
    }
}

/// Project summary: name plus every flatfile store in config order.
pub fn project_info(project: &ProjectSession) -> Result<ProjectInfo, String> {
    let session = &project.session;
    let labels = session.list_corpora().map_err(db_error)?;
    let records = session
        .list_nodes(&NodeFilter::default())
        .map_err(db_error)?;
    let primary_id = session.store().primary().corpus_id.clone();
    let stores: Vec<StoreInfo> = session
        .store()
        .corpus_stores()
        .iter()
        .map(|store| {
            let label = labels
                .iter()
                .find(|c| c.id == store.corpus_id)
                .map(|c| c.label.clone())
                .unwrap_or_else(|| store.corpus_id.clone());
            StoreInfo {
                id: store.corpus_id.clone(),
                label,
                access: access_str(store.access).into(),
                content_path: store.content_dir.display().to_string(),
                primary: store.corpus_id == primary_id,
                record_count: records
                    .iter()
                    .filter(|r| {
                        r.corpus_id.as_deref() == Some(store.corpus_id.as_str())
                            && lab_model::kind_for_tables(&r.type_table_ids).is_some()
                    })
                    .count(),
            }
        })
        .collect();
    let name = stores
        .iter()
        .find(|s| s.primary && s.label != s.id)
        .map(|s| s.label.clone())
        .or_else(|| {
            project
                .config_path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "Project".into());
    Ok(ProjectInfo {
        config_path: project.config_path.display().to_string(),
        name,
        stores,
    })
}

/// FNV-1a, for a stable cache file name per project path.
fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// OS cache directory for project caches (`IMP_CACHE_DIR` overrides).
pub fn default_cache_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var(CACHE_DIR_ENV) {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    directories::ProjectDirs::from("com", "Silent Orb", "imp-lab")
        .map(|d| d.cache_dir().join("projects"))
}

fn cache_path_for(config_path: &Path, cache_dir: &Path) -> PathBuf {
    let key = std::fs::canonicalize(config_path)
        .unwrap_or_else(|_| config_path.to_path_buf())
        .display()
        .to_string();
    cache_dir.join(format!("{:016x}.sqlite", fnv1a(&key)))
}

fn read_config(config_path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(config_path)
        .map_err(|e| format!("io: failed to read {}: {e}", config_path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("config: invalid JSON: {e}"))
}

fn write_config(config_path: &Path, config: &Value) -> Result<(), String> {
    tome_flatfile::atomic_write(config_path, &js_json_file(config)).map_err(|e| format!("io: {e}"))
}

/// The query (SQLite) store id named by `sync.queryStoreId`, else the first `tome-sqlite`.
fn query_store_id(config: &Value) -> Option<String> {
    if let Some(id) = config
        .pointer("/sync/queryStoreId")
        .and_then(|v| v.as_str())
    {
        return Some(id.to_string());
    }
    config
        .get("dataStores")?
        .as_object()?
        .iter()
        .find(|(_, e)| {
            e.get("module")
                .and_then(|m| m.as_str())
                .is_some_and(|m| m.contains("sqlite") && !m.contains("search"))
        })
        .map(|(k, _)| k.clone())
}

/// Open a project config. `cache_dir` holds the per-project query cache.
pub fn open_project(config_path: &Path, cache_dir: &Path) -> Result<ProjectSession, String> {
    let config_path = std::fs::canonicalize(config_path)
        .map_err(|e| format!("io: {}: {e}", config_path.display()))?;
    let mut raw = read_config(&config_path)?;
    if let Some(id) = query_store_id(&raw) {
        let cache = cache_path_for(&config_path, cache_dir);
        if let Some(parent) = cache.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("io: {e}"))?;
        }
        if let Some(options) = raw
            .pointer_mut(&format!("/dataStores/{id}"))
            .and_then(|e| e.as_object_mut())
            .map(|e| e.entry("options").or_insert_with(|| json!({})))
            .and_then(|o| o.as_object_mut())
        {
            options.insert("dbPath".into(), Value::String(cache.display().to_string()));
        }
    }
    let env = ConfigEnv {
        cwd: config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        ..Default::default()
    };
    let parsed = parse_server_config(&raw).map_err(db_error)?;
    let config = normalize_server_config(&parsed, &env).map_err(db_error)?;
    if config.ids_of(StoreKind::Flatfile).is_empty() {
        return Err("config: the project has no flatfile data store".into());
    }
    let session = open_session(&config, &env, &OpenSessionOptions::default()).map_err(db_error)?;
    lab_model::ensure_lab_model(&session).map_err(db_error)?;
    Ok(ProjectSession {
        config_path,
        session: Arc::new(session),
    })
}

fn flatfile_entry(id: &str, content_path: &str, access: &str) -> Value {
    json!({
        "id": id,
        "module": "tome-flatfile",
        "export": "createFlatfileModule",
        "options": { "contentPath": content_path, "access": access }
    })
}

/// Write a new project into `dir` (created if needed): config, one read/write store at
/// `./content` bootstrapped with the lab model. Returns the config path.
pub fn create_project_files(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let config_path = dir.join(PROJECT_FILE_NAME);
    if config_path.exists() {
        return Err(format!(
            "exists: {} already contains a project",
            dir.display()
        ));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("io: {e}"))?;
    let config = json!({
        "version": 2,
        "dataStores": {
            "main": flatfile_entry("main", "content", "readwrite"),
            "cache": { "id": "cache", "module": "tome-sqlite", "export": "createSqliteModule",
                       "options": { "dbPath": ".tome/cache.sqlite" } }
        },
        "sync": { "queryStoreId": "cache" },
        "services": []
    });
    write_config(&config_path, &config)?;
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, ".tome/\n").map_err(|e| format!("io: {e}"))?;
    }
    let content = dir.join("content");
    let store = tome_flatfile::ContentStore::open(&content, Some("main"), CorpusAccess::Readwrite)
        .map_err(|e| format!("flatfile: {e}"))?;
    for (file, text) in lab_model::new_project_model_files(name) {
        store
            .write_model_file(file, &text)
            .map_err(|e| format!("flatfile: {e}"))?;
    }
    for node in lab_model::new_project_nodes() {
        store
            .write_node(&node, None)
            .map_err(|e| format!("flatfile: {e}"))?;
    }
    Ok(config_path)
}

fn valid_store_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Add a flatfile store to a project config. `content_path` inside the project directory is
/// stored relative to it.
pub fn add_store_to_config(
    config_path: &Path,
    id: &str,
    content_path: &Path,
    access: &str,
) -> Result<(), String> {
    let id = id.trim();
    if !valid_store_id(id) {
        return Err("invalid_store_id: use letters, digits, - or _".into());
    }
    if !matches!(access, "readwrite" | "readonly") {
        return Err("invalid_access: expected readwrite or readonly".into());
    }
    let mut config = read_config(config_path)?;
    let stores = config
        .get_mut("dataStores")
        .and_then(|s| s.as_object_mut())
        .ok_or_else(|| "config: dataStores missing".to_string())?;
    if stores.contains_key(id) {
        return Err(format!(
            "store_exists: a store named \"{id}\" already exists"
        ));
    }
    let project_dir = config_path.parent().unwrap_or(Path::new("."));
    let stored = match content_path.strip_prefix(project_dir) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.to_string_lossy().replace('\\', "/"),
        _ => content_path.display().to_string(),
    };
    // Insert flatfile stores before caches so config order stays "sources, then sinks".
    let mut next = serde_json::Map::new();
    let mut inserted = false;
    for (key, entry) in stores.iter() {
        let is_flatfile = entry
            .get("module")
            .and_then(|m| m.as_str())
            .is_some_and(|m| m.contains("flatfile"));
        if !inserted && !is_flatfile {
            next.insert(id.to_string(), flatfile_entry(id, &stored, access));
            inserted = true;
        }
        next.insert(key.clone(), entry.clone());
    }
    if !inserted {
        next.insert(id.to_string(), flatfile_entry(id, &stored, access));
    }
    *stores = next;
    write_config(config_path, &config)
}

/// Remove a non-primary flatfile store from a project config (its files stay on disk).
pub fn remove_store_from_config(config_path: &Path, id: &str) -> Result<(), String> {
    let mut config = read_config(config_path)?;
    let stores = config
        .get_mut("dataStores")
        .and_then(|s| s.as_object_mut())
        .ok_or_else(|| "config: dataStores missing".to_string())?;
    let first_flatfile = stores
        .iter()
        .find(|(_, e)| {
            e.get("module")
                .and_then(|m| m.as_str())
                .is_some_and(|m| m.contains("flatfile"))
        })
        .map(|(k, _)| k.clone());
    if first_flatfile.as_deref() == Some(id) {
        return Err("primary_store: the primary store cannot be removed".into());
    }
    let is_flatfile = stores
        .get(id)
        .and_then(|e| e.get("module"))
        .and_then(|m| m.as_str())
        .is_some_and(|m| m.contains("flatfile"));
    if !is_flatfile {
        return Err(format!("store_not_found: no data store \"{id}\""));
    }
    stores.remove(id);
    write_config(config_path, &config)
}

fn cache_dir() -> Result<PathBuf, String> {
    default_cache_dir().ok_or_else(|| "io: no cache directory available".to_string())
}

/// Open a project, make it current, watch its stores and forward change events.
fn activate(
    app: &AppHandle,
    state: &ProjectState,
    config_path: &Path,
) -> Result<ProjectInfo, String> {
    // Release the old session (and its cache file lock) before opening the new one.
    state.replace(None)?;
    let project = open_project(config_path, &cache_dir()?)?;
    let emitter = app.clone();
    project.session.subscribe(move |event| {
        let _ = emitter.emit(RECORDS_CHANGED_EVENT, RecordsChangedDto::from(event));
    });
    if let Err(err) = project.session.start_watching() {
        eprintln!("imp-lab: file watching unavailable: {err}");
    }
    let info = project_info(&project)?;
    state.replace(Some(project))?;
    Ok(info)
}

/// Background poller: applies external file edits for the open project.
pub fn spawn_change_poller(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(POLL_INTERVAL);
        let state = app.state::<ProjectState>();
        let Ok(session) = state.session() else {
            continue;
        };
        if let Err(err) = session.pump_external_changes() {
            eprintln!("imp-lab: external change sync failed: {err}");
        }
    });
}

#[tauri::command]
pub fn project_current(state: State<'_, ProjectState>) -> Result<Option<ProjectInfo>, String> {
    state.info()
}

#[tauri::command]
pub fn project_open(
    app: AppHandle,
    state: State<'_, ProjectState>,
    path: String,
) -> Result<ProjectInfo, String> {
    activate(&app, &state, Path::new(&path))
}

/// Native file dialog for a project config, then open it.
#[tauri::command]
pub fn project_pick_and_open(
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<Option<ProjectInfo>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Imp project / Tome config", &["json"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|e| format!("io: could not resolve picked path: {e}"))?;
    activate(&app, &state, &path).map(Some)
}

/// Native folder dialog, create a project there, then open it.
#[tauri::command]
pub fn project_pick_and_create(
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<Option<ProjectInfo>, String> {
    let picked = app.dialog().file().blocking_pick_folder();
    let Some(folder) = picked else {
        return Ok(None);
    };
    let dir = folder
        .into_path()
        .map_err(|e| format!("io: could not resolve picked folder: {e}"))?;
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Project".into());
    let config_path = create_project_files(&dir, &name)?;
    activate(&app, &state, &config_path).map(Some)
}

#[tauri::command]
pub fn project_close(state: State<'_, ProjectState>) -> Result<(), String> {
    state.replace(None)
}

/// Native folder dialog for a store's content root (Manage stores).
#[tauri::command]
pub fn project_pick_store_folder(app: AppHandle) -> Result<Option<String>, String> {
    let picked = app.dialog().file().blocking_pick_folder();
    let Some(folder) = picked else {
        return Ok(None);
    };
    folder
        .into_path()
        .map(|p| Some(p.display().to_string()))
        .map_err(|e| format!("io: could not resolve picked folder: {e}"))
}

#[tauri::command]
pub fn project_add_store(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    content_path: String,
    access: String,
) -> Result<ProjectInfo, String> {
    let config_path = state.config_path()?;
    add_store_to_config(&config_path, &id, Path::new(&content_path), &access)?;
    activate(&app, &state, &config_path)
}

#[tauri::command]
pub fn project_remove_store(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
) -> Result<ProjectInfo, String> {
    let config_path = state.config_path()?;
    remove_store_from_config(&config_path, &id)?;
    activate(&app, &state, &config_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_open_add_remove_store() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("My Lab");
        let cache = tmp.path().join("cache");
        let config_path = create_project_files(&dir, "My Lab").unwrap();
        assert!(create_project_files(&dir, "again").is_err());

        let project = open_project(&config_path, &cache).unwrap();
        let info = project_info(&project).unwrap();
        assert_eq!(info.name, "My Lab");
        assert_eq!(info.stores.len(), 1);
        assert!(info.stores[0].primary);
        assert_eq!(info.stores[0].access, "readwrite");
        assert!(!dir.join(".tome").exists(), "cache lives in the cache dir");
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1);
        drop(project);

        let lib = tmp.path().join("shared-lib");
        add_store_to_config(&config_path, "lib", &lib, "readonly").unwrap();
        add_store_to_config(&config_path, "local", &dir.join("more"), "readwrite").unwrap();
        assert!(add_store_to_config(&config_path, "lib", &lib, "readonly").is_err());
        assert!(add_store_to_config(&config_path, "bad id", &lib, "readonly").is_err());
        let raw = read_config(&config_path).unwrap();
        assert_eq!(
            raw["dataStores"]["local"]["options"]["contentPath"],
            json!("more")
        );
        let keys: Vec<&String> = raw["dataStores"].as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["main", "lib", "local", "cache"]);

        let project = open_project(&config_path, &cache).unwrap();
        let info = project_info(&project).unwrap();
        let ids: Vec<&str> = info.stores.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["main", "lib", "local"]);
        assert_eq!(info.stores[1].access, "readonly");
        drop(project);

        assert!(remove_store_from_config(&config_path, "main").is_err());
        remove_store_from_config(&config_path, "lib").unwrap();
        let project = open_project(&config_path, &cache).unwrap();
        assert_eq!(project_info(&project).unwrap().stores.len(), 2);
    }

    #[test]
    fn existing_tome_corpus_gets_lab_tables() {
        let tmp = tempfile::tempdir().unwrap();
        let content = tmp.path().join("content");
        let store =
            tome_flatfile::ContentStore::open(&content, Some("main"), CorpusAccess::Readwrite)
                .unwrap();
        // A plain Tome corpus with one set type and no lab tables.
        store
            .write_model_file(
                tome_flatfile::ASSOCIATIONS_FILENAME,
                &js_json_file(&json!({ "version": 1, "associations": {
                    "01M5B0000000000000000000A0": { "perspectives": ["Members", "Membership"], "traits": ["Set"] }
                } })),
            )
            .unwrap();
        let config_path = tmp.path().join(PROJECT_FILE_NAME);
        write_config(
            &config_path,
            &json!({ "version": 2, "dataStores": {
                "main": flatfile_entry("main", "content", "readwrite"),
                "cache": { "id": "cache", "module": "tome-sqlite", "export": "createSqliteModule", "options": {} }
            }, "sync": { "queryStoreId": "cache" }, "services": [] }),
        )
        .unwrap();
        let project = open_project(&config_path, &tmp.path().join("cache")).unwrap();
        let tables = project.session.list_type_tables().unwrap();
        assert!(tables.iter().any(|t| t.id == lab_model::IMP_GRAPH_TABLE_ID));
        assert!(tables.iter().any(|t| t.id == lab_model::NOTATION_TABLE_ID));
    }
}
