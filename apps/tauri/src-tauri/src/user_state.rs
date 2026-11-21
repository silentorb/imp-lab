//! Persisted UI chrome (JSON buckets under the OS app config dir).
//!
//! This is editor UI state (tabs, piano prefs, window geometry) — not a general
//! settings or configuration framework.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State, WindowEvent};

/// Environment variable overriding the UI-state directory (tests / tooling).
pub const USER_DIR_ENV: &str = "IMP_USER_DIR";

/// Supported bucket document schema version.
pub const BUCKET_SCHEMA_VERSION: u32 = 1;

/// Debounce idle period before writing dirty buckets.
pub const DEBOUNCE: Duration = Duration::from_millis(500);

/// Entry id → bucket file stem.
pub const ENTRY_OPEN_DOCUMENTS: &str = "open-documents";
pub const ENTRY_PIANO_ROLL_UI: &str = "piano-roll-ui";
pub const ENTRY_WINDOW: &str = "window";

pub const BUCKET_DOCUMENTS: &str = "documents";
pub const BUCKET_UI: &str = "ui";
pub const BUCKET_WINDOW: &str = "window";

const KNOWN_BUCKETS: &[&str] = &[BUCKET_DOCUMENTS, BUCKET_UI, BUCKET_WINDOW];

/// Failure while reading or writing UI-state buckets.
#[derive(Debug)]
pub enum UserStateError {
    /// Filesystem error.
    Io { path: PathBuf, source: io::Error },
    /// JSON encode failure.
    Encode(String),
}

impl std::fmt::Display for UserStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "user-state I/O error at {}: {source}", path.display())
            }
            Self::Encode(msg) => write!(f, "user-state JSON encode error: {msg}"),
        }
    }
}

impl std::error::Error for UserStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Encode(_) => None,
        }
    }
}

/// Resolved UI-state directory root (OS app config dir by default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDir {
    path: PathBuf,
}

impl UserDir {
    /// Resolves from `IMP_USER_DIR` when set, otherwise the Silent Orb / imp-lab config dir.
    pub fn resolve() -> Option<Self> {
        if let Ok(override_path) = std::env::var(USER_DIR_ENV) {
            let path = PathBuf::from(override_path);
            if !path.as_os_str().is_empty() {
                return Some(Self { path });
            }
        }
        directories::ProjectDirs::from("com", "Silent Orb", "imp-lab")
            .map(|dirs| Self {
                path: dirs.config_dir().to_path_buf(),
            })
    }

