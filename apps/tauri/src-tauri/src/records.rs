//! Records: Tome nodes of a node type Lab can open, in the project's data stores.
//!
//! Replaces the old filesystem open / save. A record is identified by its node id (unique
//! across every store of a session); the store it lives in is chosen when it is first saved.
//! What a record is comes from its Tome node type (type-table membership); its value is the
//! node-stored column that node type names. Imp graphs are built in; extensions add node types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;
use tome_db::{NewNode, NodeFilter, NodeSummary, TomeSession};
use tome_graph_interfaces::Properties;

use crate::extensions;
use crate::lab_model::{self, NodeType};
use crate::project::{db_error, ProjectState};

/// Explorer filter value listing every node type Lab can open.
pub const FILTER_OPENABLE: &str = "openable";
/// Explorer filter value listing every node, openable or not.
pub const FILTER_ALL: &str = "all";

/// Where a record lives.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordRef {
    pub node_id: String,
    pub corpus_id: String,
    /// Content root of the owning store.
    pub content_dir: String,
}

/// An opened record.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordDocument {
    pub id: String,
    pub title: String,
    /// Node type (type-table id) Lab opened it as.
    pub node_type: String,
    pub record: RecordRef,
    /// The node type's column value (`null` when the node has none yet). Imp graphs are bare
    /// `{ nodes, edges }`.
    pub value: Value,
}

/// Explorer row.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordSummary {
    pub node_id: String,
    pub title: String,
    /// Node type Lab opens it as; `None` for nodes Lab lists but cannot open.
    pub node_type: Option<String>,
    pub corpus_id: Option<String>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordsFilter {
    /// A node type id, [`FILTER_OPENABLE`] (default) or [`FILTER_ALL`].
    #[serde(default)]
    pub node_type: Option<String>,
    #[serde(default)]
    pub corpus_id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordPayload {
    pub node_type: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub record: RecordRef,
    pub title: String,
}

/// Bare `{ nodes, edges }` (an `imp-graph` envelope `type` is dropped).
fn validate_graph(graph: &Value) -> Result<Value, String> {
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_object)
        .ok_or_else(|| "invalid_graph: graph is missing a \"nodes\" object".to_string())?;
    let edges = graph
        .get("edges")
        .and_then(Value::as_object)
        .ok_or_else(|| "invalid_graph: graph is missing an \"edges\" object".to_string())?;
    let mut out = serde_json::Map::new();
    out.insert("nodes".into(), Value::Object(nodes.clone()));
    out.insert("edges".into(), Value::Object(edges.clone()));
    Ok(Value::Object(out))
}

/// Imp graphs are validated here; other node types' values belong to their editors.
fn validate_value(node_type: &NodeType, value: &Value) -> Result<Value, String> {
    if node_type.is_imp_graph() {
        validate_graph(value)
    } else {
        Ok(value.clone())
    }
}

fn find_type<'a>(types: &'a [NodeType], id: &str) -> Result<&'a NodeType, String> {
    types
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("unsupported_node_type: Lab has no editor for node type \"{id}\""))
}

fn record_ref(session: &TomeSession, node_id: &str) -> Result<RecordRef, String> {
    let corpus_id = session
        .store()
        .locate_node(node_id)
        .ok_or_else(|| format!("not_found: record {node_id}"))?
        .to_string();
    let content_dir = session
        .store()
        .corpus(&corpus_id)
        .map(|c| c.content_dir.display().to_string())
        .unwrap_or_default();
    Ok(RecordRef {
        node_id: node_id.into(),
        corpus_id,
        content_dir,
    })
}

fn summary_type(types: &[NodeType], summary: &NodeSummary) -> Option<String> {
    lab_model::node_type_of(types, &summary.type_table_ids).map(|t| t.id.clone())
}

pub fn list_records(
    session: &TomeSession,
    types: &[NodeType],
    filter: &RecordsFilter,
) -> Result<Vec<RecordSummary>, String> {
    let type_table_ids = match filter.node_type.as_deref().unwrap_or(FILTER_OPENABLE) {
        FILTER_OPENABLE => types.iter().map(|t| t.id.clone()).collect(),
        FILTER_ALL => Vec::new(),
        id => vec![id.to_string()],
    };
    let nodes = session
        .list_nodes(&NodeFilter {
            type_table_ids,
            corpus: filter.corpus_id.clone().filter(|c| !c.is_empty()),
            title_contains: filter.text.clone(),
            ..Default::default()
        })
        .map_err(db_error)?;
    Ok(nodes
        .iter()
        .map(|n| RecordSummary {
            node_id: n.id.clone(),
            title: n.title.clone(),
            node_type: summary_type(types, n),
            corpus_id: n.corpus_id.clone(),
            modified_at: n.modified_at.clone(),
        })
        .collect())
}

