//! Resolve `extensions.json` and load enabled extension cdylibs.

use crate::config::{
    parse_extensions_json, ComponentKind, ExtensionsConfig, ExtensionsError,
};
use crate::host::{
    host_vtable, ExtensionHost, ExtensionRegisterFn, EXTENSION_REGISTER_SYMBOL,
};
use crate::service::{
    invoke_service_action, menu_contributions_from_config, resolve_command_action,
    CommandDispatchError, MenuItemContribution, ServiceRegistry,
};
use libloading::Library;
use serde::Serialize;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;

/// Catalogs collected from enabled `imp-catalog` extensions.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedCatalogs {
    pub node_libraries: Vec<Value>,
    pub imp_files: Vec<Value>,
}

/// Serializable contribution snapshot for the webview (no service vtables).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionContributions {
    /// Enabled command ids.
    pub command_ids: Vec<String>,
    /// Menu items grouped for native chrome / optional client bindings.
    pub menu_items: Vec<MenuItemContributionDto>,
}

/// Serde-friendly menu item for the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuItemContributionDto {
    pub command_id: String,
    pub label: String,
    pub menu: String,
    pub order: i64,
}

impl From<&MenuItemContribution> for MenuItemContributionDto {
    fn from(item: &MenuItemContribution) -> Self {
        Self {
            command_id: item.command_id.clone(),
            label: item.label.clone(),
            menu: item.menu.clone(),
            order: item.order,
        }
    }
}

/// Result of loading optional extensions for a Lab host (webview-safe).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionsLoadResult {
    /// Parsed config when a file was found and accepted; `None` if missing/invalid.
    pub config: Option<ExtensionsConfigDto>,
    /// Absolute pack root (directory containing `extensions.json`) when loaded.
    pub pack_root: Option<String>,
    /// Path that was used (or attempted).
    pub config_path: Option<String>,
    /// Human-readable warnings (missing file, parse errors, load failures).
    pub warnings: Vec<String>,
    /// Catalogs from successful `imp-catalog` loads.
    pub catalogs: LoadedCatalogs,
    /// Command / menu contributions from the config (even when libraries fail).
    pub contributions: ExtensionContributions,
}

/// Full runtime after load: DTO snapshot + retained services + parsed config.
pub struct ExtensionsRuntime {
    /// Webview-safe load snapshot.
    pub result: ExtensionsLoadResult,
    /// Parsed config when valid (needed for command resolve).
    pub config: Option<ExtensionsConfig>,
    /// Registered service vtables keyed by implementation id.
    pub services: Mutex<ServiceRegistry>,
}

impl ExtensionsRuntime {
    /// Empty runtime (no pack).
    pub fn empty() -> Self {
        Self {
            result: ExtensionsLoadResult {
                config: None,
                pack_root: None,
                config_path: None,
                warnings: Vec::new(),
                catalogs: LoadedCatalogs::default(),
                contributions: ExtensionContributions::default(),
            },
            config: None,
            services: Mutex::new(ServiceRegistry::default()),
        }
    }