    /// Explicit directory (tests and tooling).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Directory path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Path of the JSON file for `bucket`.
    pub fn bucket_path(&self, bucket: &str) -> PathBuf {
        self.path.join(format!("{bucket}.json"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct BucketDocument {
    version: u32,
    #[serde(default)]
    entries: BTreeMap<String, Value>,
}

impl Default for BucketDocument {
    fn default() -> Self {
        Self {
            version: BUCKET_SCHEMA_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

/// In-memory UI-state store with optional on-disk bucket persistence.
#[derive(Debug, Clone)]
pub struct UserStateStore {
    user_dir: Option<UserDir>,
    buckets: HashMap<String, BucketDocument>,
    dirty: HashSet<String>,
    generation: u64,
}

impl Default for UserStateStore {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl UserStateStore {
    /// Store that never reads or writes the filesystem (tests / headless helpers).
    pub fn in_memory() -> Self {
        Self {
            user_dir: None,
            buckets: HashMap::new(),
            dirty: HashSet::new(),
            generation: 0,
        }
    }

    /// Store rooted at `user_dir` (call [`Self::load_known`] to read disk).
    pub fn open(user_dir: UserDir) -> Self {
        Self {
            user_dir: Some(user_dir),
            buckets: HashMap::new(),
            dirty: HashSet::new(),
            generation: 0,
        }
    }

    /// Open the resolved config dir (or in-memory if ProjectDirs is unavailable).
    pub fn open_default() -> Self {
        match UserDir::resolve() {
            Some(dir) => {
                let mut store = Self::open(dir);
                store.load_known();
                store
            }
            None => {
                eprintln!("imp-lab user-state: no config dir; using in-memory store");
                Self::in_memory()
            }
        }
    }

    /// Whether this store writes buckets to disk.
    pub fn persists_to_disk(&self) -> bool {
        self.user_dir.is_some()
    }

    /// User directory when persistence is enabled.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn user_dir(&self) -> Option<&UserDir> {
        self.user_dir.as_ref()
    }

    /// Monotonic generation bumped on each mutating `set_entry`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether any bucket is dirty.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }

    /// Load known UI-state bucket files (missing/invalid → empty).
    pub fn load_known(&mut self) {
        let Some(user_dir) = self.user_dir.clone() else {
            return;
        };
        for bucket in KNOWN_BUCKETS {
            let path = user_dir.bucket_path(bucket);
            let document = load_bucket_file(&path);
            self.buckets.insert((*bucket).to_string(), document);
        }
    }

    /// Bucket id for a known entry id.
    pub fn bucket_for_entry(entry_id: &str) -> Option<&'static str> {
        match entry_id {
            ENTRY_OPEN_DOCUMENTS => Some(BUCKET_DOCUMENTS),
            ENTRY_PIANO_ROLL_UI => Some(BUCKET_UI),
            ENTRY_WINDOW => Some(BUCKET_WINDOW),
            _ => None,
        }
    }

    /// Deserializes an entry when present and valid.
    pub fn get_entry<T: DeserializeOwned>(&self, bucket: &str, entry_id: &str) -> Option<T> {
        let value = self.buckets.get(bucket)?.entries.get(entry_id)?;
        match serde_json::from_value(value.clone()) {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!(
                    "imp-lab user-state: ignoring invalid entry {entry_id} in {bucket}: {error}"
                );
                None
            }
        }
    }

    /// Raw JSON for an entry (commands / frontend).
    pub fn get_entry_value(&self, entry_id: &str) -> Option<Value> {
        let bucket = Self::bucket_for_entry(entry_id)?;
        self.buckets
            .get(bucket)
            .and_then(|doc| doc.entries.get(entry_id).cloned())
    }

    /// Updates an entry in memory, marks the bucket dirty, and bumps generation.
    ///
    /// Returns the new generation (for debounced flush tokens).
    pub fn set_entry<T: Serialize>(
        &mut self,
        bucket: &str,
        entry_id: &str,
        value: &T,
    ) -> Result<u64, UserStateError> {
        let encoded = serde_json::to_value(value)
            .map_err(|error| UserStateError::Encode(error.to_string()))?;
        self.set_entry_value(bucket, entry_id, encoded)
    }

    /// Updates an entry from raw JSON.
    pub fn set_entry_value(
        &mut self,
        bucket: &str,
        entry_id: &str,
        encoded: Value,
    ) -> Result<u64, UserStateError> {
        let document = self.buckets.entry(bucket.to_string()).or_default();
        if document.entries.get(entry_id) == Some(&encoded) {
            return Ok(self.generation);
        }
        document.version = BUCKET_SCHEMA_VERSION;
        document.entries.insert(entry_id.to_string(), encoded);
        self.dirty.insert(bucket.to_string());
        self.generation = self.generation.wrapping_add(1);
        Ok(self.generation)
    }

    /// Writes all dirty buckets immediately.
    pub fn flush_now(&mut self) -> Result<(), UserStateError> {
        let Some(user_dir) = self.user_dir.clone() else {
            self.dirty.clear();
            return Ok(());
        };
        if self.dirty.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(user_dir.path()).map_err(|source| UserStateError::Io {
            path: user_dir.path().to_path_buf(),
            source,
        })?;
        let dirty: Vec<String> = self.dirty.iter().cloned().collect();
        for bucket in dirty {
            let document = self.buckets.get(&bucket).cloned().unwrap_or_default();
            let path = user_dir.bucket_path(&bucket);
            write_bucket_file(&path, &document)?;
            self.dirty.remove(&bucket);
        }
        Ok(())
    }

    /// Flushes when `generation` still matches (debounce coalescing).
    pub fn flush_if_generation(&mut self, generation: u64) -> Result<(), UserStateError> {
        if generation != self.generation {
            return Ok(());
        }
        self.flush_now()
    }
}

/// Open-document tab snapshot (`documents` bucket / `open-documents` entry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(test), allow(dead_code))]
pub struct OpenDocumentsEntry {
    /// Open filesystem paths in tab order (untitled omitted).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_paths: Vec<String>,
    /// Active filesystem path when one is selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_path: Option<String>,
}

/// Piano-roll tool / snap prefs (`ui` bucket / `piano-roll-ui` entry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(test), allow(dead_code))]
pub struct PianoRollUiEntry {
    /// Active tool: `draw` | `select` | `erase`.
    pub active_tool: String,
    /// Whether snap-to-grid is enabled.
    pub snap_enabled: bool,
    /// Snap resolution: `1/4` | `1/8` | `1/16` | `1/32`.
    pub snap_resolution: String,
}

impl Default for PianoRollUiEntry {
    fn default() -> Self {
        Self {
            active_tool: "draw".into(),
            snap_enabled: true,
            snap_resolution: "1/16".into(),
        }
    }
}

/// Main window geometry (`window` bucket / `window` entry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowEntry {
    /// Outer width in physical pixels.
    pub width: u32,
    /// Outer height in physical pixels.
    pub height: u32,
    /// Outer x position when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    /// Outer y position when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    /// Whether the window is maximized.
    #[serde(default)]
    pub maximized: bool,
}

/// Writes `data` via a temporary sibling file, then renames over `path`.
pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp_path = atomic_temp_path(path);
    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    PathBuf::from(tmp)
}

fn load_bucket_file(path: &Path) -> BucketDocument {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<BucketDocument>(&text) {
            Ok(document) if document.version == BUCKET_SCHEMA_VERSION => document,
            Ok(document) => {
                eprintln!(
                    "imp-lab user-state: unsupported bucket version {} at {}; using empty",
                    document.version,
                    path.display()
                );
                BucketDocument::default()
            }
            Err(error) => {
                eprintln!(
                    "imp-lab user-state: invalid bucket JSON at {}: {error}; using empty",
                    path.display()
                );
                BucketDocument::default()
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => BucketDocument::default(),
        Err(error) => {
            eprintln!(
                "imp-lab user-state: failed to read {}: {error}; using empty",
                path.display()
            );
            BucketDocument::default()
        }
    }
}

fn write_bucket_file(path: &Path, document: &BucketDocument) -> Result<(), UserStateError> {
    let json = serde_json::to_vec_pretty(document)
        .map_err(|error| UserStateError::Encode(error.to_string()))?;
    write_atomic(path, &json).map_err(|source| UserStateError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Schedule a debounced flush for the managed store.
pub fn schedule_flush(app: &AppHandle, generation: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(DEBOUNCE);
        let Some(state) = app.try_state::<Mutex<UserStateStore>>() else {
            return;
        };
        let Ok(mut store) = state.lock() else {
            return;
        };
        if let Err(err) = store.flush_if_generation(generation) {
            eprintln!("imp-lab user-state: flush failed: {err}");
        }
    });
}

/// Apply a saved window entry to the main window (best-effort).
pub fn apply_window_entry(app: &AppHandle, entry: &WindowEntry) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if entry.maximized {
        if let Err(err) = window.maximize() {
            eprintln!("imp-lab user-state: maximize failed: {err}");
        }
        return;
    }
    if entry.width > 0 && entry.height > 0 {
        let size = PhysicalSize::new(entry.width, entry.height);
        if let Err(err) = window.set_size(tauri::Size::Physical(size)) {
            eprintln!("imp-lab user-state: set_size failed: {err}");
        }
    }
    if let (Some(x), Some(y)) = (entry.x, entry.y) {
        let position = PhysicalPosition::new(x, y);
        if let Err(err) = window.set_position(tauri::Position::Physical(position)) {
            eprintln!("imp-lab user-state: set_position failed: {err}");
        }
    }
}

/// Capture current main-window geometry into the store and schedule a flush.
pub fn capture_window_state(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let position = window.outer_position().ok();
    let maximized = window.is_maximized().unwrap_or(false);
    let entry = WindowEntry {
        width: size.width,
        height: size.height,
        x: position.map(|p| p.x),
        y: position.map(|p| p.y),
        maximized,
    };
    let Some(state) = app.try_state::<Mutex<UserStateStore>>() else {
        return;
    };
    let Ok(mut store) = state.lock() else {
        return;
    };
    match store.set_entry(BUCKET_WINDOW, ENTRY_WINDOW, &entry) {
        Ok(generation) => {
            if store.persists_to_disk() {
                schedule_flush(app, generation);
            }
        }
        Err(err) => eprintln!("imp-lab user-state: window capture failed: {err}"),
    }
}

/// Flush dirty buckets immediately (close path).
pub fn flush_store(app: &AppHandle) {
    let Some(state) = app.try_state::<Mutex<UserStateStore>>() else {
        return;
    };
    let Ok(mut store) = state.lock() else {
        return;
    };
    if let Err(err) = store.flush_now() {
        eprintln!("imp-lab user-state: flush on close failed: {err}");
    }
}

/// Handle window events that should update or flush UI state.
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    let app = window.app_handle();
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            capture_window_state(app);
        }
        WindowEvent::CloseRequested { .. } => {
            flush_store(app);
        }
        _ => {}
    }
}

