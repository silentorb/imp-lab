//! Configurable input→command bindings loaded from JSON (VS Code–aligned host keymap).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Bundled default bindings JSON.
pub const DEFAULT_BINDINGS_JSON: &str =
    include_str!("../assets/default-bindings.json");

const SUPPORTED_VERSION: u32 = 1;

const MOD_CTRL: u8 = 1;
const MOD_ALT: u8 = 2;
const MOD_SHIFT: u8 = 4;
const MOD_LOGO: u8 = 8;

/// Builtin host command ids (aligned with React `commands.ts` / menu).
pub const BUILTIN_COMMAND_IDS: &[&str] = &[
    "document.open-file",
    "document.new.graph",
    "document.new.notation",
    "document.save",
    "document.save-as",
    "document.close_active",
    "edit.undo",
    "edit.redo",
    "view.toggle-color-scheme",
    "notation.tool.draw",
    "notation.tool.select",
    "notation.tool.erase",
    "notation.snap.toggle",
];

static BINDINGS: OnceLock<Bindings> = OnceLock::new();

/// Runtime bindings table: input trigger → command id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings {
    key_map: HashMap<KeyBinding, String>,
    /// First binding per command (menu accelerator / DTO order).
    entries: Vec<BindingDto>,
}

/// A keyboard trigger used for lookup.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct KeyBinding {
    key: BindingKey,
    modifiers: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum BindingKey {
    Character(String),
    Named(NamedKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum NamedKey {
    Escape,
    Enter,
    Tab,
    Space,
    Backspace,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

/// Serializable binding for the React resolve table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingDto {
    pub command: String,
    pub key: String,
    pub modifiers: Vec<String>,
}

/// Payload returned by `get_input_bindings`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBindingsDto {
    pub bindings: Vec<BindingDto>,
}

#[derive(Debug, Deserialize)]
struct BindingsFile {
    version: u32,
    #[serde(default)]
    bindings: Vec<BindingFileEntry>,
}

#[derive(Debug, Deserialize)]
struct BindingFileEntry {
    command: String,
    when: TriggerFile,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TriggerFile {
    Key {
        key: String,
        #[serde(default)]
        modifiers: Vec<String>,
    },
}

impl Bindings {
    /// Empty table (no input bindings).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Load using the production chain against `known_commands`.
    pub fn load(known_commands: &HashSet<String>) -> Self {
        let user_path = user_bindings_path();
        Self::load_chain(user_path.as_deref(), DEFAULT_BINDINGS_JSON, known_commands)
    }

    /// Load with an explicit user path (tests) and a default JSON string.
    pub fn load_chain(
        user_path: Option<&Path>,
        default_json: &str,
        known_commands: &HashSet<String>,
    ) -> Self {
        if let Some(path) = user_path {
            if path.is_file() {
                match fs::read_to_string(path) {
                    Ok(contents) => match Self::from_json_str(&contents, known_commands) {
                        Ok(bindings) => {
                            eprintln!(
                                "imp-lab bindings: loaded user file {} ({} bindings)",
                                path.display(),
                                bindings.len()
                            );
                            return bindings;
                        }
                        Err(error) => {
                            eprintln!(
                                "imp-lab bindings: failed to load user file {}: {error}; trying default",
                                path.display()
                            );
                        }
                    },
                    Err(error) => {
                        eprintln!(
                            "imp-lab bindings: failed to read user file {}: {error}; trying default",
                            path.display()
                        );
                    }
                }
            }
        }

        match Self::from_json_str(default_json, known_commands) {
            Ok(bindings) => {
                eprintln!(
                    "imp-lab bindings: loaded bundled defaults ({} bindings)",
                    bindings.len()
                );
                bindings
            }
            Err(error) => {
                eprintln!(
                    "imp-lab bindings: failed to load bundled defaults ({error}); using empty"
                );
                Self::empty()
            }
        }
    }

    /// Parse bindings from a JSON string, skipping ids not in `known_commands`.
    pub fn from_json_str(
        json: &str,
        known_commands: &HashSet<String>,
    ) -> Result<Self, String> {
        let file: BindingsFile =
            serde_json::from_str(json).map_err(|e| e.to_string())?;
        if file.version != SUPPORTED_VERSION {
            return Err(format!(
                "unsupported bindings version {} (expected {SUPPORTED_VERSION})",
                file.version
            ));
        }

        let mut key_map = HashMap::new();
        let mut entries = Vec::new();
        for entry in file.bindings {
            if !known_commands.contains(&entry.command) {
                eprintln!(
                    "imp-lab bindings: skipping unknown command id `{}`",
                    entry.command
                );
                continue;
            }

            match entry.when {
                TriggerFile::Key { key, modifiers } => {
                    let binding = match parse_key_binding(&key, &modifiers) {
                        Ok(binding) => binding,
                        Err(error) => {
                            eprintln!(
                                "imp-lab bindings: skipping invalid key for `{}`: {error}",
                                entry.command
                            );
                            continue;
                        }
                    };
                    let dto = BindingDto {
                        command: entry.command.clone(),
                        key: key.trim().to_ascii_lowercase(),
                        modifiers: modifiers
                            .iter()
                            .map(|m| m.to_ascii_lowercase())
                            .collect(),
                    };
                    if key_map
                        .insert(binding, entry.command.clone())
                        .is_some()
                    {
                        eprintln!(
                            "imp-lab bindings: duplicate trigger for `{}`; last wins",
                            entry.command
                        );
                    }
                    entries.push(dto);
                }
            }
        }

        Ok(Self { key_map, entries })
    }

    /// Number of resolved bindings.
    pub fn len(&self) -> usize {
        self.key_map.len()
    }

    /// Whether the table has no bindings.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.key_map.is_empty()
    }

    /// Entries for the frontend resolve table (file order).
    pub fn entries(&self) -> &[BindingDto] {
        &self.entries
    }

    /// Tauri/muda accelerator string for the first binding of `command`, if any.
    pub fn accelerator_for_command(&self, command: &str) -> Option<String> {
        self.entries
            .iter()
            .find(|e| e.command == command)
            .and_then(|e| format_accelerator(&e.key, &e.modifiers))
    }
}

/// Platform config path for the optional user bindings file.
pub fn user_bindings_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "Silent Orb", "imp-lab")
        .map(|dirs| dirs.config_dir().join("bindings.json"))
}

