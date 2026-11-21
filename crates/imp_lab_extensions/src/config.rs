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
    /// Optional TypeScript client module id/path (reserved for UI kinds).
    #[serde(default)]
    pub client_module: Option<String>,
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
    /// Implementation id registered by the extension package.
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
}
