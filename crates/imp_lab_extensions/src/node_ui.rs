//! Node UI an extension declares for node types it knows: dropdown options for a literal input,
//! and action buttons in Properties.
//!
//! Lab draws these without knowing what the node types mean. An action sends the owning
//! service an opaque request; a service may later edit the node's literals through the action's
//! edits topic (see [`NodeEdit`]).

use crate::config::{ComponentKind, ExtensionsConfig};
use crate::settings::{enabled_components, param_str, service_implementation};
use serde::Serialize;
use serde_json::Value;

/// A `node-literal-options` component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeLiteralOptionsDecl {
    pub id: String,
    /// Node type id the options apply to.
    pub node_type: String,
    /// Literal input port.
    pub port: String,
    /// Topic whose value is `[{value, label}]`.
    pub options_topic: String,
}

/// A `node-action` component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeActionDecl {
    pub id: String,
    pub label: String,
    pub node_type: String,
    /// Implementation id of the service that receives the request.
    pub service_id: String,
    /// Action name, passed through in the request.
    pub action: String,
    /// Topic on which the service publishes [`NodeEdit`]s for nodes it acted on.
    pub edits_topic: Option<String>,
    pub order: i64,
}

/// What an edits topic carries: replace literals of node `node` in whichever open graph holds
/// it. A `null` input removes that literal. `seq` increases with every edit; edits that share a
/// `session` are one undo step.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct NodeEdit {
    pub seq: u64,
    pub session: String,
    pub node: String,
    pub inputs: serde_json::Map<String, Value>,
}

/// Node UI declarations, plus problems found while reading them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodeUi {
    pub literal_options: Vec<NodeLiteralOptionsDecl>,
    pub actions: Vec<NodeActionDecl>,
    pub warnings: Vec<String>,
}

/// The enabled node UI declarations. Actions sort by `order`, then id.
pub fn node_ui(config: &ExtensionsConfig) -> NodeUi {
    let mut ui = NodeUi::default();
    for (component, params) in enabled_components(config, ComponentKind::NodeLiteralOptions) {
        match (
            param_str(&params, "nodeType"),
            param_str(&params, "port"),
            param_str(&params, "optionsTopic"),
        ) {
            (Some(node_type), Some(port), Some(options_topic)) => {
                ui.literal_options.push(NodeLiteralOptionsDecl {
                    id: component.id.clone(),
                    node_type,
                    port,
                    options_topic,
                })
            }
            _ => ui.warnings.push(format!(
                "node-literal-options `{}` needs nodeType, port and optionsTopic",
                component.id
            )),
        }
    }
    for (component, params) in enabled_components(config, ComponentKind::NodeAction) {
        let (Some(node_type), Some(service), Some(action)) = (
            param_str(&params, "nodeType"),
            param_str(&params, "serviceId"),
            param_str(&params, "action"),
        ) else {
            ui.warnings.push(format!(
                "node-action `{}` needs nodeType, serviceId and action",
                component.id
            ));
            continue;
        };
        let Some(service_id) = service_implementation(config, &service) else {
            ui.warnings.push(format!(
                "node-action `{}` names service `{service}`, which is not an enabled service",
                component.id
            ));
            continue;
        };
        ui.actions.push(NodeActionDecl {
            id: component.id.clone(),
            label: component.label.clone().unwrap_or_else(|| action.clone()),
            node_type,
            service_id,
            action,
            edits_topic: param_str(&params, "editsTopic"),
            order: params.get("order").and_then(Value::as_i64).unwrap_or(0),
        });
    }
    ui.actions
        .sort_by(|a, b| a.order.cmp(&b.order).then(a.id.cmp(&b.id)));
    ui
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_extensions_json;

    fn config(components: &str) -> ExtensionsConfig {
        parse_extensions_json(&format!(
            r#"{{
              "version": 1,
              "extensions": [
                {{"id": "a", "enabled": true}},
                {{"id": "off", "enabled": false}}
              ],
              "components": [
                {{"id": "svc", "extensionId": "a", "kind": "service", "implementationId": "playback"}},
                {components}
              ]
            }}"#
        ))
        .expect("config")
    }

    #[test]
    fn reads_literal_options_and_actions() {
        let ui = node_ui(&config(
            r#"
            {"id": "opts", "extensionId": "a", "kind": "node-literal-options",
             "params": {"nodeType": "synth", "port": "plugin", "optionsTopic": "a.plugins"}},
            {"id": "gui-2", "extensionId": "a", "kind": "node-action", "label": "Second",
             "params": {"nodeType": "synth", "serviceId": "svc", "action": "two", "order": 20}},
            {"id": "gui-1", "extensionId": "a", "kind": "node-action", "label": "Open GUI",
             "params": {"nodeType": "synth", "serviceId": "svc", "action": "open",
                        "editsTopic": "a.edits", "order": 10}}
            "#,
        ));
        assert!(ui.warnings.is_empty(), "{:?}", ui.warnings);
        assert_eq!(
            ui.literal_options,
            vec![NodeLiteralOptionsDecl {
                id: "opts".into(),
                node_type: "synth".into(),
                port: "plugin".into(),
                options_topic: "a.plugins".into(),
            }]
        );
        let ids: Vec<&str> = ui.actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["gui-1", "gui-2"]);
        assert_eq!(ui.actions[0].service_id, "playback", "resolved to the implementation id");
        assert_eq!(ui.actions[0].edits_topic.as_deref(), Some("a.edits"));
        assert_eq!(ui.actions[1].label, "Second");
    }

    #[test]
    fn incomplete_or_disabled_declarations_are_skipped() {
        let ui = node_ui(&config(
            r#"
            {"id": "no-topic", "extensionId": "a", "kind": "node-literal-options",
             "params": {"nodeType": "synth", "port": "plugin"}},
            {"id": "no-service", "extensionId": "a", "kind": "node-action",
             "params": {"nodeType": "synth", "serviceId": "missing", "action": "open"}},
            {"id": "off-ext", "extensionId": "off", "kind": "node-action",
             "params": {"nodeType": "synth", "serviceId": "svc", "action": "open"}}
            "#,
        ));
        assert!(ui.literal_options.is_empty());
        assert!(ui.actions.is_empty());
        assert_eq!(ui.warnings.len(), 2, "{:?}", ui.warnings);
    }

    #[test]
    fn node_edits_round_trip() {
        let edit: NodeEdit = serde_json::from_str(
            r#"{"seq": 3, "session": "s1", "node": "n1", "inputs": {"state": "AAA=", "params": null}}"#,
        )
        .expect("edit");
        assert_eq!(edit.seq, 3);
        assert_eq!(edit.inputs["params"], Value::Null);
    }
}
