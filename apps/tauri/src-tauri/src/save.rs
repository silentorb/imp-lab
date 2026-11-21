//! Thin filesystem save for the experimental Imp Lab Tauri host.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveGraphPayload {
    pub graph: Value,
    /// Existing filesystem path; when omitted/empty, Save As is required.
    pub path: Option<String>,
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

/// Overwrite a known path with an Imp graph document.
#[tauri::command]
pub fn save_document_file(payload: SaveGraphPayload) -> Result<SaveResult, String> {
    let path_str = payload
        .path
        .filter(|p| !p.is_empty() && p != "untitled")
        .ok_or_else(|| "save_document_file requires a filesystem path".to_string())?;
    let path = PathBuf::from(&path_str);
    write_imp_graph(&path, &payload.graph)?;
    Ok(SaveResult {
        origin: path.display().to_string(),
        title: title_from_path(&path),
    })
}

/// Native Save As dialog, then write Imp graph JSON.
#[tauri::command]
pub fn pick_and_save_document(
    app: AppHandle,
    payload: SaveGraphPayload,
) -> Result<Option<SaveResult>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Imp graph JSON", &["json"])
        .set_file_name("untitled.json")
        .blocking_save_file();

    let Some(file_path) = picked else {
        return Ok(None);
    };

    let path = file_path
        .into_path()
        .map_err(|err| format!("could not resolve save path: {err}"))?;
    write_imp_graph(&path, &payload.graph)?;
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
        std::env::temp_dir().join(format!("imp-lab-tauri-save-{name}-{nanos}.json"))
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
}