/// Builtin ids plus extension-contributed command ids.
pub fn known_command_ids(extension_command_ids: &[String]) -> HashSet<String> {
    let mut known: HashSet<String> = BUILTIN_COMMAND_IDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    for id in extension_command_ids {
        known.insert(id.clone());
    }
    known
}

/// Load once and cache for menu build + `get_input_bindings`.
pub fn ensure_loaded(extension_command_ids: &[String]) -> &'static Bindings {
    BINDINGS.get_or_init(|| {
        let known = known_command_ids(extension_command_ids);
        Bindings::load(&known)
    })
}

/// Snapshot for the React shell.
#[tauri::command]
pub fn get_input_bindings() -> InputBindingsDto {
    let runtime = crate::extensions::ensure_loaded();
    let bindings = ensure_loaded(&runtime.result.contributions.command_ids);
    InputBindingsDto {
        bindings: bindings.entries().to_vec(),
    }
}

fn parse_key_binding(key: &str, modifiers: &[String]) -> Result<KeyBinding, String> {
    let modifiers = parse_modifiers(modifiers)?;
    let binding_key = parse_binding_key(key)?;
    Ok(KeyBinding {
        key: binding_key,
        modifiers,
    })
}

fn parse_modifiers(names: &[String]) -> Result<u8, String> {
    let mut modifiers = 0u8;
    for name in names {
        match name.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= MOD_CTRL,
            "alt" => modifiers |= MOD_ALT,
            "shift" => modifiers |= MOD_SHIFT,
            "logo" | "super" | "meta" | "cmd" | "command" => {
                modifiers |= MOD_LOGO
            }
            other => {
                return Err(format!("unknown modifier `{other}`"));
            }
        }
    }
    Ok(modifiers)
}

