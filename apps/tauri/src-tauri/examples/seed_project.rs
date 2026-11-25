//! Build a sample project from standalone JSON documents.
//!
//! ```text
//! cargo run -p imp_lab --example seed_project -- [--extensions <extensions.json>] \
//!   <project-dir> <name> <doc.json>...
//! ```
//!
//! `<project-dir>` must not already contain a project. Each document becomes a record in the
//! project's primary store (`content/`); documents with a node id keep it. Imp graphs
//! (`"type": "imp-graph"`) are always known. `--extensions` adds the node types of that
//! config's `node-type-editor` components, matched by their `documentType`.

use std::path::PathBuf;

fn main() {
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let mut editors = Vec::new();
    if args.first().is_some_and(|a| a == "--extensions") {
        let Some(path) = args.get(1).cloned() else {
            usage();
        };
        args.drain(..2);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("error: {}: {e}", PathBuf::from(&path).display());
            std::process::exit(1);
        });
        let config = imp_lab_extensions::parse_extensions_json(&text).unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });
        let (decls, warnings) = imp_lab_extensions::node_type_editors(&config);
        for warning in warnings {
            eprintln!("warning: {warning}");
        }
        editors = decls;
    }
    let mut args = args.into_iter();
    let (Some(dir), Some(name)) = (args.next(), args.next()) else {
        usage();
    };
    let files: Vec<PathBuf> = args.map(PathBuf::from).collect();
    let types = imp_lab_lib::lab_model::node_types_with(&editors);
    let cache = tempfile_dir();
    let result = imp_lab_lib::seed::seed_project(
        &PathBuf::from(dir),
        &name.to_string_lossy(),
        &files,
        &types,
        &cache,
    );
    let _ = std::fs::remove_dir_all(&cache);
    match result {
        Ok((config, records)) => {
            println!("wrote {}", config.display());
            for r in records {
                println!(
                    "  {} {} \"{}\" <- {}",
                    r.node_type,
                    r.node_id,
                    r.title,
                    r.source.display()
                );
            }
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}

fn usage() -> ! {
    eprintln!("usage: seed_project [--extensions <extensions.json>] <project-dir> <name> <doc.json>...");
    std::process::exit(2);
}

/// Throwaway query cache (derived data; the project itself is the flat files).
fn tempfile_dir() -> PathBuf {
    std::env::temp_dir().join(format!("imp-lab-seed-{}", std::process::id()))
}
