//! UI components implemented in an extension's webview UI library, and default key bindings.
//!
//! A `node-type-editor` component wires one implementation from its extension's UI library
//! (`uiLibraryPath`) to records of one Tome node type. Which node type it edits, the record
//! column it reads and writes, and the services it talks to are configuration; the library only
//! exports implementations by id. A `binding` component adds a default key binding.

use crate::config::{ComponentKind, ExtensionsConfig};
use crate::settings::{enabled_components, param_str, service_implementation};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Column type used when a `node-type-editor` doesn't name one.
pub const DEFAULT_COLUMN_TYPE: &str = "json";

/// An enabled `node-type-editor` component.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeTypeEditorDecl {
    /// Component id.
    pub id: String,
    /// Owning extension; its UI library holds the implementation.
    pub extension_id: String,
    /// Implementation id exported by the UI library.
    pub implementation_id: String,
    /// Singular label for one record ("Sketch"): New menu items, explorer badges.
    pub label: String,
    /// Tome type-table id; a record of this node type is a member of that table.
    pub node_type_id: String,
    /// Title of the type-table hub node ("Sketches") when Lab adds the type to a store.
    pub type_title: String,
    /// Node-stored column holding the record's value.
    pub column: String,
    /// Tome column type of `column`.
    pub column_type: String,
    /// `type` tag of standalone JSON documents of this node type (seeding sample projects).
    pub document_type: Option<String>,
    /// Merged params passed to the implementation. A `serviceId` naming a service component is
    /// replaced by that service's implementation id.
    pub params: BTreeMap<String, Value>,
}

/// An enabled `binding` component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingDecl {
    pub id: String,
    pub command: String,
    pub key: String,
    pub modifiers: Vec<String>,
}

/// The enabled `node-type-editor` components, plus problems found while reading them.
pub fn node_type_editors(config: &ExtensionsConfig) -> (Vec<NodeTypeEditorDecl>, Vec<String>) {
    let mut editors: Vec<NodeTypeEditorDecl> = Vec::new();
    let mut warnings = Vec::new();
    for (component, mut params) in enabled_components(config, ComponentKind::NodeTypeEditor) {
        let has_ui_library = config
            .extensions
            .iter()
            .any(|e| e.id == component.extension_id && e.ui_library_path.is_some());
        if !has_ui_library {
            warnings.push(format!(
                "node-type-editor `{}`: extension `{}` has no uiLibraryPath",
                component.id, component.extension_id
            ));
            continue;
        }
        if component.implementation_id.is_empty() {
            warnings.push(format!(
                "node-type-editor `{}` needs an implementationId",
                component.id
            ));
            continue;
        }
        let (Some(node_type_id), Some(column)) =
            (param_str(&params, "nodeTypeId"), param_str(&params, "column"))
        else {
            warnings.push(format!(
                "node-type-editor `{}` needs nodeTypeId and column",
                component.id
            ));
            continue;
        };
        if editors.iter().any(|e| e.node_type_id == node_type_id) {
            warnings.push(format!(
                "node-type-editor `{}`: node type `{node_type_id}` already has an editor",
                component.id
            ));
            continue;
        }
        if let Some(service) = param_str(&params, "serviceId") {
            match service_implementation(config, &service) {
                Some(implementation) => {
                    params.insert("serviceId".into(), Value::String(implementation));
                }
                None => {
                    warnings.push(format!(
                        "node-type-editor `{}` names service `{service}`, which is not an enabled service",
                        component.id
                    ));
                    params.remove("serviceId");
                }
            }
        }
        let label = component
            .label
            .clone()
            .unwrap_or_else(|| component.id.clone());
        editors.push(NodeTypeEditorDecl {
            id: component.id.clone(),
            extension_id: component.extension_id.clone(),
            implementation_id: component.implementation_id.clone(),
            type_title: param_str(&params, "typeTitle").unwrap_or_else(|| label.clone()),
            label,
            node_type_id,
            column,
            column_type: param_str(&params, "columnType")
                .unwrap_or_else(|| DEFAULT_COLUMN_TYPE.into()),
            document_type: param_str(&params, "documentType"),
            params,
        });
    }
    (editors, warnings)
}

