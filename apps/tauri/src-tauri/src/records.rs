//! Records: lab documents stored as Tome nodes in the project's data stores.
//!
//! Replaces the old filesystem open / save. A record is identified by its node id (unique
//! across every store of a session); the store it lives in is chosen when it is first saved.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;
use tome_db::{NewNode, NodeFilter, NodeSummary, TomeSession};
use tome_graph_interfaces::Properties;

use crate::lab_model::{self, KIND_IMP_GRAPH, KIND_NOTATION};
use crate::project::{db_error, ProjectState};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PropertyRow {
    pub label: String,
    pub value: String,
}

/// Where a record lives.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordRef {
    pub node_id: String,
    pub corpus_id: String,
    /// Content root of the owning store.
    pub content_dir: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub record: RecordRef,
    pub summary: String,
    pub properties: Vec<PropertyRow>,
    /// Bare Imp `{ nodes, edges }` for graph records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph: Option<Value>,
    /// Aura notation JSON for notation records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notation: Option<Value>,
}

/// Explorer row.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordSummary {
    pub node_id: String,
    pub title: String,
    /// `imp-graph` | `notation` | `other` (other nodes are listed but not openable).
    pub kind: String,
    pub corpus_id: Option<String>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordsFilter {
    /// `imp-graph` | `notation` | `lab` (both) | `all`.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub corpus_id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordPayload {
    pub kind: String,
    #[serde(default)]
    pub graph: Option<Value>,
    #[serde(default)]
    pub notation: Option<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub record: RecordRef,
    pub title: String,
}

fn row(label: &str, value: impl Into<String>) -> PropertyRow {
    PropertyRow {
        label: label.into(),
        value: value.into(),
    }
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

fn validate_notation(notation: &Value) -> Result<Value, String> {
    let type_id = notation
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("notation");
    if type_id != "notation" {
        return Err(format!(
            "invalid_notation: expected notation type, got \"{type_id}\""
        ));
    }
    let version = notation
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| "invalid_notation: notation is missing version".to_string())?;
    if version != 1 {
        return Err(format!(
            "invalid_notation: unsupported notation version {version}"
        ));
    }
    let tempo = notation
        .get("tempoBpm")
        .and_then(Value::as_f64)
        .ok_or_else(|| "invalid_notation: notation is missing tempoBpm".to_string())?;
    if !tempo.is_finite() || tempo <= 0.0 {
        return Err("invalid_notation: notation tempoBpm must be positive".into());
    }
    notation
        .get("timeSignature")
        .ok_or_else(|| "invalid_notation: notation is missing timeSignature".to_string())?;
    Ok(notation.clone())
}

/// `(type table, column, validated body)` for a payload.
fn payload_body(payload: &RecordPayload) -> Result<(&'static str, &'static str, Value), String> {
    let (table, column) = lab_model::table_for_kind(&payload.kind)
        .ok_or_else(|| format!("unsupported_kind: \"{}\"", payload.kind))?;
    let body = match payload.kind.as_str() {
        KIND_IMP_GRAPH => validate_graph(
            payload
                .graph
                .as_ref()
                .ok_or_else(|| "invalid_graph: payload missing graph".to_string())?,
        )?,
        _ => validate_notation(
            payload
                .notation
                .as_ref()
                .ok_or_else(|| "invalid_notation: payload missing notation".to_string())?,
        )?,
    };
    Ok((table, column, body))
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

fn kind_of(summary: &NodeSummary) -> &'static str {
    lab_model::kind_for_tables(&summary.type_table_ids).unwrap_or("other")
}

pub fn list_records(
    session: &TomeSession,
    filter: &RecordsFilter,
) -> Result<Vec<RecordSummary>, String> {
    let kind = filter.kind.as_deref().unwrap_or("lab");
    let type_table_ids = match kind {
        KIND_IMP_GRAPH => vec![lab_model::IMP_GRAPH_TABLE_ID.to_string()],
        KIND_NOTATION => vec![lab_model::NOTATION_TABLE_ID.to_string()],
        "lab" => vec![
            lab_model::IMP_GRAPH_TABLE_ID.to_string(),
            lab_model::NOTATION_TABLE_ID.to_string(),
        ],
        _ => Vec::new(),
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
            kind: kind_of(n).into(),
            corpus_id: n.corpus_id.clone(),
            modified_at: n.modified_at.clone(),
        })
        .collect())
}

fn kind_for_node(session: &TomeSession, node_id: &str) -> Result<Option<&'static str>, String> {
    let found = session
        .list_nodes(&NodeFilter {
            type_table_ids: vec![
                lab_model::IMP_GRAPH_TABLE_ID.to_string(),
                lab_model::NOTATION_TABLE_ID.to_string(),
            ],
            include_archived: true,
            ..Default::default()
        })
        .map_err(db_error)?;
    Ok(found
        .iter()
        .find(|n| n.id == node_id)
        .and_then(|n| lab_model::kind_for_tables(&n.type_table_ids)))
}

