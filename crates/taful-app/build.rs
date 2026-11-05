//! Embeds every file under the workspace `content/` folder, so the app ships
//! its lessons, quizzes and skill tree inside the binary (and loads them with
//! the same code the content checks use).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // Not canonicalized: on Windows that yields a `\\?\` path.
    let root = manifest.join("..").join("..").join("content");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    walk(&root, &mut files);
    let mut code = String::from("&[\n");
    for f in files {
        // Text content only; snapshots are a CI artifact, not app data.
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "md" | "toml" | "json") {
            continue;
        }
        let rel = f
            .strip_prefix(&root)
            .unwrap()
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if rel.contains("/.snapshots/") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", f.display());
        writeln!(
            code,
            "    ({rel:?}, include_str!({:?})),",
            f.display().to_string()
        )
        .unwrap();
    }
    code.push(']');
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("content_files.rs");
    std::fs::write(out, code).unwrap();
}