/// The enabled `binding` components, plus problems found while reading them.
pub fn binding_decls(config: &ExtensionsConfig) -> (Vec<BindingDecl>, Vec<String>) {
    let mut bindings = Vec::new();
    let mut warnings = Vec::new();
    for (component, params) in enabled_components(config, ComponentKind::Binding) {
        let (Some(command), Some(key)) = (param_str(&params, "command"), param_str(&params, "key"))
        else {
            warnings.push(format!("binding `{}` needs command and key", component.id));
            continue;
        };
        let modifiers = params
            .get("modifiers")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        bindings.push(BindingDecl {
            id: component.id.clone(),
            command,
            key,
            modifiers,
        });
    }
    (bindings, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_extensions_json;

    fn config() -> ExtensionsConfig {
        parse_extensions_json(
            r#"{
              "version": 1,
              "extensions": [
                {"id":"svc","libraryPath":"extensions/svc"},
                {"id":"ui","uiLibraryPath":"extensions/ui/index.js"},
                {"id":"no-ui"}
              ],
              "components": [
                {"id":"playback","extensionId":"svc","kind":"service","implementationId":"play"},
                {"id":"sketch-editor","extensionId":"ui","kind":"node-type-editor",
                 "implementationId":"sketch","label":"Sketch",
                 "params":{"nodeTypeId":"01SKETCH","column":"sketch","typeTitle":"Sketches",
                           "documentType":"sketch","serviceId":"playback"}},
                {"id":"twice","extensionId":"ui","kind":"node-type-editor","implementationId":"sketch",
                 "params":{"nodeTypeId":"01SKETCH","column":"sketch"}},
                {"id":"headless","extensionId":"no-ui","kind":"node-type-editor","implementationId":"x",
                 "params":{"nodeTypeId":"01OTHER","column":"x"}},
                {"id":"missing","extensionId":"ui","kind":"node-type-editor","implementationId":"x"},
                {"id":"off","extensionId":"ui","kind":"node-type-editor","implementationId":"x",
                 "enabled":false,"params":{"nodeTypeId":"01OFF","column":"x"}},
                {"id":"draw","extensionId":"ui","kind":"binding",
                 "params":{"command":"sketch.draw","key":"d"}},
                {"id":"new","extensionId":"ui","kind":"binding",
                 "params":{"command":"document.new:sketch-editor","key":"n","modifiers":["ctrl"]}},
                {"id":"bad","extensionId":"ui","kind":"binding","params":{"key":"x"}}
              ]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn reads_node_type_editors_resolving_services() {
        let (editors, warnings) = node_type_editors(&config());
        assert_eq!(editors.len(), 1, "{warnings:?}");
        let editor = &editors[0];
        assert_eq!(editor.implementation_id, "sketch");
        assert_eq!(editor.node_type_id, "01SKETCH");
        assert_eq!(editor.type_title, "Sketches");
        assert_eq!(editor.column_type, DEFAULT_COLUMN_TYPE);
        assert_eq!(editor.document_type.as_deref(), Some("sketch"));
        assert_eq!(editor.params["serviceId"], Value::String("play".into()));
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("already has an editor")));
        assert!(warnings.iter().any(|w| w.contains("no uiLibraryPath")));
        assert!(warnings.iter().any(|w| w.contains("needs nodeTypeId")));
    }

    #[test]
    fn reads_bindings() {
        let (bindings, warnings) = binding_decls(&config());
        assert_eq!(
            bindings.iter().map(|b| b.command.as_str()).collect::<Vec<_>>(),
            vec!["sketch.draw", "document.new:sketch-editor"]
        );
        assert_eq!(bindings[1].modifiers, vec!["ctrl".to_string()]);
        assert_eq!(warnings.len(), 1);
    }
}
