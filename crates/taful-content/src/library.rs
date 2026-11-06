//! Loading the content tree: skill tree, lessons per locale, quizzes.
//!
//! The loader works over [`Files`], an in-memory map of relative paths to
//! text, so the app can load content embedded in the binary and the checks
//! can load it from disk with the same code.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use taful_core::profile::Profile;

use crate::Finding;
use crate::lesson::Lesson;
use crate::quiz::Quiz;

/// Locales content is written in; the first is the source.
pub const LOCALES: [&str; 2] = ["en-US", "pt-BR"];
/// Where content is authored (§12.1).
pub const SOURCE_LOCALE: &str = "en-US";

/// Text files by path relative to their root, `/`-separated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Files(pub BTreeMap<String, String>);

impl Files {
    /// Reads every UTF-8 file under `root` (recursively).
    pub fn from_dir(root: &Path) -> io::Result<Files> {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> io::Result<()> {
            let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let path = e.path();
                if path.is_dir() {
                    walk(root, &path, out)?;
                } else {
                    let rel = path
                        .strip_prefix(root)
                        .expect("under root")
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/");
                    match std::fs::read_to_string(&path) {
                        Ok(text) => {
                            out.insert(rel, text);
                        }
                        // Binary files (images later) are not content text.
                        Err(e) if e.kind() == io::ErrorKind::InvalidData => {}
                        Err(e) => return Err(e),
                    }
                }
            }
            Ok(())
        }
        let meta = std::fs::metadata(root)?;
        if !meta.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("{} is not a directory", root.display()),
            ));
        }
        let mut out = BTreeMap::new();
        walk(root, root, &mut out)?;
        Ok(Files(out))
    }

    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Files {
        Files(
            pairs
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
        )
    }

    pub fn get(&self, path: &str) -> Option<&str> {
        self.0.get(path).map(String::as_str)
    }

    /// Files directly under `dir` (not in subfolders) with `ext`, as
    /// `(file stem, text)`.
    pub fn in_dir<'a>(
        &'a self,
        dir: &'a str,
        ext: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.0.iter().filter_map(move |(path, text)| {
            let rest = path.strip_prefix(dir)?.strip_prefix('/')?;
            if rest.contains('/') {
                return None;
            }
            Some((rest.strip_suffix(ext)?, text.as_str()))
        })
    }
}

/// `content/skill-tree.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillTree {
    pub tracks: Vec<Track>,
}

/// A group of lessons in order. Its title is the Fluent message `track-<id>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub id: String,
    pub lessons: Vec<String>,
}

impl SkillTree {
    /// Every lesson id in tree order.
    pub fn order(&self) -> Vec<&str> {
        self.tracks
            .iter()
            .flat_map(|t| t.lessons.iter().map(String::as_str))
            .collect()
    }
}

/// The loaded content.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Library {
    pub tree: SkillTree,
    /// Lesson id → locale → lesson.
    pub lessons: BTreeMap<String, BTreeMap<String, Lesson>>,
    /// Quiz id → quiz.
    pub quizzes: BTreeMap<String, Quiz>,
    /// Bot profile id → profile (`bots/<id>.toml`, spec §9).
    pub profiles: BTreeMap<String, Profile>,
}

impl Library {
    /// Parses everything it can; files that fail to parse become findings.
    pub fn load(files: &Files) -> (Library, Vec<Finding>) {
        let mut findings = Vec::new();
        let mut lib = Library::default();
        match files.get("skill-tree.toml") {
            Some(text) => match toml::from_str::<SkillTree>(text) {
                Ok(t) => lib.tree = t,
                Err(e) => findings.push(Finding::new("skill-tree.toml", e.to_string())),
            },
            None => findings.push(Finding::new("skill-tree.toml", "missing")),
        }
        for locale in LOCALES {
            let dir = format!("lessons/{locale}");
            for (stem, text) in files.in_dir(&dir, ".md") {
                let path = format!("{dir}/{stem}.md");
                match Lesson::parse(locale, text) {
                    Ok(l) => {
                        if l.meta.id != stem {
                            findings.push(Finding::new(
                                &path,
                                format!("id `{}` does not match the file name", l.meta.id),
                            ));
                        }
                        lib.lessons
                            .entry(stem.to_owned())
                            .or_default()
                            .insert(locale.to_owned(), l);
                    }
                    Err(e) => findings.push(Finding::new(&path, e)),
                }
            }
        }
        for (stem, text) in files.in_dir("quizzes", ".toml") {
            let path = format!("quizzes/{stem}.toml");
            match Quiz::parse(text) {
                Ok(q) => {
                    if q.id != stem {
                        findings.push(Finding::new(
                            &path,
                            format!("id `{}` does not match the file name", q.id),
                        ));
                    }
                    lib.quizzes.insert(stem.to_owned(), q);
                }
                Err(e) => findings.push(Finding::new(&path, e)),
            }
        }
        for (stem, text) in files.in_dir("bots", ".toml") {
            let path = format!("bots/{stem}.toml");
            match Profile::from_toml(text) {
                Ok(p) => {
                    if p.id != stem {
                        findings.push(Finding::new(
                            &path,
                            format!("id `{}` does not match the file name", p.id),
                        ));
                    }
                    lib.profiles.insert(stem.to_owned(), p);
                }
                Err(e) => findings.push(Finding::new(&path, e)),
            }
        }
        (lib, findings)
    }

    /// The lesson in `locale`, falling back to the source locale.
    pub fn lesson(&self, id: &str, locale: &str) -> Option<&Lesson> {
        let by_locale = self.lessons.get(id)?;
        by_locale
            .get(locale)
            .or_else(|| by_locale.get(SOURCE_LOCALE))
    }

    /// The quiz a lesson's unlock names, if any.
    pub fn quiz_for(&self, lesson_id: &str) -> Option<&Quiz> {
        match &self.lesson(lesson_id, SOURCE_LOCALE)?.meta.unlock {
            Some(crate::lesson::Unlock::Quiz { quiz }) => self.quizzes.get(quiz),
            _ => None,
        }
    }
}
