//! Taful content (spec §12): lesson, quiz and skill-tree loader, the
//! placeholder expression language, the Markdown renderer and the content
//! checks. The checks run in CI through `tools/content-check` and the app
//! loads the same content embedded in its binary.

pub mod check;
pub mod expr;
pub mod format;
pub mod lesson;
pub mod library;
pub mod quiz;

use std::fmt;
use std::path::{Path, PathBuf};

pub use library::{Files, LOCALES, Library, SOURCE_LOCALE, SkillTree, Track};

/// One problem found by a content check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// File the finding refers to, relative to the content root.
    pub path: PathBuf,
    /// Human-readable description (en-US; checks are a developer tool).
    pub message: String,
}

impl Finding {
    pub fn new(path: &str, message: impl Into<String>) -> Finding {
        Finding {
            path: PathBuf::from(path),
            message: message.into(),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

/// The `i18n/` folder next to a content root (`<root>/../i18n`).
pub fn i18n_root_for(content_root: &Path) -> PathBuf {
    content_root
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .join("i18n")
}

/// Runs every content check over the `content/` tree at `root`, with the
/// Fluent files from the sibling `i18n/` folder.
///
/// Returns the findings; an empty list means the tree is valid. An IO error
/// (missing root, unreadable file) is returned as `Err`.
pub fn check(root: &Path) -> std::io::Result<Vec<Finding>> {
    let content = Files::from_dir(root)?;
    let i18n = Files::from_dir(&i18n_root_for(root))?;
    let (lib, mut findings) = Library::load(&content);
    findings.extend(check::check_library(&lib, &content, &i18n));
    Ok(findings)
}

/// Rewrites every snapshot under `root` to the current rendered values and
/// removes stale ones. Returns the paths written or removed.
pub fn bless(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let content = Files::from_dir(root)?;
    let (lib, _) = Library::load(&content);
    let expected = check::expected_snapshots(&lib);
    let mut touched = Vec::new();
    for (path, text) in &expected {
        if content.get(path) != Some(text.as_str()) {
            let full = root.join(path);
            if let Some(dir) = full.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&full, text)?;
            touched.push(full);
        }
    }
    for path in content.0.keys() {
        let is_snapshot = (path.starts_with("lessons/.snapshots/")
            || path.starts_with("quizzes/.snapshots/"))
            && path.ends_with(".toml");
        if is_snapshot && !expected.contains_key(path) {
            let full = root.join(path);
            std::fs::remove_file(&full)?;
            touched.push(full);
        }
    }
    Ok(touched)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_content_tree_passes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let findings = check(&root).unwrap();
        assert!(
            findings.is_empty(),
            "{}",
            findings
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn missing_root_is_an_error() {
        assert!(check(Path::new("does/not/exist")).is_err());
    }

    fn lesson(id: &str, prereq: &[&str], extra: &str, body: &str) -> String {
        format!(
            "+++\nid = \"{id}\"\nversion = 1\ntitle = \"T\"\nprerequisites = {prereq:?}\n{extra}\n+++\n{body}\n"
        )
    }

    fn quiz(id: &str, lesson: &str) -> String {
        format!(
            "id = \"{id}\"\nversion = 1\nlesson = \"{lesson}\"\n\n[[questions]]\nid = \"q\"\nkind = \"choice\"\n\
             prompt = {{ en-US = \"?\", pt-BR = \"?\" }}\nchoices = [{{ en-US = \"a\", pt-BR = \"a\" }}, {{ en-US = \"b\", pt-BR = \"b\" }}]\nanswer = 0\n"
        )
    }

    fn run(content: &[(&str, String)], i18n: &[(&str, &str)]) -> Vec<String> {
        let content = Files(
            content
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        );
        let i18n = Files::from_pairs(i18n.iter().copied());
        let (lib, mut f) = Library::load(&content);
        f.extend(check::check_library(&lib, &content, &i18n));
        f.into_iter().map(|f| f.to_string()).collect()
    }

    const I18N: &[(&str, &str)] = &[
        ("en-US/lessons.ftl", "track-t = T\n"),
        ("pt-BR/lessons.ftl", "track-t = T\n"),
        (
            "en-US/glossary.ftl",
            "glossary-equity = Equity\nglossary-equity-def = Share\n",
        ),
        (
            "pt-BR/glossary.ftl",
            "glossary-equity = Equity\nglossary-equity-def = Parte\n",
        ),
    ];

    fn valid() -> Vec<(&'static str, String)> {
        let a = lesson(
            "a",
            &[],
            "unlock = { quiz = \"a-quiz\" }\nglossary = [\"equity\"]",
            "Hi [equity](term:equity).",
        );
        let b = lesson(
            "b",
            &["a"],
            "unlock = { quiz = \"b-quiz\" }",
            "Need {{ calc: pot_odds(pot=100, bet=50) | pct(1) }}.",
        );
        let b_pt = lesson(
            "b",
            &["a"],
            "unlock = { quiz = \"b-quiz\" }",
            "Precisa {{ calc: pot_odds(pot=100, bet=50) | pct(1) }}.",
        );
        vec![
            (
                "skill-tree.toml",
                "[[tracks]]\nid = \"t\"\nlessons = [\"a\", \"b\"]\n".into(),
            ),
            ("lessons/en-US/a.md", a.clone()),
            ("lessons/pt-BR/a.md", a),
            ("lessons/en-US/b.md", b),
            ("lessons/pt-BR/b.md", b_pt),
            ("quizzes/a-quiz.toml", quiz("a-quiz", "a")),
            ("quizzes/b-quiz.toml", quiz("b-quiz", "b")),
        ]
    }

    fn with_snapshots(mut c: Vec<(&'static str, String)>) -> Vec<(&'static str, String)> {
        let files = Files(c.iter().map(|(k, v)| (k.to_string(), v.clone())).collect());
        let (lib, _) = Library::load(&files);
        for (path, text) in check::expected_snapshots(&lib) {
            c.push((Box::leak(path.into_boxed_str()), text));
        }
        c
    }

    #[test]
    fn valid_tree_passes_and_needs_snapshots() {
        let f = run(&valid(), I18N);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("snapshot missing"));
        assert_eq!(run(&with_snapshots(valid()), I18N), Vec::<String>::new());
    }

    #[test]
    fn drift_is_caught() {
        let mut c = with_snapshots(valid());
        for (k, v) in &mut c {
            if k.ends_with(".snapshots/b.toml") {
                *v = v.replace("25,0%", "26,0%");
            }
        }
        let f = run(&c, I18N);
        assert!(f.iter().any(|m| m.contains("drifted")), "{f:?}");
    }

    #[test]
    fn rule_violations_are_caught() {
        type Mutation = Box<dyn Fn(&mut Vec<(&'static str, String)>)>;
        let cases: Vec<(Mutation, &str)> = vec![
            (
                Box::new(|c| c[4].1 = c[4].1.replace("pct(1)", "pct(2)")),
                "placeholder set differs",
            ),
            (
                Box::new(|c| c[4].1 = c[4].1.replace("version = 1", "version = 2")),
                "id/version",
            ),
            (
                Box::new(|c| c[3].1 = c[3].1.replace("pot_odds(", "nope(")),
                "unknown function",
            ),
            (
                Box::new(|c| {
                    c[1].1 = c[1]
                        .1
                        .replace("prerequisites = []", "prerequisites = [\"b\"]")
                }),
                "cycle",
            ),
            (
                Box::new(|c| {
                    c[1].1 = c[1]
                        .1
                        .replace("prerequisites = []", "prerequisites = [\"zzz\"]")
                }),
                "missing prerequisite",
            ),
            (
                Box::new(|c| c[1].1 = c[1].1.replace("unlock = { quiz = \"a-quiz\" }", "")),
                "no unlock",
            ),
            (
                Box::new(|c| c[0].1 = "[[tracks]]\nid = \"t\"\nlessons = [\"b\", \"a\"]\n".into()),
                "comes later",
            ),
            (
                Box::new(|c| c[0].1 = "[[tracks]]\nid = \"t\"\nlessons = [\"a\"]\n".into()),
                "not in skill-tree",
            ),
            (
                Box::new(|c| c[0].1 = "[[tracks]]\nid = \"x\"\nlessons = [\"a\", \"b\"]\n".into()),
                "track-x",
            ),
            (
                Box::new(|c| {
                    c.remove(4);
                }),
                "missing in this locale",
            ),
            (
                Box::new(|c| {
                    c[1].1 = c[1].1.replace(
                        "glossary = [\"equity\"]",
                        "glossary = [\"equity\", \"spr\"]",
                    )
                }),
                "glossary-spr",
            ),
            (
                Box::new(|c| c[1].1 = c[1].1.replace("glossary = [\"equity\"]", "glossary = []")),
                "not listed in `glossary`",
            ),
            (
                Box::new(|c| c[5].1 = quiz("a-quiz", "b")),
                "belongs to lesson",
            ),
            (
                Box::new(|c| c[5].1 = c[5].1.replace(", pt-BR = \"?\"", "")),
                "missing `pt-BR`",
            ),
        ];
        for (mutate, expect) in cases {
            let mut c = with_snapshots(valid());
            mutate(&mut c);
            let f = run(&c, I18N);
            assert!(
                f.iter().any(|m| m.contains(expect)),
                "expected `{expect}` in {f:#?}"
            );
        }
    }

    #[test]
    fn bot_profiles_are_checked() {
        let profile = |id: &str, vpip: &str| {
            format!(
                "id = \"{id}\"\n[preflop]\nvpip_mult = {vpip}\npfr_mult = 1.0\nthreebet_mult = 1.0\n\
                 [postflop]\nfold_to_cbet = 0.0\nfold_to_raise = 0.0\nbluff_value_shift = 0.0\naggression = 0.0\n\
                 [sizing]\nprefer = \"reference\"\n"
            )
        };
        let names = "bot-profile-nit = Nit\nbot-profile-nit-about = Tight\n";
        let mut i18n = I18N.to_vec();
        i18n.push(("en-US/table.ftl", names));
        i18n.push(("pt-BR/table.ftl", names));
        let with = |path: &'static str, text: String| {
            let mut c = with_snapshots(valid());
            c.push((path, text));
            c
        };
        assert_eq!(
            run(&with("bots/nit.toml", profile("nit", "0.6")), &i18n),
            Vec::<String>::new()
        );
        let cases = [
            (with("bots/nit.toml", profile("nit", "0.0")), "vpip_mult"),
            (
                with("bots/nit.toml", profile("tag", "0.6")),
                "does not match the file name",
            ),
            (
                with("bots/tag.toml", profile("tag", "0.6")),
                "no `bot-profile-tag` message in i18n/en-US",
            ),
        ];
        for (content, expect) in cases {
            let f = run(&content, &i18n);
            assert!(
                f.iter().any(|m| m.contains(expect)),
                "expected `{expect}` in {f:#?}"
            );
        }
    }
}