/// Read a UI-state entry as JSON for the frontend.
#[tauri::command]
pub fn get_user_state_entry(
    store: State<'_, Mutex<UserStateStore>>,
    entry_id: String,
) -> Result<Option<Value>, String> {
    if UserStateStore::bucket_for_entry(&entry_id).is_none() {
        return Err(format!("unknown user-state entry \"{entry_id}\""));
    }
    let guard = store
        .lock()
        .map_err(|_| "user-state store lock poisoned".to_string())?;
    Ok(guard.get_entry_value(&entry_id))
}

/// Write a UI-state entry from the frontend and schedule a debounced flush.
#[tauri::command]
pub fn set_user_state_entry(
    app: AppHandle,
    store: State<'_, Mutex<UserStateStore>>,
    entry_id: String,
    value: Value,
) -> Result<(), String> {
    let bucket = UserStateStore::bucket_for_entry(&entry_id)
        .ok_or_else(|| format!("unknown user-state entry \"{entry_id}\""))?;
    let mut guard = store
        .lock()
        .map_err(|_| "user-state store lock poisoned".to_string())?;
    let generation = guard
        .set_entry_value(bucket, &entry_id, value)
        .map_err(|err| err.to_string())?;
    if guard.persists_to_disk() {
        schedule_flush(&app, generation);
    }
    Ok(())
}

