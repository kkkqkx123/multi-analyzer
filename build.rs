//! Build script: assembles built-in filter files at compile time.
//!
//! Reads all `src/filters/*.toml` files, concatenates them, and writes
//! the result to `$OUT_DIR/builtin_filters.toml` for inclusion via
//! `include_str!()` in the filter registry.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let filters_dir = PathBuf::from("src/filters");

    if !filters_dir.is_dir() {
        panic!("src/filters/ directory not found");
    }

    let mut combined = String::from("# Built-in filters — assembled by build.rs\n\n");

    let mut entries: Vec<_> = fs::read_dir(&filters_dir)
        .expect("Failed to read src/filters/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        panic!("No .toml filter files found in src/filters/");
    }

    for entry in &entries {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy();
        eprintln!("cargo:warning=Embedding filter: {}", name);
        let content = fs::read_to_string(&path).expect("Failed to read filter file");
        combined.push_str(&content);
        combined.push('\n');
    }

    let dest = out_dir.join("builtin_filters.toml");
    fs::write(&dest, &combined).expect("Failed to write builtin_filters.toml");

    // Re-run build.rs if any filter file changes
    println!("cargo:rerun-if-changed=src/filters/");
}
