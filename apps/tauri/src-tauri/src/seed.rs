//! Build a project from standalone lab JSON documents (`imp-graph` / `notation` tips).
//!
//! Tooling only (the `seed_project` example): products use it to ship sample projects. The app
//! itself has no legacy-file import. Documents that carry a node id keep it, so records that
//! reference each other by id (a graph using a notation) still resolve.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::lab_model::{KIND_IMP_GRAPH, KIND_NOTATION};
use crate::project::{create_project_files, open_project};
use crate::records::{create_record_with_id, RecordPayload};

/// One record written by [`seed_project`].
#[derive(Debug, Clone, PartialEq)]
pub struct SeededRecord {
    pub source: PathBuf,
    pub kind: &'static str,
    pub node_id: String,
    pub title: String,
}

/// Payload, explicit id and title for one document. The title is the file stem.
fn document_payload(path: &Path) -> Result<(RecordPayload, Option<String>, String), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("io: {}: {e}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|e| format!("json: {}: {e}", path.display()))?;
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| tome_flatfile::is_node_id(id))
        .map(str::to_string);
    let payload = match value.get("type").and_then(Value::as_str) {
        Some(KIND_IMP_GRAPH) => RecordPayload {
            kind: KIND_IMP_GRAPH.into(),
            graph: Some(value),
            notation: None,
        },
        Some(KIND_NOTATION) => RecordPayload {
            kind: KIND_NOTATION.into(),
            graph: None,
            notation: Some(value),
        },
        other => {
            return Err(format!(
                "unsupported_kind: {}: type {other:?} is not a graph or notation",
                path.display()
            ))
        }
    };
    Ok((payload, id, title))
}

/// Create a project named `name` in `dir` (which must not already hold one) and add every
/// document in `files` to its primary store. Returns the config path and the records.
pub fn seed_project(
    dir: &Path,
    name: &str,
    files: &[PathBuf],
    cache_dir: &Path,
) -> Result<(PathBuf, Vec<SeededRecord>), String> {
    // Parse everything first so a bad file leaves no half-written project behind.
    let documents = files
        .iter()
        .map(|path| document_payload(path).map(|doc| (path, doc)))
        .collect::<Result<Vec<_>, _>>()?;
    let config_path = create_project_files(dir, name)?;
    let project = open_project(&config_path, cache_dir)?;
    let primary = project.session.store().primary().corpus_id.clone();
    let mut seeded = Vec::new();
    for (path, (payload, id, title)) in documents {
        let saved =
            create_record_with_id(&project.session, id.as_deref(), &title, &primary, &payload)?;
        seeded.push(SeededRecord {
            source: path.clone(),
            kind: if payload.kind == KIND_IMP_GRAPH {
                KIND_IMP_GRAPH
            } else {
                KIND_NOTATION
            },
            node_id: saved.record.node_id,
            title: saved.title,
        });
    }
    Ok((config_path, seeded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::open_record;
    use serde_json::json;

    #[test]
    fn seeds_graphs_and_notations_keeping_notation_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let graph = src.join("song.json");
        std::fs::write(
            &graph,
            json!({ "type": "imp-graph", "nodes": {}, "edges": {} }).to_string(),
        )
        .unwrap();
        let notation = src.join("arp.json");
        std::fs::write(
            &notation,
            json!({
                "type": "notation", "id": "01M7HJ0CRN88N5QEQQQ1GBJT8H", "title": "arp",
                "version": 1, "tempoBpm": 120.0,
                "timeSignature": { "beatsPerBar": 4, "beatUnit": 4 }, "events": []
            })
            .to_string(),
        )
        .unwrap();
        let bad = src.join("library.json");
        std::fs::write(&bad, json!({ "type": "imp-graph-library" }).to_string()).unwrap();

        let project_dir = tmp.path().join("project");
        let cache = tmp.path().join("cache");
        let err = seed_project(&project_dir, "Demo", &[graph.clone(), bad], &cache).unwrap_err();
        assert!(err.starts_with("unsupported_kind"), "{err}");
        assert!(!project_dir.exists(), "nothing written on a bad input");

        let (config, records) =
            seed_project(&project_dir, "Demo", &[graph, notation], &cache).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].title, "song");
        assert_eq!(records[1].node_id, "01M7HJ0CRN88N5QEQQQ1GBJT8H");

        let reopened = open_project(&config, &cache).unwrap();
        let doc = open_record(&reopened.session, "01M7HJ0CRN88N5QEQQQ1GBJT8H").unwrap();
        assert_eq!(doc.kind, KIND_NOTATION);
        let doc = open_record(&reopened.session, &records[0].node_id).unwrap();
        assert_eq!(doc.kind, KIND_IMP_GRAPH);
    }
}