fn parse_binding_key(key: &str) -> Result<BindingKey, String> {
    let normalized = key.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(String::from("empty key"));
    }

    if let Some(named) = named_key_from_str(&normalized) {
        return Ok(BindingKey::Named(named));
    }

    if normalized.chars().count() == 1 {
        return Ok(BindingKey::Character(normalized));
    }

    Err(format!("unsupported key `{key}`"))
}

fn named_key_from_str(name: &str) -> Option<NamedKey> {
    Some(match name {
        "escape" | "esc" => NamedKey::Escape,
        "enter" | "return" => NamedKey::Enter,
        "tab" => NamedKey::Tab,
        "space" | "spacebar" => NamedKey::Space,
        "backspace" => NamedKey::Backspace,
        "delete" | "del" => NamedKey::Delete,
        "arrowup" | "up" => NamedKey::ArrowUp,
        "arrowdown" | "down" => NamedKey::ArrowDown,
        "arrowleft" | "left" => NamedKey::ArrowLeft,
        "arrowright" | "right" => NamedKey::ArrowRight,
        "home" => NamedKey::Home,
        "end" => NamedKey::End,
        "pageup" | "page_up" => NamedKey::PageUp,
        "pagedown" | "page_down" => NamedKey::PageDown,
        "f1" => NamedKey::F1,
        "f2" => NamedKey::F2,
        "f3" => NamedKey::F3,
        "f4" => NamedKey::F4,
        "f5" => NamedKey::F5,
        "f6" => NamedKey::F6,
        "f7" => NamedKey::F7,
        "f8" => NamedKey::F8,
        "f9" => NamedKey::F9,
        "f10" => NamedKey::F10,
        "f11" => NamedKey::F11,
        "f12" => NamedKey::F12,
        _ => return None,
    })
}

/// Format a JSON key + modifiers as a Tauri/muda accelerator string.
fn format_accelerator(key: &str, modifiers: &[String]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut has_ctrl = false;
    let mut has_alt = false;
    let mut has_shift = false;
    let mut has_logo = false;
    for name in modifiers {
        match name.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => has_ctrl = true,
            "alt" => has_alt = true,
            "shift" => has_shift = true,
            "logo" | "super" | "meta" | "cmd" | "command" => has_logo = true,
            _ => {}
        }
    }
    // JSON `ctrl` → CmdOrCtrl so macOS gets ⌘ and Win/Linux get Ctrl.
    if has_ctrl {
        parts.push("CmdOrCtrl".into());
    } else if has_logo {
        parts.push("Super".into());
    }
    if has_alt {
        parts.push("Alt".into());
    }
    if has_shift {
        parts.push("Shift".into());
    }

    let key_part = accelerator_key_token(key)?;
    parts.push(key_part);
    Some(parts.join("+"))
}

fn accelerator_key_token(key: &str) -> Option<String> {
    let normalized = key.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if let Some(named) = named_key_from_str(&normalized) {
        return Some(named_accelerator_token(named).into());
    }
    if normalized.chars().count() == 1 {
        return Some(normalized.to_ascii_uppercase());
    }
    None
}

