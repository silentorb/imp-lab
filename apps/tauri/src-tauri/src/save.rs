//! Thin filesystem save for the Imp Lab desktop app.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDocumentPayload {
    /// Document kind: `imp-graph` | `notation`.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Bare Imp graph body when kind is `imp-graph`.
    #[serde(default)]
    pub graph: Option<Value>,
    /// Aura notation tip when kind is `notation`.
    #[serde(default)]
    pub notation: Option<Value>,
    /// Existing filesystem path; when omitted/empty, Save As is required.
    pub path: Option<String>,
}

fn default_kind() -> String {
    "imp-graph".into()
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub origin: String,
    pub title: String,
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "Untitled".to_string())
}

fn write_imp_graph(path: &Path, graph: &Value) -> Result<(), String> {
    let nodes = graph
        .get("nodes")
        .ok_or_else(|| "graph is missing \"nodes\"".to_string())?;
    let edges = graph
        .get("edges")
        .ok_or_else(|| "graph is missing \"edges\"".to_string())?;
    let doc = json!({
        "type": "imp-graph",
        "nodes": nodes,
        "edges": edges,
    });
    let text = serde_json::to_string_pretty(&doc)
        .map_err(|err| format!("failed to serialize graph: {err}"))?;
    fs::write(path, text).map_err(|err| format!("failed to write file: {err}"))?;
    Ok(())
}

fn write_notation(path: &Path, notation: &Value) -> Result<(), String> {
    let type_id = notation
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("notation");
    if type_id != "notation" {
        return Err(format!("expected notation type, got \"{type_id}\""));
    }
    let version = notation
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| "notation is missing version".to_string())?;
    if version != 1 {
        return Err(format!("unsupported notation version {version}"));
    }
    let text = serde_json::to_string_pretty(notation)
        .map_err(|err| format!("failed to serialize notation: {err}"))?;
    fs::write(path, text).map_err(|err| format!("failed to write file: {err}"))?;
    Ok(())
}

fn write_payload(path: &Path, payload: &SaveDocumentPayload) -> Result<(), String> {
    match payload.kind.as_str() {
        "notation" => {
            let notation = payload
                .notation
                .as_ref()
                .ok_or_else(|| "notation payload missing notation body".to_string())?;
            write_notation(path, notation)
        }
        "imp-graph" => {
            let graph = payload
                .graph
                .as_ref()
                .ok_or_else(|| "graph payload missing graph body".to_string())?;
            write_imp_graph(path, graph)
        }
        other => Err(format!("unsupported save kind \"{other}\"")),
    }
}

fn default_file_name(kind: &str) -> &'static str {
    match kind {
        "notation" => "untitled-notation.json",
        _ => "untitled.json",
    }
}

fn filter_label(kind: &str) -> &'static str {
    match kind {
        "notation" => "Aura notation JSON",
        _ => "Imp graph JSON",
    }
}

/// Overwrite a known path with a document payload.
#[tauri::command]
pub fn save_document_file(payload: SaveDocumentPayload) -> Result<SaveResult, String> {
    let path_str = payload
        .path
        .clone()
        .filter(|p| !p.is_empty() && p != "untitled")
        .ok_or_else(|| "save_document_file requires a filesystem path".to_string())?;
    let path = PathBuf::from(&path_str);
    write_payload(&path, &payload)?;
    Ok(SaveResult {
        origin: path.display().to_string(),
        title: title_from_path(&path),
    })
}

/// Native Save As dialog, then write document JSON.
#[tauri::command]
pub fn pick_and_save_document(
    app: AppHandle,
    payload: SaveDocumentPayload,
) -> Result<Option<SaveResult>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter(filter_label(&payload.kind), &["json"])
        .set_file_name(default_file_name(&payload.kind))
        .blocking_save_file();

    let Some(file_path) = picked else {
        return Ok(None);
    };

    let path = file_path
        .into_path()
        .map_err(|err| format!("could not resolve save path: {err}"))?;
    write_payload(&path, &payload)?;
    Ok(Some(SaveResult {
        origin: path.display().to_string(),
        title: title_from_path(&path),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("imp-lab-save-{name}-{nanos}.json"))
    }

    #[test]
    fn writes_imp_graph_envelope() {
        let path = temp_path("graph");
        let graph = json!({
            "nodes": {
                "a": { "id": "a", "type": "output", "inputs": {} }
            },
            "edges": {}
        });
        write_imp_graph(&path, &graph).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_file(&path);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["type"], "imp-graph");
        assert!(value["nodes"].get("a").is_some());
        assert!(value["edges"].as_object().unwrap().is_empty());
    }

    #[test]
    fn writes_notation_tip() {
        let path = temp_path("notation");
        let notation = json!({
            "type": "notation",
            "id": "tip1",
            "version": 1,
            "tempoBpm": 120,
            "timeSignature": { "beatsPerBar": 4, "beatUnit": 4 },
            "events": []
        });
        write_notation(&path, &notation).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_file(&path);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["type"], "notation");
        assert_eq!(value["id"], "tip1");
    }
}
