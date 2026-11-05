//! `manifest.json` for a library directory (spec §7.6): every `.tafl` file
//! under it with its size and BLAKE3 hash, so clients can verify downloads.

use std::path::Path;

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, u64, String)>) -> Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(&p, root, out)?;
        } else if p.extension().is_some_and(|x| x == "tafl") {
            let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
            let rel = p
                .strip_prefix(root)
                .expect("under root")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((
                rel,
                bytes.len() as u64,
                blake3::hash(&bytes).to_hex().to_string(),
            ));
        }
    }
    Ok(())
}

/// Write `<root>/manifest.json`; returns the number of files listed.
pub fn write(root: &Path) -> Result<usize, String> {
    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    let list: Vec<serde_json::Value> = files
        .iter()
        .map(|(path, size, hash)| serde_json::json!({ "path": path, "size": size, "blake3": hash }))
        .collect();
    let json = serde_json::json!({ "files": list });
    let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())? + "\n";
    std::fs::write(root.join("manifest.json"), text).map_err(|e| e.to_string())?;
    Ok(files.len())
}