fn type_of_node<'a>(
    session: &TomeSession,
    types: &'a [NodeType],
    node_id: &str,
) -> Result<Option<&'a NodeType>, String> {
    let found = session
        .list_nodes(&NodeFilter {
            type_table_ids: types.iter().map(|t| t.id.clone()).collect(),
            include_archived: true,
            ..Default::default()
        })
        .map_err(db_error)?;
    Ok(found
        .iter()
        .find(|n| n.id == node_id)
        .and_then(|n| lab_model::node_type_of(types, &n.type_table_ids)))
}

pub fn open_record(
    session: &TomeSession,
    types: &[NodeType],
    node_id: &str,
) -> Result<RecordDocument, String> {
    let node = session
        .get_node(node_id)
        .map_err(db_error)?
        .ok_or_else(|| format!("not_found: record {node_id}"))?;
    let node_type = type_of_node(session, types, node_id)?.ok_or_else(|| {
        "not_openable: no editor for this node's type (an extension may provide one)".to_string()
    })?;
    let stored = node
        .properties
        .get(&node_type.column)
        .filter(|v| !v.is_null())
        .cloned();
    let value = match stored {
        Some(v) => validate_value(node_type, &v)?,
        None if node_type.is_imp_graph() => serde_json::json!({ "nodes": {}, "edges": {} }),
        None => Value::Null,
    };
    Ok(RecordDocument {
        id: node_id.into(),
        title: tome_db::title_from_properties(&node.properties),
        node_type: node_type.id.clone(),
        record: record_ref(session, node_id)?,
        value,
    })
}

/// Create a new record in `corpus_id` (the save target picked by the user).
pub fn create_record(
    session: &TomeSession,
    types: &[NodeType],
    title: &str,
    corpus_id: &str,
    payload: &RecordPayload,
) -> Result<SaveResult, String> {
    create_record_with_id(session, types, None, title, corpus_id, payload)
}

/// [`create_record`] with an explicit node id, so imported documents keep the ids other
/// records reference them by.
pub fn create_record_with_id(
    session: &TomeSession,
    types: &[NodeType],
    id: Option<&str>,
    title: &str,
    corpus_id: &str,
    payload: &RecordPayload,
) -> Result<SaveResult, String> {
    let node_type = find_type(types, &payload.node_type)?;
    let value = validate_value(node_type, &payload.value)?;
    lab_model::ensure_node_types(session, &[node_type]).map_err(db_error)?;
    let mut properties = Properties::new();
    properties.insert(node_type.column.clone(), value);
    let node = session
        .create_node(NewNode {
            id: id.map(str::to_string),
            title: title.into(),
            corpus: Some(corpus_id.into()),
            properties,
            type_tables: vec![node_type.id.clone()],
            ..Default::default()
        })
        .map_err(db_error)?;
    Ok(SaveResult {
        record: record_ref(session, &node.id)?,
        title: tome_db::title_from_properties(&node.properties),
    })
}

/// Overwrite an existing record's value.
pub fn save_record(
    session: &TomeSession,
    types: &[NodeType],
    node_id: &str,
    payload: &RecordPayload,
) -> Result<SaveResult, String> {
    let node_type = find_type(types, &payload.node_type)?;
    let value = validate_value(node_type, &payload.value)?;
    lab_model::ensure_node_types(session, &[node_type]).map_err(db_error)?;
    session
        .add_set_membership(node_id, &node_type.id)
        .map_err(db_error)?;
    session
        .set_structured_property(node_id, &node_type.id, &node_type.column, value)
        .map_err(db_error)?;
    let node = session
        .get_node(node_id)
        .map_err(db_error)?
        .ok_or_else(|| format!("not_found: record {node_id}"))?;
    Ok(SaveResult {
        record: record_ref(session, node_id)?,
        title: tome_db::title_from_properties(&node.properties),
    })
}

#[tauri::command]
pub fn records_list(
    state: State<'_, ProjectState>,
    filter: Option<RecordsFilter>,
) -> Result<Vec<RecordSummary>, String> {
    let session = state.session()?;
    list_records(&session, &extensions::node_types(), &filter.unwrap_or_default())
}

#[tauri::command]
pub fn record_open(
    state: State<'_, ProjectState>,
    node_id: String,
) -> Result<RecordDocument, String> {
    let session = state.session()?;
    open_record(&session, &extensions::node_types(), &node_id)
}

