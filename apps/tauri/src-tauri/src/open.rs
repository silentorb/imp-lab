//! Thin filesystem open for the Imp Lab desktop app.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyRow {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub origin: String,
    pub dirty: bool,
    pub summary: String,
    pub properties: Vec<PropertyRow>,
    /// Bare Imp `{ nodes, edges }` for graph docs (`type` tip stripped when present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph: Option<Value>,
    /// Aura notation tip JSON for notation docs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notation: Option<Value>,
}

/// Native dialog + classify Imp graph / notation JSON into a document summary.
#[tauri::command]
pub fn pick_and_open_document(app: AppHandle) -> Result<Option<DocumentSummary>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Imp / Aura JSON", &["json"])
        .blocking_pick_file();

    let Some(file_path) = picked else {
        return Ok(None);
    };

    let path = file_path
        .into_path()
        .map_err(|err| format!("could not resolve picked path: {err}"))?;
    open_document_at(&path).map(Some)
}

/// Open a known filesystem path (tests / tooling).
#[tauri::command]
pub fn open_document_file(path: String) -> Result<DocumentSummary, String> {
    open_document_at(Path::new(&path))
}

fn open_document_at(path: &Path) -> Result<DocumentSummary, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("failed to read file: {err}"))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|err| format!("invalid JSON: {err}"))?;

    let doc_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "document is missing a string \"type\" field".to_string())?;

    match doc_type {
        "imp-graph" => summarize_imp_graph(path, &value),
        "notation" => summarize_notation(path, value),
        "imp-graph-library" => Err(
            "GraphLibrary catalogs cannot be opened as editor documents yet".to_string(),
        ),
        other => Err(format!("unsupported document type \"{other}\"")),
    }
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "Untitled".to_string())
}

fn origin_label(path: &Path) -> String {
    PathBuf::from(path).display().to_string()
}

fn summarize_imp_graph(path: &Path, value: &Value) -> Result<DocumentSummary, String> {
    let nodes = value
        .get("nodes")
        .and_then(Value::as_object)
        .ok_or_else(|| "imp-graph is missing a \"nodes\" object".to_string())?;
    let edges = value
        .get("edges")
        .and_then(Value::as_object)
        .ok_or_else(|| "imp-graph is missing an \"edges\" object".to_string())?;

    let node_count = nodes.len();
    let edge_count = edges.len();
    let title = title_from_path(path);
    let origin = origin_label(path);
    let id = origin.clone();
    let graph = Value::Object(
        [
            ("nodes".to_string(), Value::Object(nodes.clone())),
            ("edges".to_string(), Value::Object(edges.clone())),
        ]
        .into_iter()
        .collect(),
    );

    Ok(DocumentSummary {
        id,
        title: title.clone(),
        kind: "imp-graph".into(),
        origin: origin.clone(),
        dirty: false,
        summary: format!("Imp graph · {node_count} nodes · {edge_count} edges"),
        graph: Some(graph),
        notation: None,
        properties: vec![
            PropertyRow {
                label: "Type".into(),
                value: "imp-graph".into(),
            },
            PropertyRow {
                label: "Title".into(),
                value: title,
            },
            PropertyRow {
                label: "Path".into(),
                value: origin,
            },
            PropertyRow {
                label: "Nodes".into(),
                value: node_count.to_string(),
            },
            PropertyRow {
                label: "Edges".into(),
                value: edge_count.to_string(),
            },
        ],
    })
}

fn summarize_notation(path: &Path, value: Value) -> Result<DocumentSummary, String> {
    let version = value
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| "notation is missing version".to_string())?;
    if version != 1 {
        return Err(format!("unsupported notation version {version}"));
    }
    let tempo = value
        .get("tempoBpm")
        .and_then(Value::as_f64)
        .ok_or_else(|| "notation is missing tempoBpm".to_string())?;
    if !tempo.is_finite() || tempo <= 0.0 {
        return Err("notation tempoBpm must be positive".into());
    }
    let _ = value
        .get("timeSignature")
        .ok_or_else(|| "notation is missing timeSignature".to_string())?;
    let event_count = value
        .get("events")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);

    let path_title = title_from_path(path);
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(&path_title)
        .to_string();
    let origin = origin_label(path);
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| origin.clone());

    Ok(DocumentSummary {
        id,
        title: title.clone(),
        kind: "notation".into(),
        origin: origin.clone(),
        dirty: false,
        summary: format!(
            "Notation · {event_count} note{} · {tempo} BPM",
            if event_count == 1 { "" } else { "s" }
        ),
        graph: None,
        notation: Some(value),
        properties: vec![
            PropertyRow {
                label: "Type".into(),
                value: "notation".into(),
            },
            PropertyRow {
                label: "Title".into(),
                value: title,
            },
            PropertyRow {
                label: "Path".into(),
                value: origin,
            },
            PropertyRow {
                label: "Events".into(),
                value: event_count.to_string(),
            },
            PropertyRow {
                label: "Tempo".into(),
                value: tempo.to_string(),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp(name: &str, body: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("imp-lab-open-{name}-{nanos}.json"));
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn opens_imp_graph_fixture_shape() {
        let path = write_temp(
            "graph",
            r#"{"type":"imp-graph","nodes":{"a":{"id":"a","type":"output","inputs":{}}},"edges":{}}"#,
        );
        let summary = open_document_at(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(summary.kind, "imp-graph");
        assert!(summary.summary.contains("1 nodes"));
        let graph = summary.graph.expect("imp-graph must carry graph body");
        assert!(graph.get("type").is_none());
        assert!(graph["nodes"].get("a").is_some());
        assert!(graph["edges"].as_object().unwrap().is_empty());
    }

    #[test]
    fn rejects_graph_library() {
        let path = write_temp(
            "library",
            r#"{"type":"imp-graph-library","id":"x","definitions":{}}"#,
        );
        let err = open_document_at(&path).unwrap_err();
        let _ = fs::remove_file(&path);
        assert!(err.contains("GraphLibrary"));
    }

    #[test]
    fn opens_notation() {
        let path = write_temp(
            "notation",
            r#"{"type":"notation","id":"tip1","version":1,"tempoBpm":120,"timeSignature":{"beatsPerBar":4,"beatUnit":4},"events":[{"semitone":60,"startBeats":0,"durationBeats":0.25}]}"#,
        );
        let summary = open_document_at(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(summary.kind, "notation");
        assert!(summary.summary.contains("1 note"));
        let notation = summary.notation.expect("notation body");
        assert_eq!(notation["type"], "notation");
        assert_eq!(notation["id"], "tip1");
    }
}