fn named_accelerator_token(named: NamedKey) -> &'static str {
    match named {
        NamedKey::Escape => "Escape",
        NamedKey::Enter => "Return",
        NamedKey::Tab => "Tab",
        NamedKey::Space => "Space",
        NamedKey::Backspace => "Backspace",
        NamedKey::Delete => "Delete",
        NamedKey::ArrowUp => "Up",
        NamedKey::ArrowDown => "Down",
        NamedKey::ArrowLeft => "Left",
        NamedKey::ArrowRight => "Right",
        NamedKey::Home => "Home",
        NamedKey::End => "End",
        NamedKey::PageUp => "PageUp",
        NamedKey::PageDown => "PageDown",
        NamedKey::F1 => "F1",
        NamedKey::F2 => "F2",
        NamedKey::F3 => "F3",
        NamedKey::F4 => "F4",
        NamedKey::F5 => "F5",
        NamedKey::F6 => "F6",
        NamedKey::F7 => "F7",
        NamedKey::F8 => "F8",
        NamedKey::F9 => "F9",
        NamedKey::F10 => "F10",
        NamedKey::F11 => "F11",
        NamedKey::F12 => "F12",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> HashSet<String> {
        HashSet::from([
            "document.open-file".to_string(),
            "document.close_active".to_string(),
            "edit.undo".to_string(),
            "edit.redo".to_string(),
            "play.toggle".to_string(),
        ])
    }

    #[test]
    fn parses_default_bindings() {
        let bindings =
            Bindings::from_json_str(DEFAULT_BINDINGS_JSON, &known()).unwrap();
        // Defaults include more commands than `known()`; only known kept.
        assert_eq!(bindings.len(), 5);
        assert_eq!(
            bindings.accelerator_for_command("document.open-file").as_deref(),
            Some("CmdOrCtrl+O")
        );
        assert_eq!(
            bindings.accelerator_for_command("edit.undo").as_deref(),
            Some("CmdOrCtrl+Z")
        );
        assert_eq!(
            bindings.accelerator_for_command("edit.redo").as_deref(),
            Some("CmdOrCtrl+Shift+Z")
        );
        assert_eq!(
            bindings.accelerator_for_command("play.toggle").as_deref(),
            Some("Space")
        );
    }

    #[test]
    fn rejects_unsupported_version() {
        let err = Bindings::from_json_str(
            r#"{"version": 99, "bindings": []}"#,
            &known(),
        )
        .unwrap_err();
        assert!(err.contains("unsupported bindings version"));
    }

    #[test]
    fn skips_unknown_command() {
        let bindings = Bindings::from_json_str(
            r#"{
              "version": 1,
              "bindings": [
                {
                  "command": "future.unknown",
                  "when": { "type": "key", "key": "x", "modifiers": [] }
                },
                {
                  "command": "document.open-file",
                  "when": { "type": "key", "key": "o", "modifiers": ["ctrl"] }
                }
              ]
            }"#,
            &known(),
        )
        .unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings.accelerator_for_command("document.open-file").as_deref(),
            Some("CmdOrCtrl+O")
        );
    }

    #[test]
    fn load_chain_prefers_user_file() {
        let dir = std::env::temp_dir().join(format!(
            "imp-lab-bindings-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let user = dir.join("bindings.json");
        fs::write(
            &user,
            r#"{
              "version": 1,
              "bindings": [
                {
                  "command": "document.close_active",
                  "when": { "type": "key", "key": "q", "modifiers": ["ctrl"] }
                }
              ]
            }"#,
        )
        .unwrap();

        let bindings =
            Bindings::load_chain(Some(&user), DEFAULT_BINDINGS_JSON, &known());
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings
                .accelerator_for_command("document.close_active")
                .as_deref(),
            Some("CmdOrCtrl+Q")
        );
        assert!(bindings
            .accelerator_for_command("document.open-file")
            .is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_chain_falls_back_to_default_when_user_missing() {
        let missing =
            PathBuf::from("/tmp/imp-lab-bindings-does-not-exist.json");
        let bindings =
            Bindings::load_chain(Some(&missing), DEFAULT_BINDINGS_JSON, &known());
        assert_eq!(bindings.len(), 5);
    }

    #[test]
    fn load_chain_empty_when_default_invalid() {
        let bindings = Bindings::load_chain(None, "{not json", &known());
        assert!(bindings.is_empty());
    }

    #[test]
    fn named_key_accelerator() {
        let bindings = Bindings::from_json_str(
            r#"{
              "version": 1,
              "bindings": [
                {
                  "command": "document.open-file",
                  "when": { "type": "key", "key": "escape", "modifiers": [] }
                }
              ]
            }"#,
            &known(),
        )
        .unwrap();
        assert_eq!(
            bindings.accelerator_for_command("document.open-file").as_deref(),
            Some("Escape")
        );
    }
}
