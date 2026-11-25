//! `extensions.json` schema (Tome-shaped).

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

/// Supported `extensions.json` schema version.
pub const SUPPORTED_VERSION: u32 = 1;

/// Top-level extensions configuration.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ExtensionsConfig {
    /// Schema version.
    pub version: u32,
    /// Linked extension packages.
    #[serde(default)]
    pub extensions: Vec<ExtensionConfig>,
    /// Component instances wired into the host.
    #[serde(default)]
    pub components: Vec<ComponentConfig>,
}

/// One extension package row.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionConfig {
    /// Stable extension id (e.g. `aura-imp-catalogs`).
    pub id: String,
    /// Whether the extension is active.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Pack-relative path to the host Rust cdylib (`.dll` / `.so` / `.dylib`).
    #[serde(default)]
    pub library_path: Option<String>,
    /// Pack-relative path to the extension's webview UI library (one ES module exporting
    /// `implementations`), for UI component kinds such as `node-type-editor`.
    #[serde(default)]
    pub ui_library_path: Option<String>,
    /// Extension-level params merged under component params.
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

/// One component instance row.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentConfig {
    /// Stable component id (often equal to `implementation_id`).
    pub id: String,
    /// Owning extension id.
    pub extension_id: String,
    /// Component kind.
    pub kind: ComponentKind,
    /// Implementation id registered by the extension package (unused by settings kinds).
    #[serde(default)]
    pub implementation_id: String,
    /// Whether this component is active.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional human label.
    #[serde(default)]
    pub label: Option<String>,
    /// Roles this component fills (views; optional for catalogs).
    #[serde(default)]
    pub roles: Vec<String>,
    /// Component-level params (win over extension params on key conflict).
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

/// Known component kinds for Lab v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentKind {
    /// Imp node / constraint / graph library catalogs.
    ImpCatalog,
    /// Host command id (`implementationId`, e.g. `play.start`) dispatched to a service.
    Command,
    /// Native menu entry that emits a command id (`implementationId`).
    MenuItem,
    /// Long-lived host backend registered by the extension cdylib.
    Service,
    /// One setting (id = component id); shown on the pages of the categories it is
    /// associated with, or on the fallback page.
    Setting,
    /// A settings category, shown as one settings page.
    SettingsCategory,
    /// Associates a setting with a category (`params.settingId`, `params.categoryId`).
    SettingsCategoryItem,
    /// A toolbar / status-bar entry displaying a topic (`params.topic`).
    StatusItem,
    /// Dropdown options for one literal input of a node type, from a topic
    /// (`params.nodeType`, `params.port`, `params.optionsTopic`).
    NodeLiteralOptions,
    /// A button in Properties for nodes of a type that sends a service request
    /// (`params.nodeType`, `params.serviceId`, `params.action`, optional `params.editsTopic`).
    NodeAction,
    /// An editor for records of one Tome node type, implemented in the extension's UI library
    /// (`params.nodeTypeId`, `params.column`; see [`crate::editors`]).
    NodeTypeEditor,
    /// A default key binding (`params.command`, `params.key`, optional `params.modifiers`).
    Binding,
}

impl ComponentKind {
    /// Kebab-case kind string for DTOs / docs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImpCatalog => "imp-catalog",
            Self::Command => "command",
            Self::MenuItem => "menu-item",
            Self::Service => "service",
            Self::Setting => "setting",
            Self::SettingsCategory => "settings-category",
            Self::SettingsCategoryItem => "settings-category-item",
            Self::StatusItem => "status-item",
            Self::NodeLiteralOptions => "node-literal-options",
            Self::NodeAction => "node-action",
            Self::NodeTypeEditor => "node-type-editor",
            Self::Binding => "binding",
        }
    }
}

fn default_true() -> bool {
    true
}

/// Failure while parsing or validating extensions config.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExtensionsError {
    /// JSON did not parse.
    #[error("invalid extensions JSON: {0}")]
    Json(String),
    /// Unsupported schema version.
    #[error("unsupported extensions version {found} (expected {SUPPORTED_VERSION})")]
    UnsupportedVersion {
        /// Version found in the file.
        found: u32,
    },
}

/// Parse and validate an `extensions.json` document.
pub fn parse_extensions_json(json: &str) -> Result<ExtensionsConfig, ExtensionsError> {
    let config: ExtensionsConfig =
        serde_json::from_str(json).map_err(|e| ExtensionsError::Json(e.to_string()))?;
    if config.version != SUPPORTED_VERSION {
        return Err(ExtensionsError::UnsupportedVersion {
            found: config.version,
        });
    }
    Ok(config)
}

impl ComponentConfig {
    /// Merged params: extension params first, then component params.
    pub fn merged_params(
        &self,
        extension: Option<&ExtensionConfig>,
    ) -> BTreeMap<String, Value> {
        let mut merged = BTreeMap::new();
        if let Some(ext) = extension {
            merged.extend(ext.params.clone());
        }
        merged.extend(self.params.clone());
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tome_shaped_catalog_config() {
        let json = r#"{
          "version": 1,
          "extensions": [
            {
              "id": "aura-imp-catalogs",
              "enabled": true,
              "libraryPath": "extensions/aura_imp_catalogs.so",
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
        }"#;
        let config = parse_extensions_json(json).expect("parse");
        assert_eq!(config.extensions.len(), 1);
        assert_eq!(
            config.extensions[0].library_path.as_deref(),
            Some("extensions/aura_imp_catalogs.so")
        );
        assert_eq!(config.components[0].kind, ComponentKind::ImpCatalog);
    }

    #[test]
    fn rejects_unsupported_version() {
        let err = parse_extensions_json(r#"{"version":2,"extensions":[],"components":[]}"#)
            .unwrap_err();
        assert!(matches!(
            err,
            ExtensionsError::UnsupportedVersion { found: 2 }
        ));
    }

    #[test]
    fn parses_command_menu_and_service_kinds() {
        let json = r#"{
          "version": 1,
          "extensions": [{"id":"aura-playback","enabled":true,"libraryPath":"extensions/x"}],
          "components": [
            {
              "id":"aura-playback",
              "extensionId":"aura-playback",
              "kind":"service",
              "implementationId":"playback",
              "enabled":true
            },
            {
              "id":"play.start",
              "extensionId":"aura-playback",
              "kind":"command",
              "implementationId":"play.start",
              "enabled":true,
              "params":{"serviceId":"aura-playback","action":"start"}
            },
            {
              "id":"menu.play.start",
              "extensionId":"aura-playback",
              "kind":"menu-item",
              "implementationId":"play.start",
              "label":"Start",
              "enabled":true,
              "params":{"menu":"Play","order":10}
            }
          ]
        }"#;
        let config = parse_extensions_json(json).expect("parse");
        assert_eq!(config.components[0].kind, ComponentKind::Service);
        assert_eq!(config.components[1].kind, ComponentKind::Command);
        assert_eq!(config.components[2].kind, ComponentKind::MenuItem);
    }
}