/// Force a flush (tests / tooling).
#[tauri::command]
pub fn flush_user_state(store: State<'_, Mutex<UserStateStore>>) -> Result<(), String> {
    let mut guard = store
        .lock()
        .map_err(|_| "user-state store lock poisoned".to_string())?;
    guard.flush_now().map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_user_dir(label: &str) -> UserDir {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("imp-lab-user-state-{label}-{nanos}"));
        let _ = fs::remove_dir_all(&path);
        UserDir::from_path(path)
    }

    #[test]
    fn atomic_replace_round_trips() {
        let dir = temp_user_dir("atomic");
        fs::create_dir_all(dir.path()).unwrap();
        let path = dir.bucket_path("documents");
        write_atomic(&path, br#"{"version":1,"entries":{}}"#).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{"version":1,"entries":{}}"#
        );
        write_atomic(&path, br#"{"version":1,"entries":{"a":1}}"#).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{"version":1,"entries":{"a":1}}"#
        );
        let _ = fs::remove_dir_all(dir.path());
    }

    #[test]
    fn flush_now_writes_dirty_buckets() {
        let dir = temp_user_dir("flush");
        let mut store = UserStateStore::open(dir.clone());
        let entry = OpenDocumentsEntry {
            open_paths: vec!["/tmp/a.json".into()],
            active_path: Some("/tmp/a.json".into()),
        };
        let gen = store
            .set_entry(BUCKET_DOCUMENTS, ENTRY_OPEN_DOCUMENTS, &entry)
            .unwrap();
        assert!(gen > 0);
        assert!(store.is_dirty());
        store.flush_now().unwrap();
        assert!(!store.is_dirty());

        let mut reloaded = UserStateStore::open(dir.clone());
        reloaded.load_known();
        let loaded: OpenDocumentsEntry = reloaded
            .get_entry(BUCKET_DOCUMENTS, ENTRY_OPEN_DOCUMENTS)
            .expect("entry");
        assert_eq!(loaded, entry);
        let _ = fs::remove_dir_all(dir.path());
    }

    #[test]
    fn flush_if_generation_coalesces() {
        let dir = temp_user_dir("coalesce");
        let mut store = UserStateStore::open(dir.clone());
        let first = store
            .set_entry(
                BUCKET_DOCUMENTS,
                ENTRY_OPEN_DOCUMENTS,
                &OpenDocumentsEntry {
                    open_paths: vec!["/a".into()],
                    active_path: None,
                },
            )
            .unwrap();
        let second = store
            .set_entry(
                BUCKET_DOCUMENTS,
                ENTRY_OPEN_DOCUMENTS,
                &OpenDocumentsEntry {
                    open_paths: vec!["/b".into()],
                    active_path: None,
                },
            )
            .unwrap();
        assert_ne!(first, second);
        store.flush_if_generation(first).unwrap();
        assert!(store.is_dirty());
        store.flush_if_generation(second).unwrap();
        assert!(!store.is_dirty());
        let text = fs::read_to_string(dir.bucket_path("documents")).unwrap();
        assert!(text.contains("/b"));
        assert!(!text.contains("\"/a\""));
        let _ = fs::remove_dir_all(dir.path());
    }

    #[test]
    fn missing_and_invalid_buckets_load_empty() {
        let dir = temp_user_dir("empty");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.bucket_path("documents"), "not-json").unwrap();
        let mut store = UserStateStore::open(dir.clone());
        store.load_known();
        let entry: Option<OpenDocumentsEntry> =
            store.get_entry(BUCKET_DOCUMENTS, ENTRY_OPEN_DOCUMENTS);
        assert!(entry.is_none());
        let _ = fs::remove_dir_all(dir.path());
    }

    #[test]
    fn piano_and_window_round_trip() {
        let dir = temp_user_dir("prefs");
        let mut store = UserStateStore::open(dir.clone());
        let piano = PianoRollUiEntry {
            active_tool: "select".into(),
            snap_enabled: false,
            snap_resolution: "1/8".into(),
        };
        let window = WindowEntry {
            width: 1280,
            height: 800,
            x: Some(40),
            y: Some(60),
            maximized: false,
        };
        store
            .set_entry(BUCKET_UI, ENTRY_PIANO_ROLL_UI, &piano)
            .unwrap();
        store
            .set_entry(BUCKET_WINDOW, ENTRY_WINDOW, &window)
            .unwrap();
        store.flush_now().unwrap();

        let mut reloaded = UserStateStore::open(dir.clone());
        reloaded.load_known();
        let loaded_piano: PianoRollUiEntry = reloaded
            .get_entry(BUCKET_UI, ENTRY_PIANO_ROLL_UI)
            .unwrap();
        let loaded_window: WindowEntry = reloaded.get_entry(BUCKET_WINDOW, ENTRY_WINDOW).unwrap();
        assert_eq!(loaded_piano, piano);
        assert_eq!(loaded_window, window);
        let _ = fs::remove_dir_all(dir.path());
    }

    #[test]
    fn in_memory_store_does_not_require_disk() {
        let mut store = UserStateStore::in_memory();
        assert!(!store.persists_to_disk());
        store
            .set_entry(
                BUCKET_DOCUMENTS,
                ENTRY_OPEN_DOCUMENTS,
                &OpenDocumentsEntry::default(),
            )
            .unwrap();
        store.flush_now().unwrap();
    }

    #[test]
    fn from_path_isolates_env_override_shape() {
        let dir = temp_user_dir("override");
        let resolved = UserDir::from_path(dir.path());
        assert_eq!(resolved.path(), dir.path());
        let store = UserStateStore::open(resolved);
        assert!(store.user_dir().is_some());
        assert_eq!(store.generation(), 0);
        let _ = fs::remove_dir_all(dir.path());
    }
}
