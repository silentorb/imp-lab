//! Lab model: the well-known Tome type tables that make a node an Imp graph or a notation.
//!
//! A lab document is one Tome node: its title, plus a node-stored structured column holding the
//! document body (`graph: imp_graph` for Imp graphs, `notation: json` for notations), plus
//! set membership in the matching type table. The ids are fixed so every project (and TS Tome)
//! recognises lab records the same way.

use serde_json::{json, Map, Value};
use tome_db::{DbError, TomeSession};
use tome_flatfile::{
    set_trait_relationship_type_ids, ASSOCIATIONS_FILENAME, CORPUS_FILENAME, TABLE_SCHEMAS_FILENAME,
};
use tome_graph_interfaces::{js_json_file, Node, Properties};

/// Type table whose members are Imp graphs (column `graph`, `imp_graph`).
pub const IMP_GRAPH_TABLE_ID: &str = "01M80M9GRAPH00000000000001";
/// Type table whose members are notations (column `notation`, `json`).
pub const NOTATION_TABLE_ID: &str = "01M80M9N0TAT00000000000001";
/// Set-trait relationship type written into new projects.
pub const SET_TYPE_ID: &str = "01M80M9SET0000000000000001";
/// Home and archive hubs of a new project's primary store.
pub const HOME_NODE_ID: &str = "01M80M9H0ME000000000000001";
pub const ARCHIVE_NODE_ID: &str = "01M80M9ARCH000000000000001";

pub const KIND_IMP_GRAPH: &str = "imp-graph";
pub const KIND_NOTATION: &str = "notation";

/// Document kind → (type table, structured column key).
pub fn table_for_kind(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        KIND_IMP_GRAPH => Some((IMP_GRAPH_TABLE_ID, "graph")),
        KIND_NOTATION => Some((NOTATION_TABLE_ID, "notation")),
        _ => None,
    }
}

/// Kind of a record from its type-table memberships.
pub fn kind_for_tables(type_table_ids: &[String]) -> Option<&'static str> {
    if type_table_ids.iter().any(|t| t == IMP_GRAPH_TABLE_ID) {
        Some(KIND_IMP_GRAPH)
    } else if type_table_ids.iter().any(|t| t == NOTATION_TABLE_ID) {
        Some(KIND_NOTATION)
    } else {
        None
    }
}

fn table_schema_entries() -> Vec<(&'static str, Value)> {
    vec![
        (
            IMP_GRAPH_TABLE_ID,
            json!({ "columns": [
                { "key": "graph", "name": "Graph", "type": "imp_graph", "storage": "node" }
            ] }),
        ),
        (
            NOTATION_TABLE_ID,
            json!({ "columns": [
                { "key": "notation", "name": "Notation", "type": "json", "storage": "node" }
            ] }),
        ),
    ]
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
        "protectedNodeIds": [HOME_NODE_ID, ARCHIVE_NODE_ID, IMP_GRAPH_TABLE_ID, NOTATION_TABLE_ID],
        "graphExplorer": { "defaultAnchorNodeId": HOME_NODE_ID },
        "staticSite": { "homeNodeId": HOME_NODE_ID },
        "quickLinks": [],
        "branding": { "appTitle": name }
    });
    let mut tables = Map::new();
    for (id, schema) in table_schema_entries() {
        tables.insert(id.into(), schema);
    }
    let table_schemas = json!({ "version": 1, "tables": tables });
    vec![
        (ASSOCIATIONS_FILENAME, js_json_file(&associations)),
        (CORPUS_FILENAME, js_json_file(&corpus)),
        (TABLE_SCHEMAS_FILENAME, js_json_file(&table_schemas)),
    ]
}

/// Hub nodes for a brand-new project.
pub fn new_project_nodes() -> Vec<Node> {
    vec![
        titled_node(HOME_NODE_ID, "Home"),
        titled_node(ARCHIVE_NODE_ID, "Archive"),
        titled_node(IMP_GRAPH_TABLE_ID, "Imp Graphs"),
        titled_node(NOTATION_TABLE_ID, "Notations"),
    ]
}

/// Make sure the primary store carries the lab type tables (existing Tome corpora opened as
/// projects). Only touches a writable primary; a read-only one is left as is and lab records
/// can then only be read from it.
pub fn ensure_lab_model(session: &TomeSession) -> Result<(), DbError> {
    let primary = session.store().primary().clone();
    if primary.is_readonly() {
        return Ok(());
    }
    let mut model_files: Vec<(&str, String)> = Vec::new();
    let tables = primary.read_table_schemas_file()?;
    let missing: Vec<(&str, Value)> = table_schema_entries()
        .into_iter()
        .filter(|(id, _)| !tables.tables.contains_key(*id))
        .collect();
    if !missing.is_empty() {
        let mut merged = tables.tables.clone();
        for (id, schema) in missing {
            merged.insert(id.to_string(), schema);
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
    let nodes: Vec<Node> = [
        (IMP_GRAPH_TABLE_ID, "Imp Graphs"),
        (NOTATION_TABLE_ID, "Notations"),
    ]
    .into_iter()
    .filter(|(id, _)| session.store().locate_node(id).is_none())
    .map(|(id, title)| titled_node(id, title))
    .collect();
    if model_files.is_empty() && nodes.is_empty() {
        return Ok(());
    }
    session.bootstrap_corpus(&primary.corpus_id, &model_files, &nodes, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_ids_are_ulids() {
        for id in [
            IMP_GRAPH_TABLE_ID,
            NOTATION_TABLE_ID,
            SET_TYPE_ID,
            HOME_NODE_ID,
            ARCHIVE_NODE_ID,
        ] {
            assert!(tome_flatfile::is_node_id(id), "{id}");
        }
    }
}