    /// Run an extension command against a registered service.
    pub fn run_command(
        &self,
        command_id: &str,
        graph_json: Option<&str>,
        neighbor_path: Option<&str>,
    ) -> Result<Option<bool>, CommandDispatchError> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| CommandDispatchError::UnknownCommand(command_id.to_string()))?;
        let resolved = resolve_command_action(config, command_id)?;
        let services = self
            .services
            .lock()
            .map_err(|_| CommandDispatchError::ServiceFailed("service registry poisoned".into()))?;
        let vtable = services
            .get(&resolved.service_implementation_id)
            .copied()
            .ok_or_else(|| {
                CommandDispatchError::ServiceNotRegistered(
                    resolved.service_implementation_id.clone(),
                )
            })?;
        drop(services);
        unsafe { invoke_service_action(&vtable, &resolved.action, graph_json, neighbor_path) }
    }

    /// Playback status for a registered service implementation id (e.g. `playback`).
    pub fn playback_status(
        &self,
        implementation_id: &str,
    ) -> Result<(bool, f64), CommandDispatchError> {
        let services = self
            .services
            .lock()
            .map_err(|_| CommandDispatchError::ServiceFailed("service registry poisoned".into()))?;
        let vtable = services
            .get(implementation_id)
            .copied()
            .ok_or_else(|| {
                CommandDispatchError::ServiceNotRegistered(implementation_id.to_string())
            })?;
        drop(services);
        let playing = unsafe {
            match vtable.is_playing {
                Some(is_playing) => {
                    let code = is_playing(vtable.ctx);
                    if code < 0 {
                        return Err(CommandDispatchError::ServiceFailed(format!(
                            "is_playing failed ({code})"
                        )));
                    }
                    code != 0
                }
                None => {
                    return Err(CommandDispatchError::UnsupportedAction(
                        "is_playing".into(),
                    ))
                }
            }
        };
        let position = unsafe {
            crate::service::service_position_secs(&vtable).unwrap_or(0.0)
        };
        Ok((playing, position))
    }

    /// Deliver a service-defined JSON event to a registered service implementation id.
    pub fn send_event(
        &self,
        implementation_id: &str,
        json: &str,
    ) -> Result<(), CommandDispatchError> {
        let services = self
            .services
            .lock()
            .map_err(|_| CommandDispatchError::ServiceFailed("service registry poisoned".into()))?;
        let vtable = services
            .get(implementation_id)
            .copied()
            .ok_or_else(|| {
                CommandDispatchError::ServiceNotRegistered(implementation_id.to_string())
            })?;
        drop(services);
        unsafe { crate::service::send_service_event(&vtable, json) }
    }

    /// Send a service-defined JSON request and return the reply.
    pub fn request(
        &self,
        implementation_id: &str,
        json: &str,
    ) -> Result<String, CommandDispatchError> {
        let services = self
            .services
            .lock()
            .map_err(|_| CommandDispatchError::ServiceFailed("service registry poisoned".into()))?;
        let vtable = services
            .get(implementation_id)
            .copied()
            .ok_or_else(|| {
                CommandDispatchError::ServiceNotRegistered(implementation_id.to_string())
            })?;
        drop(services);
        unsafe { crate::service::send_service_request(&vtable, json) }
    }
}

/// Serde-friendly config mirror for the frontend (keeps camelCase).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionsConfigDto {
    pub version: u32,
    pub extensions: Vec<ExtensionConfigDto>,
    pub components: Vec<ComponentConfigDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionConfigDto {
    pub id: String,
    pub enabled: bool,
    pub library_path: Option<String>,
    pub ui_library_path: Option<String>,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentConfigDto {
    pub id: String,
    pub extension_id: String,
    pub kind: String,
    pub implementation_id: String,
    pub enabled: bool,
    pub label: Option<String>,
    pub roles: Vec<String>,
    pub params: Value,
}

/// Errors that abort a single library load (surfaced as warnings on the aggregate result).
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to load dynamic library {path}: {source}")]
    Library {
        path: String,
        source: libloading::Error,
    },
    #[error("missing symbol imp_lab_extension_register in {path}: {source}")]
    Symbol {
        path: String,
        source: libloading::Error,
    },
    #[error("extension register failed with code {code} ({path})")]
    Register { path: String, code: i32 },
}