pub fn open_record(session: &TomeSession, node_id: &str) -> Result<DocumentSummary, String> {
    let node = session
        .get_node(node_id)
        .map_err(db_error)?
        .ok_or_else(|| format!("not_found: record {node_id}"))?;
    let kind = kind_for_node(session, node_id)?
        .ok_or_else(|| "not_a_lab_record: this node is not an Imp graph or notation".to_string())?;
    let record = record_ref(session, node_id)?;
    let title = tome_db::title_from_properties(&node.properties);
    let store_row = row("Store", record.corpus_id.clone());
    let id_row = row("Node id", node_id);
    let empty_graph = || serde_json::json!({ "nodes": {}, "edges": {} });
    if kind == KIND_IMP_GRAPH {
        let graph = node
            .properties
            .get("graph")
            .filter(|g| g.is_object())
            .map(validate_graph)
            .transpose()?
            .unwrap_or_else(empty_graph);
        let nodes = graph["nodes"].as_object().map(|m| m.len()).unwrap_or(0);
        let edges = graph["edges"].as_object().map(|m| m.len()).unwrap_or(0);
        return Ok(DocumentSummary {
            id: node_id.into(),
            title: title.clone(),
            kind: KIND_IMP_GRAPH.into(),
            record,
            summary: format!("Imp graph · {nodes} nodes · {edges} edges"),
            properties: vec![
                row("Type", KIND_IMP_GRAPH),
                row("Title", title),
                store_row,
                id_row,
                row("Nodes", nodes.to_string()),
                row("Edges", edges.to_string()),
            ],
            graph: Some(graph),
            notation: None,
        });
    }
    let notation = node
        .properties
        .get("notation")
        .cloned()
        .ok_or_else(|| "invalid_notation: record has no notation body".to_string())
        .and_then(|n| validate_notation(&n))?;
    let events = notation
        .get("events")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);
    let tempo = notation
        .get("tempoBpm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    Ok(DocumentSummary {
        id: node_id.into(),
        title: title.clone(),
        kind: KIND_NOTATION.into(),
        record,
        summary: format!(
            "Notation · {events} note{} · {tempo} BPM",
            if events == 1 { "" } else { "s" }
        ),
        properties: vec![
            row("Type", KIND_NOTATION),
            row("Title", title),
            store_row,
            id_row,
            row("Events", events.to_string()),
            row("Tempo", tempo.to_string()),
        ],
        graph: None,
        notation: Some(notation),
    })
}

/// Create a new record in `corpus_id` (the save target picked by the user).
pub fn create_record(
    session: &TomeSession,
    title: &str,
    corpus_id: &str,
    payload: &RecordPayload,
) -> Result<SaveResult, String> {
    create_record_with_id(session, None, title, corpus_id, payload)
}

/// [`create_record`] with an explicit node id, so imported documents keep the ids other
/// records reference them by.
pub fn create_record_with_id(
    session: &TomeSession,
    id: Option<&str>,
    title: &str,
    corpus_id: &str,
    payload: &RecordPayload,
) -> Result<SaveResult, String> {
    let (table, column, body) = payload_body(payload)?;
    let mut properties = Properties::new();
    properties.insert(column.into(), body);
    let node = session
        .create_node(NewNode {
            id: id.map(str::to_string),
            title: title.into(),
            corpus: Some(corpus_id.into()),
            properties,
            type_tables: vec![table.into()],
            ..Default::default()
        })
        .map_err(db_error)?;
    Ok(SaveResult {
        record: record_ref(session, &node.id)?,
        title: tome_db::title_from_properties(&node.properties),
    })
}

/// Overwrite an existing record's body.
pub fn save_record(
    session: &TomeSession,
    node_id: &str,
    payload: &RecordPayload,
) -> Result<SaveResult, String> {
    let (table, column, body) = payload_body(payload)?;
    session
        .add_set_membership(node_id, table)
        .map_err(db_error)?;
    session
        .set_structured_property(node_id, table, column, body)
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
    list_records(&session, &filter.unwrap_or_default())
}

#[tauri::command]
pub fn record_open(
    state: State<'_, ProjectState>,
    node_id: String,
) -> Result<DocumentSummary, String> {
    let session = state.session()?;
    open_record(&session, &node_id)
}

#[tauri::command]
pub fn record_create(
    state: State<'_, ProjectState>,
    title: String,
    corpus_id: String,
    payload: RecordPayload,
) -> Result<SaveResult, String> {
    let session = state.session()?;
    create_record(&session, &title, &corpus_id, &payload)
}

