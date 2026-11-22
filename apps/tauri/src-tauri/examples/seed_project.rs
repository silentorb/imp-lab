//! Build a sample project from lab JSON documents.
//!
//! ```text
//! cargo run -p imp_lab --example seed_project -- <project-dir> <name> <doc.json>...
//! ```
//!
//! `<project-dir>` must not already contain a project. Each document becomes a record in the
//! project's primary store (`content/`); notations keep their ids.

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let (Some(dir), Some(name)) = (args.next(), args.next()) else {
        eprintln!("usage: seed_project <project-dir> <name> <doc.json>...");
        std::process::exit(2);
    };
    let files: Vec<PathBuf> = args.map(PathBuf::from).collect();
    let cache = tempfile_dir();
    let result = imp_lab_lib::seed::seed_project(
        &PathBuf::from(dir),
        &name.to_string_lossy(),
        &files,
        &cache,
    );
    let _ = std::fs::remove_dir_all(&cache);
    match result {
        Ok((config, records)) => {
            println!("wrote {}", config.display());
            for r in records {
                println!(
                    "  {} {} \"{}\" <- {}",
                    r.kind,
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

/// Throwaway query cache (derived data; the project itself is the flat files).
fn tempfile_dir() -> PathBuf {
    std::env::temp_dir().join(format!("imp-lab-seed-{}", std::process::id()))
}