/// Resolve the extensions config path.
///
/// Order: `IMP_LAB_EXTENSIONS` → `{exe_dir}/extensions.json` → `{cwd}/extensions.json`.
pub fn resolve_extensions_config_path() -> Option<PathBuf> {
    if let Ok(path) = env::var("IMP_LAB_EXTENSIONS") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("extensions.json");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Ok(cwd) = env::current_dir() {
        let candidate = cwd.join("extensions.json");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Load optional extensions from the default search path.
pub fn load_extensions_default() -> ExtensionsRuntime {
    match resolve_extensions_config_path() {
        Some(path) => load_extensions_from_path(&path),
        None => ExtensionsRuntime::empty(),
    }
}

/// Load extensions from an explicit `extensions.json` path.
pub fn load_extensions_from_path(config_path: &Path) -> ExtensionsRuntime {
    let mut warnings = Vec::new();
    let config_path_str = config_path.display().to_string();
    let text = match fs::read_to_string(config_path) {
        Ok(t) => t,
        Err(err) => {
            warnings.push(format!("failed to read {config_path_str}: {err}"));
            let mut runtime = ExtensionsRuntime::empty();
            runtime.result.pack_root = config_path.parent().map(|p| p.display().to_string());
            runtime.result.config_path = Some(config_path_str);
            runtime.result.warnings = warnings;
            return runtime;
        }
    };

    let config = match parse_extensions_json(&text) {
        Ok(c) => c,
        Err(err) => {
            warnings.push(format!("extensions config error ({config_path_str}): {err}"));
            let mut runtime = ExtensionsRuntime::empty();
            runtime.result.pack_root = config_path.parent().map(|p| p.display().to_string());
            runtime.result.config_path = Some(config_path_str);
            runtime.result.warnings = warnings;
            return runtime;
        }
    };

    let pack_root = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut catalogs = LoadedCatalogs::default();
    let mut services = ServiceRegistry::default();
    // Keep libraries alive for the process (extensions may hold TLS / statics).
    let mut _libs: Vec<Library> = Vec::new();

    for ext in config.extensions.iter().filter(|e| e.enabled) {
        let Some(rel) = ext.library_path.as_deref() else {
            continue;
        };
        let lib_path = resolve_library_path(&pack_root, rel);
        match load_one_library(&lib_path, &pack_root) {
            Ok((lib, host)) => {
                if extension_has_enabled_imp_catalog(&config, &ext.id) {
                    catalogs.node_libraries.extend(host.node_libraries);
                    catalogs.imp_files.extend(host.imp_files);
                }
                // Retain services from every successful library load.
                for (id, vtable) in host.services.into_map() {
                    services.insert(id, vtable);
                }
                _libs.push(lib);
            }
            Err(err) => warnings.push(err.to_string()),
        }
    }

    // Leak libraries for process lifetime so extension code stays mapped.
    std::mem::forget(_libs);

    let contributions = contributions_from_config(&config);

    ExtensionsRuntime {
        result: ExtensionsLoadResult {
            config: Some(config_to_dto(&config)),
            pack_root: Some(pack_root.display().to_string()),
            config_path: Some(config_path_str),
            warnings,
            catalogs,
            contributions,
        },
        config: Some(config),
        services: Mutex::new(services),
    }
}

fn contributions_from_config(config: &ExtensionsConfig) -> ExtensionContributions {
    let menu_items: Vec<MenuItemContributionDto> = menu_contributions_from_config(config)
        .iter()
        .map(MenuItemContributionDto::from)
        .collect();
    let mut command_ids: Vec<String> = config
        .components
        .iter()
        .filter(|c| c.enabled && matches!(c.kind, ComponentKind::Command))
        .map(|c| c.id.clone())
        .collect();
    command_ids.sort();
    command_ids.dedup();
    ExtensionContributions {
        command_ids,
        menu_items,
    }
}

fn extension_has_enabled_imp_catalog(config: &ExtensionsConfig, extension_id: &str) -> bool {
    config.components.iter().any(|c| {
        c.enabled
            && c.extension_id == extension_id
            && matches!(c.kind, ComponentKind::ImpCatalog)
    })
}

/// Resolve a pack-relative library path, trying platform-typical cdylib names when needed.
fn resolve_library_path(pack_root: &Path, relative: &str) -> PathBuf {
    let direct = pack_root.join(relative);
    if direct.is_file() {
        return direct;
    }
    let path = Path::new(relative);
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(relative);
    let stem = stem.strip_prefix("lib").unwrap_or(stem);
    let candidates = [
        format!("lib{stem}.so"),
        format!("{stem}.dll"),
        format!("lib{stem}.dylib"),
        format!("{stem}.so"),
    ];
    for name in candidates {
        let candidate = pack_root.join(parent).join(&name);
        if candidate.is_file() {
            return candidate;
        }
    }
    direct
}

fn load_one_library(
    lib_path: &Path,
    pack_root: &Path,
) -> Result<(Library, ExtensionHost), LoadError> {
    let path_str = lib_path.display().to_string();
    let library = unsafe { Library::new(lib_path) }.map_err(|source| LoadError::Library {
        path: path_str.clone(),
        source,
    })?;

    let register: libloading::Symbol<ExtensionRegisterFn> = unsafe {
        library
            .get(EXTENSION_REGISTER_SYMBOL)
            .map_err(|source| LoadError::Symbol {
                path: path_str.clone(),
                source,
            })?
    };

    let mut host = ExtensionHost::new(pack_root);
    let vtable = host_vtable(&mut host);
    let code = unsafe { register(&vtable) };
    if code != 0 {
        return Err(LoadError::Register {
            path: path_str,
            code,
        });
    }
    Ok((library, host))
}

fn config_to_dto(config: &ExtensionsConfig) -> ExtensionsConfigDto {
    ExtensionsConfigDto {
        version: config.version,
        extensions: config
            .extensions
            .iter()
            .map(|e| ExtensionConfigDto {
                id: e.id.clone(),
                enabled: e.enabled,
                library_path: e.library_path.clone(),
                ui_library_path: e.ui_library_path.clone(),
                params: Value::Object(e.params.clone().into_iter().collect()),
            })
            .collect(),
        components: config
            .components
            .iter()
            .map(|c| ComponentConfigDto {
                id: c.id.clone(),
                extension_id: c.extension_id.clone(),
                kind: c.kind.as_str().into(),
                implementation_id: c.implementation_id.clone(),
                enabled: c.enabled,
                label: c.label.clone(),
                roles: c.roles.clone(),
                params: Value::Object(c.params.clone().into_iter().collect()),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn missing_config_is_empty_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("extensions.json");
        let runtime = load_extensions_from_path(&path);
        assert!(runtime.result.config.is_none());
        assert!(!runtime.result.warnings.is_empty());
        assert!(runtime.result.catalogs.node_libraries.is_empty());
    }

    #[test]
    fn invalid_config_warns_without_catalogs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("extensions.json");
        let mut f = fs::File::create(&path).unwrap();
        write!(f, r#"{{"version":99,"extensions":[],"components":[]}}"#).unwrap();
        let runtime = load_extensions_from_path(&path);
        assert!(runtime.result.config.is_none());
        assert!(runtime
            .result
            .warnings
            .iter()
            .any(|w| w.contains("version")));
    }

    #[test]
    fn valid_config_without_library_loads_dto_and_contributions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("extensions.json");
        fs::write(
            &path,
            r#"{
              "version": 1,
              "extensions": [{"id":"x","enabled":true,"uiLibraryPath":"x/ui.js"}],
              "components": [
                {
                  "id":"play.start",
                  "extensionId":"x",
                  "kind":"command",
                  "implementationId":"play.start",
                  "enabled":true,
                  "params":{"serviceId":"svc","action":"start"}
                },
                {
                  "id":"menu.play.start",
                  "extensionId":"x",
                  "kind":"menu-item",
                  "implementationId":"play.start",
                  "label":"Start",
                  "enabled":true,
                  "params":{"menu":"Play","order":1}
                }
              ]
            }"#,
        )
        .unwrap();
        let runtime = load_extensions_from_path(&path);
        assert!(runtime.result.warnings.is_empty());
        let config = runtime.result.config.expect("config");
        assert_eq!(
            config.extensions[0].ui_library_path.as_deref(),
            Some("x/ui.js")
        );
        assert_eq!(
            runtime.result.contributions.command_ids,
            vec!["play.start".to_string()]
        );
        assert_eq!(runtime.result.contributions.menu_items.len(), 1);
        assert_eq!(
            runtime.result.contributions.menu_items[0].menu,
            "Play"
        );
    }
}

// Silence unused import warning for ExtensionsError in non-test — used by parse re-exports.
#[allow(dead_code)]
fn _use_extensions_error(_: ExtensionsError) {}