#[tauri::command]
pub fn record_save(
    state: State<'_, ProjectState>,
    node_id: String,
    payload: RecordPayload,
) -> Result<SaveResult, String> {
    let session = state.session()?;
    save_record(&session, &node_id, &payload)
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
mod tests {
    use super::*;
    use crate::project::{add_store_to_config, create_project_files, open_project};
    use serde_json::json;

    fn graph_payload() -> RecordPayload {
        RecordPayload {
            kind: KIND_IMP_GRAPH.into(),
            graph: Some(
                json!({ "nodes": { "a": { "id": "a", "type": "imp.add", "inputs": {} } }, "edges": {} }),
            ),
            notation: None,
        }
    }

    fn notation_payload() -> RecordPayload {
        RecordPayload {
            kind: KIND_NOTATION.into(),
            graph: None,
            notation: Some(json!({ "type": "notation", "version": 1, "tempoBpm": 96,
                                   "timeSignature": [4, 4], "events": [] })),
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

        let graph = create_record(session, "Groove", "main", &graph_payload()).unwrap();
        assert_eq!(graph.record.corpus_id, "main");
        let notation = create_record(session, "Melody", "extra", &notation_payload()).unwrap();
        assert_eq!(notation.record.corpus_id, "extra");

        let err = create_record(session, "Nope", "frozen", &graph_payload()).unwrap_err();
        assert!(err.starts_with("corpus_readonly"), "{err}");

        let mut edited = graph_payload();
        edited.graph = Some(json!({ "nodes": {}, "edges": {}, "type": "imp-graph" }));
        save_record(session, &graph.record.node_id, &edited).unwrap();
        let doc = open_record(session, &graph.record.node_id).unwrap();
        assert_eq!(doc.kind, KIND_IMP_GRAPH);
        assert_eq!(doc.graph, Some(json!({ "nodes": {}, "edges": {} })));
        assert_eq!(doc.title, "Groove");

        let doc = open_record(session, &notation.record.node_id).unwrap();
        assert_eq!(doc.kind, KIND_NOTATION);
        assert_eq!(doc.notation.unwrap()["tempoBpm"], json!(96));

        let all = list_records(session, &RecordsFilter::default()).unwrap();
        assert_eq!(all.len(), 2);
        let graphs = list_records(
            session,
            &RecordsFilter {
                kind: Some(KIND_IMP_GRAPH.into()),
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
            &RecordsFilter {
                corpus_id: Some("extra".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(in_extra.len(), 1);
        let others = list_records(
            session,
            &RecordsFilter {
                kind: Some("all".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(others
            .iter()
            .any(|r| r.kind == "other" && r.title == "Home"));
        assert!(open_record(
            session,
            &others.iter().find(|r| r.kind == "other").unwrap().node_id
        )
        .is_err());

        session
            .rename_node(&graph.record.node_id, "Groove 2")
            .unwrap();
        session.archive_node(&notation.record.node_id).unwrap();
        let after = list_records(session, &RecordsFilter::default()).unwrap();
        assert_eq!(
            after.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(),
            vec!["Groove 2"]
        );
        drop(project);

        // Reopen from disk: records survive, cache rebuilt from files.
        let reopened = open_project(&config_path, &tmp.path().join("cache2")).unwrap();
        let doc = open_record(&reopened.session, &graph.record.node_id).unwrap();
        assert_eq!(doc.title, "Groove 2");
        assert!(dir.join("content/data/nodes").read_dir().unwrap().count() > 0);
    }

    #[test]
    fn resolves_notation_records_and_nodes_of_other_graphs() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = create_project_files(&tmp.path().join("lab"), "Lab").unwrap();
        let project = open_project(&config_path, &tmp.path().join("cache")).unwrap();
        let session = &project.session;
        create_record(session, "Groove", "main", &graph_payload()).unwrap();
        let notation = create_record(session, "Melody", "main", &notation_payload()).unwrap();
        let notation_id = notation.record.node_id;

        let ids = vec![notation_id.clone(), "a".to_string(), "nope".to_string()];
        let types = vec![lab_model::NOTATION_TABLE_ID.to_string()];
        let nodes = resolve_nodes(session, &ids, &types).unwrap();
        assert_eq!(
            nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            vec![notation_id.as_str(), "a"]
        );
        assert_eq!(nodes[0].node_type, lab_model::NOTATION_TABLE_ID);
        assert_eq!(nodes[0].title.as_deref(), Some("Melody"));
        assert_eq!(nodes[1].node_type, "imp.add");

        // Without the notation type registered, the record is not an Imp node.
        let nodes = resolve_nodes(session, &ids, &[]).unwrap();
        assert_eq!(nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), vec!["a"]);
    }
}
