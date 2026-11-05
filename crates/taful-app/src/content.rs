//! Content shipped in the binary (lessons, quizzes, skill tree, themes).

use taful_content::{Files, Library};

/// Every text file under `content/` except snapshots, as `(path, text)`.
pub static CONTENT_FILES: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/content_files.rs"));

/// The embedded content tree.
pub fn files() -> Files {
    Files::from_pairs(CONTENT_FILES.iter().copied())
}

/// Loads the embedded library. Content is checked in CI, so findings here
/// mean a broken build; they are returned for the app to log.
pub fn library() -> (Library, Vec<taful_content::Finding>) {
    Library::load(&files())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_content_loads_cleanly() {
        let (lib, findings) = library();
        assert!(findings.is_empty(), "{findings:?}");
        assert!(!lib.tree.tracks.is_empty());
        assert!(lib.lessons.len() >= 10, "{} lessons", lib.lessons.len());
    }
}
