//! Lab model: Tome node types Lab can open, and the well-known ids of a Lab project.
//!
//! A record is one Tome node: its title, plus a node-stored column holding its value, plus set
//! membership in its node type's type table. Imp graphs (`graph: imp_graph`) are built in;
//! extensions add node types through `node-type-editor` components. The Imp graph ids are fixed
//! so every project (and TS Tome) recognises graph records the same way.

use serde::Serialize;
use serde_json::{json, Map, Value};
use tome_db::{DbError, TomeSession};
use tome_flatfile::{
    set_trait_relationship_type_ids, ASSOCIATIONS_FILENAME, CORPUS_FILENAME, TABLE_SCHEMAS_FILENAME,
};
use tome_graph_interfaces::{js_json_file, Node, Properties};

/// Type table whose members are Imp graphs (column `graph`, `imp_graph`).
pub const IMP_GRAPH_TABLE_ID: &str = "01M80M9GRAPH00000000000001";
/// Set-trait relationship type written into new projects.
pub const SET_TYPE_ID: &str = "01M80M9SET0000000000000001";
/// Home and archive hubs of a new project's primary store.
pub const HOME_NODE_ID: &str = "01M80M9H0ME000000000000001";
pub const ARCHIVE_NODE_ID: &str = "01M80M9ARCH000000000000001";

/// A Tome node type Lab can open: its type table, and the node-stored column with the value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeType {
    /// Type-table id (also the hub node's id).
    pub id: String,
    /// Singular label for one record ("Imp graph").
    pub label: String,
    /// Hub node title ("Imp Graphs").
    pub title: String,
    /// Node-stored column holding the record's value.
    pub column: String,
    /// Tome column type of `column`.
    pub column_type: String,
    /// `type` tag of standalone JSON documents of this node type (seeding). Imp graphs have none:
    /// Imp files are recognized by `format: "imp"`.
    pub document_type: Option<String>,
}

impl NodeType {
    /// The built-in Imp graph node type.
    pub fn imp_graph() -> Self {
        Self {
            id: IMP_GRAPH_TABLE_ID.into(),
            label: "Imp graph".into(),
            title: "Imp Graphs".into(),
            column: "graph".into(),
            column_type: "imp_graph".into(),
            document_type: None,
        }
    }

    pub fn is_imp_graph(&self) -> bool {
        self.id == IMP_GRAPH_TABLE_ID
    }

    /// The node type an extension's `node-type-editor` opens.
    pub fn from_editor(editor: &imp_lab_extensions::NodeTypeEditorDecl) -> Self {
        Self {
            id: editor.node_type_id.clone(),
            label: editor.label.clone(),
            title: editor.type_title.clone(),
            column: editor.column.clone(),
            column_type: editor.column_type.clone(),
            document_type: editor.document_type.clone(),
        }
    }

    fn table_schema(&self) -> Value {
        json!({ "columns": [
            { "key": self.column, "name": self.label, "type": self.column_type, "storage": "node" }
        ] })
    }
}

/// Imp graphs, then the node types of `editors` (extension `node-type-editor` components).
pub fn node_types_with(editors: &[imp_lab_extensions::NodeTypeEditorDecl]) -> Vec<NodeType> {
    std::iter::once(NodeType::imp_graph())
        .chain(editors.iter().map(NodeType::from_editor))
        .collect()
}

/// The first of `type_table_ids` that is one of `types`.
pub fn node_type_of<'a>(types: &'a [NodeType], type_table_ids: &[String]) -> Option<&'a NodeType> {
    types.iter().find(|t| type_table_ids.contains(&t.id))
}

fn titled_node(id: &str, title: &str) -> Node {
    let mut properties = Properties::new();
    properties.insert("title".into(), Value::String(title.into()));
    let now = tome_db::now_iso();
    properties.insert("created_at".into(), Value::String(now.clone()));
    properties.insert("modified_at".into(), Value::String(now));
    Node {
        id: id.into(),
        properties,
    }
}

