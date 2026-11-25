//! Build a project from standalone JSON documents (Imp graphs, or any node type an extension
//! adds).
//!
//! Tooling only (the `seed_project` example): products use it to ship sample projects. The app
//! itself has no legacy-file import. A document's `type` tag picks its node type (Imp graphs:
//! `imp-graph`; extension node types: their editor's `documentType`). Documents that carry a
//! node id keep it, so records that reference each other by id still resolve.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::lab_model::NodeType;
use crate::project::{create_project_files, open_project};
use crate::records::{create_record_with_id, RecordPayload};

/// One record written by [`seed_project`].
#[derive(Debug, Clone, PartialEq)]
pub struct SeededRecord {
    pub source: PathBuf,
    /// Label of the record's node type.
    pub node_type: String,
    pub node_id: String,
    pub title: String,
}

/// Node type, payload, explicit id and title for one document. The title is the file stem.
fn document_payload<'a>(
    types: &'a [NodeType],
    path: &Path,
) -> Result<(&'a NodeType, RecordPayload, Option<String>, String), String> {
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
    let tag = value.get("type").and_then(Value::as_str);
    let node_type = types
        .iter()
        .find(|t| tag.is_some() && t.document_type.as_deref() == tag)
        .ok_or_else(|| {
            format!(
                "unsupported_document_type: {}: type {tag:?} is not a known node type",
                path.display()
            )
        })?;
    let payload = RecordPayload {
        node_type: node_type.id.clone(),
        value,
    };
    Ok((node_type, payload, id, title))
}

/// Create a project named `name` in `dir` (which must not already hold one) and add every
/// document in `files` to its primary store, as records of `types`. Returns the config path
/// and the records.
pub fn seed_project(
    dir: &Path,
    name: &str,
    files: &[PathBuf],
    types: &[NodeType],
    cache_dir: &Path,
) -> Result<(PathBuf, Vec<SeededRecord>), String> {
    // Parse everything first so a bad file leaves no half-written project behind.
    let documents = files
        .iter()
        .map(|path| document_payload(types, path).map(|doc| (path, doc)))
        .collect::<Result<Vec<_>, _>>()?;
    let config_path = create_project_files(dir, name)?;
    let project = open_project(&config_path, cache_dir)?;
    let primary = project.session.store().primary().corpus_id.clone();
    let mut seeded = Vec::new();
    for (path, (node_type, payload, id, title)) in documents {
        let saved = create_record_with_id(
            &project.session,
            types,
            id.as_deref(),
            &title,
            &primary,
            &payload,
        )?;
        seeded.push(SeededRecord {
            source: path.clone(),
            node_type: node_type.label.clone(),
            node_id: saved.record.node_id,
            title: saved.title,
        });
    }
    Ok((config_path, seeded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab_model::IMP_GRAPH_TABLE_ID;
    use crate::records::{open_record, tests::sketch_type, tests::types};
    use serde_json::json;

    #[test]
    fn seeds_graphs_and_extension_types_keeping_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let graph = src.join("song.json");
        std::fs::write(
            &graph,
            json!({ "type": "imp-graph", "nodes": {}, "edges": {} }).to_string(),
        )
        .unwrap();
        let sketch = src.join("doodle.json");
        std::fs::write(
            &sketch,
            json!({ "type": "sketch", "id": "01M7HJ0CRN88N5QEQQQ1GBJT8H", "strokes": [] })
                .to_string(),
        )
        .unwrap();
        let bad = src.join("library.json");
        std::fs::write(&bad, json!({ "type": "imp-graph-library" }).to_string()).unwrap();

        let types = types();
        let project_dir = tmp.path().join("project");
        let cache = tmp.path().join("cache");
        let err = seed_project(&project_dir, "Demo", &[graph.clone(), bad], &types, &cache)
            .unwrap_err();
        assert!(err.starts_with("unsupported_document_type"), "{err}");
        assert!(!project_dir.exists(), "nothing written on a bad input");

        // Without the extension's node type, its documents are not seedable.
        let graph_only = vec![NodeType::imp_graph()];
        assert!(seed_project(&project_dir, "Demo", std::slice::from_ref(&sketch), &graph_only, &cache)
            .is_err());

        let (config, records) =
            seed_project(&project_dir, "Demo", &[graph, sketch], &types, &cache).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].title, "song");
        assert_eq!(records[1].node_id, "01M7HJ0CRN88N5QEQQQ1GBJT8H");
        assert_eq!(records[1].node_type, "Sketch");

        let reopened = open_project(&config, &cache).unwrap();
        let doc = open_record(&reopened.session, &types, "01M7HJ0CRN88N5QEQQQ1GBJT8H").unwrap();
        assert_eq!(doc.node_type, sketch_type().id);
        let doc = open_record(&reopened.session, &types, &records[0].node_id).unwrap();
        assert_eq!(doc.node_type, IMP_GRAPH_TABLE_ID);
    }
}