#[tauri::command]
pub fn record_create(
    state: State<'_, ProjectState>,
    title: String,
    corpus_id: String,
    payload: RecordPayload,
) -> Result<SaveResult, String> {
    let session = state.session()?;
    create_record(&session, &extensions::node_types(), &title, &corpus_id, &payload)
}

#[tauri::command]
pub fn record_save(
    state: State<'_, ProjectState>,
    node_id: String,
    payload: RecordPayload,
) -> Result<SaveResult, String> {
    let session = state.session()?;
    save_record(&session, &extensions::node_types(), &node_id, &payload)
}

#[tauri::command]
pub fn record_rename(
    state: State<'_, ProjectState>,
    node_id: String,
    title: String,
) -> Result<(), String> {
    state
        .session()?
        .rename_node(&node_id, &title)
        .map(|_| ())
        .map_err(db_error)
}

#[tauri::command]
pub fn record_archive(state: State<'_, ProjectState>, node_id: String) -> Result<(), String> {
    state
        .session()?
        .archive_node(&node_id)
        .map(|_| ())
        .map_err(db_error)
}

/// Imp nodes for `ids`, resolved from the project's Tome stores: records that are members of a
/// type hub listed in `node_types` (the hub id is the node type), and nodes embedded in graph
/// records. Ids nothing resolves are left out; the graph view shows them as unresolved.
pub fn resolve_nodes(
    session: &TomeSession,
    ids: &[String],
    node_types: &[String],
) -> Result<Vec<imp_core_types::Node>, String> {
    let types: std::collections::BTreeSet<&str> = node_types.iter().map(String::as_str).collect();
    let provider = tome_imp::from_session(session, |t| types.contains(t)).map_err(|e| e.to_string())?;
    Ok(ids
        .iter()
        .filter_map(|id| imp_core_types::NodeProvider::get_node(&provider, id))
        .map(std::borrow::Cow::into_owned)
        .collect())
}

#[tauri::command]
pub fn records_resolve_nodes(
    state: State<'_, ProjectState>,
    ids: Vec<String>,
    node_types: Vec<String>,
) -> Result<Vec<imp_core_types::Node>, String> {
    let session = state.session()?;
    resolve_nodes(&session, &ids, &node_types)
}


