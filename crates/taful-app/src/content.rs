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

    /// A Windows checkout with `core.autocrlf` embeds CRLF files; every
    /// lesson must still load (the 0-of-0 lessons bug in 0.1.0-126).
    #[test]
    fn embedded_content_loads_with_crlf_line_endings() {
        let crlf: Vec<(String, String)> = CONTENT_FILES
            .iter()
            .map(|(p, t)| ((*p).to_owned(), t.replace('\n', "\r\n")))
            .collect();
        let (lib, findings) = Library::load(&Files::from_pairs(
            crlf.iter().map(|(p, t)| (p.as_str(), t.as_str())),
        ));
        let (lf, _) = library();
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(lib.lessons.len(), lf.lessons.len());
        assert_eq!(lib.quizzes.len(), lf.quizzes.len());
    }
}