/// Model files for a brand-new project's primary store.
pub fn new_project_model_files(name: &str) -> Vec<(&'static str, String)> {
    let associations = json!({
        "version": 1,
        "associations": {
            SET_TYPE_ID: { "perspectives": ["Members", "Member of"], "traits": ["Set"] }
        }
    });
    let corpus = json!({
        "version": 1,
        "homeNodeId": HOME_NODE_ID,
        "archiveNodeId": ARCHIVE_NODE_ID,
        "protectedNodeIds": [HOME_NODE_ID, ARCHIVE_NODE_ID, IMP_GRAPH_TABLE_ID],
        "graphExplorer": { "defaultAnchorNodeId": HOME_NODE_ID },
        "staticSite": { "homeNodeId": HOME_NODE_ID },
        "quickLinks": [],
        "branding": { "appTitle": name }
    });
    let graph = NodeType::imp_graph();
    let mut tables = Map::new();
    tables.insert(graph.id.clone(), graph.table_schema());
    let table_schemas = json!({ "version": 1, "tables": tables });
    vec![
        (ASSOCIATIONS_FILENAME, js_json_file(&associations)),
        (CORPUS_FILENAME, js_json_file(&corpus)),
        (TABLE_SCHEMAS_FILENAME, js_json_file(&table_schemas)),
    ]
}

/// Hub nodes for a brand-new project.
pub fn new_project_nodes() -> Vec<Node> {
    let graph = NodeType::imp_graph();
    vec![
        titled_node(HOME_NODE_ID, "Home"),
        titled_node(ARCHIVE_NODE_ID, "Archive"),
        titled_node(&graph.id, &graph.title),
    ]
}

/// Make sure the primary store carries the set type and the type tables of `types` (existing
/// Tome corpora opened as projects, or a node type an extension adds on first use). Only
/// touches a writable primary; a read-only one is left as is and records of a missing type can
/// then only be read.
pub fn ensure_node_types(session: &TomeSession, types: &[&NodeType]) -> Result<(), DbError> {
    let primary = session.store().primary().clone();
    if primary.is_readonly() {
        return Ok(());
    }
    let mut model_files: Vec<(&str, String)> = Vec::new();
    let tables = primary.read_table_schemas_file()?;
    let missing: Vec<&NodeType> = types
        .iter()
        .copied()
        .filter(|t| !tables.tables.contains_key(&t.id))
        .collect();
    if !missing.is_empty() {
        let mut merged = tables.tables.clone();
        for node_type in missing {
            merged.insert(node_type.id.clone(), node_type.table_schema());
        }
        let file = json!({ "version": tables.version.max(1), "tables": merged });
        model_files.push((TABLE_SCHEMAS_FILENAME, js_json_file(&file)));
    }
    let registry = primary.read_relationship_types_file()?;
    let has_set_type = !set_trait_relationship_type_ids(&registry).is_empty();
    if !has_set_type {
        let raw = std::fs::read_to_string(
            tome_flatfile::content_model_dir(&primary.content_dir).join(ASSOCIATIONS_FILENAME),
        )
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or_else(|| json!({ "version": 1, "associations": {} }));
        let mut file = raw;
        if let Some(assoc) = file.get_mut("associations").and_then(|a| a.as_object_mut()) {
            assoc.insert(
                SET_TYPE_ID.into(),
                json!({ "perspectives": ["Members", "Member of"], "traits": ["Set"] }),
            );
        }
        model_files.push((ASSOCIATIONS_FILENAME, js_json_file(&file)));
    }
    let nodes: Vec<Node> = types
        .iter()
        .filter(|t| session.store().locate_node(&t.id).is_none())
        .map(|t| titled_node(&t.id, &t.title))
        .collect();
    if model_files.is_empty() && nodes.is_empty() {
        return Ok(());
    }
    session.bootstrap_corpus(&primary.corpus_id, &model_files, &nodes, &[])
}

/// [`ensure_node_types`] for Lab's own model (Imp graphs), at project open.
pub fn ensure_lab_model(session: &TomeSession) -> Result<(), DbError> {
    ensure_node_types(session, &[&NodeType::imp_graph()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_ids_are_ulids() {
        for id in [IMP_GRAPH_TABLE_ID, SET_TYPE_ID, HOME_NODE_ID, ARCHIVE_NODE_ID] {
            assert!(tome_flatfile::is_node_id(id), "{id}");
        }
    }
}