#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::project::{add_store_to_config, create_project_files, open_project};
    use serde_json::json;

    /// A node type an extension would add (`node-type-editor`).
    pub(crate) fn sketch_type() -> NodeType {
        NodeType {
            id: "01M80M9SKETCH0000000000001".into(),
            label: "Sketch".into(),
            title: "Sketches".into(),
            column: "sketch".into(),
            column_type: "json".into(),
            document_type: Some("sketch".into()),
        }
    }

    pub(crate) fn types() -> Vec<NodeType> {
        vec![NodeType::imp_graph(), sketch_type()]
    }

    fn graph_payload() -> RecordPayload {
        RecordPayload {
            node_type: lab_model::IMP_GRAPH_TABLE_ID.into(),
            value: json!({ "nodes": { "a": { "id": "a", "type": "imp.add", "inputs": {} } }, "edges": {} }),
        }
    }

    fn sketch_payload() -> RecordPayload {
        RecordPayload {
            node_type: sketch_type().id,
            value: json!({ "strokes": [1, 2, 3] }),
        }
    }

    #[test]
    fn create_save_reopen_list_and_reject_readonly() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("lab");
        let cache = tmp.path().join("cache");
        let config_path = create_project_files(&dir, "Lab").unwrap();
        add_store_to_config(
            &config_path,
            "frozen",
            &tmp.path().join("frozen"),
            "readonly",
        )
        .unwrap();
        add_store_to_config(&config_path, "extra", &dir.join("extra"), "readwrite").unwrap();
        let project = open_project(&config_path, &cache).unwrap();
        let session = &project.session;
        let types = types();
        let graph_id = lab_model::IMP_GRAPH_TABLE_ID;

        // A new project only knows Imp graphs; the sketch type is added on first use.
        assert!(!session
            .list_type_tables()
            .unwrap()
            .iter()
            .any(|t| t.id == sketch_type().id));

        let graph = create_record(session, &types, "Groove", "main", &graph_payload()).unwrap();
        assert_eq!(graph.record.corpus_id, "main");
        let sketch =
            create_record(session, &types, "Doodle", "extra", &sketch_payload()).unwrap();
        assert_eq!(sketch.record.corpus_id, "extra");
        assert!(session
            .list_type_tables()
            .unwrap()
            .iter()
            .any(|t| t.id == sketch_type().id));

        let err = create_record(session, &types, "Nope", "frozen", &graph_payload()).unwrap_err();
        assert!(err.starts_with("corpus_readonly"), "{err}");
        let unknown = RecordPayload {
            node_type: "01M80M9NOPE00000000000001".into(),
            value: json!({}),
        };
        let err = create_record(session, &types, "Nope", "main", &unknown).unwrap_err();
        assert!(err.starts_with("unsupported_node_type"), "{err}");

        let mut edited = graph_payload();
        edited.value = json!({ "nodes": {}, "edges": {}, "type": "imp-graph" });
        save_record(session, &types, &graph.record.node_id, &edited).unwrap();
        let doc = open_record(session, &types, &graph.record.node_id).unwrap();
        assert_eq!(doc.node_type, graph_id);
        assert_eq!(doc.value, json!({ "nodes": {}, "edges": {} }));
        assert_eq!(doc.title, "Groove");

        let doc = open_record(session, &types, &sketch.record.node_id).unwrap();
        assert_eq!(doc.node_type, sketch_type().id);
        assert_eq!(doc.value, json!({ "strokes": [1, 2, 3] }));

        // Without the extension, the sketch is listed but not openable.
        let graph_only = vec![NodeType::imp_graph()];
        assert!(open_record(session, &graph_only, &sketch.record.node_id)
            .unwrap_err()
            .starts_with("not_openable"));
        let listed = list_records(
            session,
            &graph_only,
            &RecordsFilter {
                node_type: Some(FILTER_ALL.into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(listed
            .iter()
            .any(|r| r.node_id == sketch.record.node_id && r.node_type.is_none()));

        let all = list_records(session, &types, &RecordsFilter::default()).unwrap();
        assert_eq!(all.len(), 2);
        let graphs = list_records(
            session,
            &types,
            &RecordsFilter {
                node_type: Some(graph_id.into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            graphs.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(),
            vec!["Groove"]
        );
        let in_extra = list_records(
            session,
            &types,
            &RecordsFilter {
                corpus_id: Some("extra".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(in_extra.len(), 1);
        let others = list_records(
            session,
            &types,
            &RecordsFilter {
                node_type: Some(FILTER_ALL.into()),
                ..Default::default()
            },
        )
        .unwrap();
        let home = others
            .iter()
            .find(|r| r.title == "Home")
            .expect("home hub listed");
        assert_eq!(home.node_type, None);
        assert!(open_record(session, &types, &home.node_id).is_err());

        session
            .rename_node(&graph.record.node_id, "Groove 2")
            .unwrap();
        session.archive_node(&sketch.record.node_id).unwrap();
        let after = list_records(session, &types, &RecordsFilter::default()).unwrap();
        assert_eq!(
            after.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(),
            vec!["Groove 2"]
        );
        drop(project);

        // Reopen from disk: records survive, cache rebuilt from files.
        let reopened = open_project(&config_path, &tmp.path().join("cache2")).unwrap();
        let doc = open_record(&reopened.session, &types, &graph.record.node_id).unwrap();
        assert_eq!(doc.title, "Groove 2");
        assert!(dir.join("content/data/nodes").read_dir().unwrap().count() > 0);
    }

    #[test]
    fn resolves_records_of_registered_types_and_nodes_of_other_graphs() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = create_project_files(&tmp.path().join("lab"), "Lab").unwrap();
        let project = open_project(&config_path, &tmp.path().join("cache")).unwrap();
        let session = &project.session;
        let types = types();
        create_record(session, &types, "Groove", "main", &graph_payload()).unwrap();
        let sketch = create_record(session, &types, "Doodle", "main", &sketch_payload()).unwrap();
        let sketch_id = sketch.record.node_id;

        let ids = vec![sketch_id.clone(), "a".to_string(), "nope".to_string()];
        let node_types = vec![sketch_type().id];
        let nodes = resolve_nodes(session, &ids, &node_types).unwrap();
        assert_eq!(
            nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            vec![sketch_id.as_str(), "a"]
        );
        assert_eq!(nodes[0].node_type, sketch_type().id);
        assert_eq!(nodes[0].title.as_deref(), Some("Doodle"));
        assert_eq!(nodes[1].node_type, "imp.add");

        // Without the sketch type registered, the record is not an Imp node.
        let nodes = resolve_nodes(session, &ids, &[]).unwrap();
        assert_eq!(nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), vec!["a"]);
    }
}
