//! Taful content (spec §12): lesson, quiz and drill loader, expression
//! language, renderer and content checks. The checks run in CI through
//! `tools/content-check` and in the app at load time.
//!
//! M0 provides the check entry point with no rules yet; loaders and rules land
//! in M2.

use std::fmt;
use std::path::{Path, PathBuf};

/// One problem found by a content check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// File the finding refers to, relative to the content root.
    pub path: PathBuf,
    /// Human-readable description (en-US; checks are a developer tool).
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

/// Runs every content check over the `content/` tree at `root`.
///
/// Returns the findings; an empty list means the tree is valid. An IO error
/// (missing root, unreadable file) is returned as `Err`.
pub fn check(root: &Path) -> std::io::Result<Vec<Finding>> {
    let meta = std::fs::metadata(root)?;
    if !meta.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            format!("{} is not a directory", root.display()),
        ));
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_content_tree_passes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        assert_eq!(check(&root).unwrap(), Vec::new());
    }

    #[test]
    fn missing_root_is_an_error() {
        assert!(check(Path::new("does/not/exist")).is_err());
    }
}
