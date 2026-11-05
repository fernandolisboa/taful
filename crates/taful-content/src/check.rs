//! Content checks (spec §12.3). `content-check` fails the build when:
//!
//! 1. any placeholder errors in any locale;
//! 2. a rendered value differs from its pinned snapshot (drift); snapshots
//!    are rewritten with `content-check --bless`;
//! 3. locales disagree on id, version, front matter or placeholder set;
//! 4. the skill tree has a cycle, a missing prerequisite, a prerequisite that
//!    comes later in the tree, or a lesson with no unlock;
//! 5. a glossary term is undefined in `i18n/*/glossary.ftl`.
//!
//! Plus the structural checks those rules need: every lesson is in the tree
//! once and exists in every locale, every quiz belongs to the lesson that
//! unlocks with it, and every track has a Fluent title.

use std::collections::{BTreeMap, BTreeSet};

use crate::Finding;
use crate::expr;
use crate::lesson::Unlock;
use crate::library::{Files, LOCALES, Library, SOURCE_LOCALE};

/// Snapshot file of a lesson.
pub fn lesson_snapshot_path(id: &str) -> String {
    format!("lessons/.snapshots/{id}.toml")
}

/// Snapshot file of a quiz.
pub fn quiz_snapshot_path(id: &str) -> String {
    format!("quizzes/.snapshots/{id}.toml")
}

/// Locale → placeholder source → rendered text.
type Snapshot = BTreeMap<String, BTreeMap<String, String>>;

const SNAPSHOT_HEADER: &str = "# Rendered placeholder values, pinned (spec §12.3). Regenerate with\n\
     # `cargo run -p taful-content-check -- --bless`; review the diff.\n\n";

fn snapshot_text(s: &Snapshot) -> String {
    format!(
        "{SNAPSHOT_HEADER}{}",
        toml::to_string(s).expect("string maps serialize")
    )
}

/// Every snapshot file the content should have, by path, with its text.
pub fn expected_snapshots(lib: &Library) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let render_all = |sources: Vec<(String, Vec<String>)>| -> Snapshot {
        let mut snap = Snapshot::new();
        for (locale, srcs) in sources {
            let entry = snap.entry(locale.clone()).or_default();
            for src in srcs {
                let text = expr::render(&src, &locale, None)
                    .map(|r| r.text)
                    .unwrap_or_else(|e| format!("ERROR: {e}"));
                entry.insert(src, text);
            }
        }
        snap
    };
    for (id, by_locale) in &lib.lessons {
        let sources: Vec<(String, Vec<String>)> = by_locale
            .iter()
            .map(|(loc, l)| {
                let srcs = l
                    .placeholders()
                    .map(|ps| ps.into_iter().map(|p| p.source).collect())
                    .unwrap_or_default();
                (loc.clone(), srcs)
            })
            .collect();
        let snap = render_all(sources);
        if snap.values().any(|m| !m.is_empty()) {
            out.insert(lesson_snapshot_path(id), snapshot_text(&snap));
        }
    }
    for (id, quiz) in &lib.quizzes {
        let sources: Vec<(String, Vec<String>)> = LOCALES
            .iter()
            .map(|loc| {
                let srcs = quiz
                    .placeholders(loc)
                    .map(|ps| ps.into_iter().map(|p| p.source).collect())
                    .unwrap_or_default();
                (loc.to_string(), srcs)
            })
            .collect();
        let snap = render_all(sources);
        if snap.values().any(|m| !m.is_empty()) {
            out.insert(quiz_snapshot_path(id), snapshot_text(&snap));
        }
    }
    out
}

/// Fluent message ids defined per locale, from `i18n/<locale>/*.ftl`, and
/// per locale and file.
fn fluent_ids(i18n: &Files) -> (BTreeMap<String, BTreeSet<String>>, Vec<Finding>) {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut findings = Vec::new();
    for (path, text) in &i18n.0 {
        let Some((locale, file)) = path.split_once('/') else {
            continue;
        };
        if !file.ends_with(".ftl") {
            continue;
        }
        let res = match fluent_syntax::parser::parse(text.as_str()) {
            Ok(r) => r,
            Err((r, errs)) => {
                findings.push(Finding::new(
                    &format!("../i18n/{path}"),
                    format!("{} Fluent syntax error(s)", errs.len()),
                ));
                r
            }
        };
        let ids = out.entry(locale.to_owned()).or_default();
        for entry in res.body {
            if let fluent_syntax::ast::Entry::Message(m) = entry {
                ids.insert(format!("{file}#{}", m.id.name));
                ids.insert(m.id.name.to_owned());
            }
        }
    }
    (out, findings)
}

/// Runs every check over loaded content. `content` is the raw tree (for the
/// snapshot files), `i18n` the `i18n/` tree.
pub fn check_library(lib: &Library, content: &Files, i18n: &Files) -> Vec<Finding> {
    let mut f = Vec::new();
    let (ids, fluent_findings) = fluent_ids(i18n);
    f.extend(fluent_findings);
    let has = |locale: &str, id: &str| ids.get(locale).is_some_and(|s| s.contains(id));
    let has_glossary = |locale: &str, id: &str| {
        ids.get(locale)
            .is_some_and(|s| s.contains(&format!("glossary.ftl#{id}")))
    };

    // ---- 3. locale parity --------------------------------------------------------------
    for (id, by_locale) in &lib.lessons {
        for loc in LOCALES {
            if !by_locale.contains_key(loc) {
                f.push(Finding::new(
                    &format!("lessons/{loc}/{id}.md"),
                    "missing in this locale",
                ));
            }
        }
        let Some(src) = by_locale.get(SOURCE_LOCALE) else {
            continue;
        };
        let src_ph = sorted_sources(src.placeholders().ok());
        for (loc, l) in by_locale {
            let path = format!("lessons/{loc}/{id}.md");
            if loc == SOURCE_LOCALE {
                continue;
            }
            let (a, b) = (&src.meta, &l.meta);
            if a.id != b.id || a.version != b.version {
                f.push(Finding::new(
                    &path,
                    format!(
                        "id/version `{}@{}` differ from {SOURCE_LOCALE} `{}@{}`",
                        b.id, b.version, a.id, a.version
                    ),
                ));
            }
            if a.prerequisites != b.prerequisites
                || a.tags != b.tags
                || a.glossary != b.glossary
                || a.unlock != b.unlock
            {
                f.push(Finding::new(
                    &path,
                    format!("front matter (other than title) differs from {SOURCE_LOCALE}"),
                ));
            }
            if sorted_sources(l.placeholders().ok()) != src_ph {
                f.push(Finding::new(
                    &path,
                    format!("placeholder set differs from {SOURCE_LOCALE}"),
                ));
            }
        }
    }
    for (id, quiz) in &lib.quizzes {
        let path = format!("quizzes/{id}.toml");
        let mut qids = BTreeSet::new();
        for q in &quiz.questions {
            if !qids.insert(q.id.as_str()) {
                f.push(Finding::new(
                    &path,
                    format!("question id `{}` repeated", q.id),
                ));
            }
        }
        if quiz.questions.is_empty() {
            f.push(Finding::new(&path, "no questions"));
        }
        for (at, s) in quiz.strings() {
            for loc in LOCALES {
                if !s.contains_key(loc) {
                    f.push(Finding::new(&path, format!("{at}: missing `{loc}`")));
                }
            }
            for loc in s.keys() {
                if !LOCALES.contains(&loc.as_str()) {
                    f.push(Finding::new(&path, format!("{at}: unknown locale `{loc}`")));
                }
            }
            let srcs = |loc: &str| {
                sorted_sources(s.get(loc).and_then(|t| expr::find_placeholders(t).ok()))
            };
            let base = srcs(SOURCE_LOCALE);
            for loc in LOCALES {
                if srcs(loc) != base {
                    f.push(Finding::new(
                        &path,
                        format!("{at}: placeholder set in `{loc}` differs from {SOURCE_LOCALE}"),
                    ));
                }
            }
        }
    }

    // ---- 1. every placeholder renders ---------------------------------------------------
    for (id, by_locale) in &lib.lessons {
        for (loc, l) in by_locale {
            if let Err(errs) = l.render(None) {
                for e in errs {
                    f.push(Finding::new(&format!("lessons/{loc}/{id}.md"), e));
                }
            }
        }
    }
    for (id, quiz) in &lib.quizzes {
        for q in &quiz.questions {
            for loc in LOCALES {
                if let Err(errs) = quiz.render_question(q, loc, None) {
                    for e in errs {
                        f.push(Finding::new(
                            &format!("quizzes/{id}.toml"),
                            format!("[{loc}] {e}"),
                        ));
                    }
                }
            }
        }
    }

    // ---- 2. snapshots ------------------------------------------------------------------------
    let expected = expected_snapshots(lib);
    for (path, text) in &expected {
        match content.get(path) {
            None => f.push(Finding::new(
                path,
                "snapshot missing; run `content-check --bless`",
            )),
            Some(have) if normalize_newlines(have) != normalize_newlines(text) => {
                f.push(Finding::new(
                    path,
                    "rendered values drifted from the snapshot; review and run `content-check --bless`",
                ))
            }
            Some(_) => {}
        }
    }
    for path in content.0.keys() {
        let is_snapshot = (path.starts_with("lessons/.snapshots/")
            || path.starts_with("quizzes/.snapshots/"))
            && path.ends_with(".toml");
        if is_snapshot && !expected.contains_key(path) {
            f.push(Finding::new(
                path,
                "stale snapshot (no such lesson/quiz or no placeholders); run `content-check --bless`",
            ));
        }
    }

    // ---- 4. skill tree -----------------------------------------------------------------------
    let order = lib.tree.order();
    let mut position = BTreeMap::new();
    for (i, id) in order.iter().enumerate() {
        if position.insert(*id, i).is_some() {
            f.push(Finding::new(
                "skill-tree.toml",
                format!("lesson `{id}` listed twice"),
            ));
        }
        if !lib.lessons.contains_key(*id) {
            f.push(Finding::new(
                "skill-tree.toml",
                format!("lesson `{id}` has no file"),
            ));
        }
    }
    let mut track_ids = BTreeSet::new();
    for t in &lib.tree.tracks {
        if !track_ids.insert(t.id.as_str()) {
            f.push(Finding::new(
                "skill-tree.toml",
                format!("track `{}` repeated", t.id),
            ));
        }
        for loc in LOCALES {
            if !has(loc, &format!("track-{}", t.id)) {
                f.push(Finding::new(
                    "skill-tree.toml",
                    format!(
                        "track `{}` has no `track-{}` message in i18n/{loc}",
                        t.id, t.id
                    ),
                ));
            }
        }
    }
    for (id, by_locale) in &lib.lessons {
        let Some(l) = by_locale.get(SOURCE_LOCALE) else {
            continue;
        };
        let path = format!("lessons/{SOURCE_LOCALE}/{id}.md");
        let Some(&pos) = position.get(id.as_str()) else {
            f.push(Finding::new(&path, "not in skill-tree.toml"));
            continue;
        };
        for p in &l.meta.prerequisites {
            match position.get(p.as_str()) {
                None if !lib.lessons.contains_key(p) => {
                    f.push(Finding::new(&path, format!("missing prerequisite `{p}`")))
                }
                Some(&pp) if pp >= pos => f.push(Finding::new(
                    &path,
                    format!("prerequisite `{p}` comes later in the skill tree"),
                )),
                _ => {}
            }
        }
        match &l.meta.unlock {
            None => f.push(Finding::new(&path, "no unlock (quiz or drill)")),
            Some(Unlock::Quiz { quiz }) => match lib.quizzes.get(quiz) {
                None => f.push(Finding::new(
                    &path,
                    format!("unlock quiz `{quiz}` not found"),
                )),
                Some(q) if q.lesson != *id => f.push(Finding::new(
                    &path,
                    format!("quiz `{quiz}` belongs to lesson `{}`", q.lesson),
                )),
                Some(_) => {}
            },
            Some(Unlock::Drill { count, .. }) => {
                // Spot-template ids are validated once the strategy library
                // exists (M3/M4); the shape is checked here.
                if *count == 0 {
                    f.push(Finding::new(&path, "drill unlock needs count > 0"));
                }
            }
        }
    }
    if let Some(cycle) = find_cycle(lib) {
        f.push(Finding::new(
            "skill-tree.toml",
            format!("prerequisite cycle: {}", cycle.join(" → ")),
        ));
    }
    for (id, quiz) in &lib.quizzes {
        let unlocked_by = lib
            .lesson(&quiz.lesson, SOURCE_LOCALE)
            .and_then(|l| l.meta.unlock.as_ref());
        if !matches!(unlocked_by, Some(Unlock::Quiz { quiz: q }) if q == id) {
            f.push(Finding::new(
                &format!("quizzes/{id}.toml"),
                format!("lesson `{}` does not unlock with this quiz", quiz.lesson),
            ));
        }
    }

    // ---- 5. glossary -------------------------------------------------------------------------
    for (id, by_locale) in &lib.lessons {
        for (loc, l) in by_locale {
            let path = format!("lessons/{loc}/{id}.md");
            for t in l.linked_terms() {
                if !l.meta.glossary.contains(&t) {
                    f.push(Finding::new(
                        &path,
                        format!("links term `{t}` not listed in `glossary`"),
                    ));
                }
            }
        }
        if let Some(l) = by_locale.get(SOURCE_LOCALE) {
            for t in &l.meta.glossary {
                for loc in LOCALES {
                    for key in [format!("glossary-{t}"), format!("glossary-{t}-def")] {
                        if !has_glossary(loc, &key) {
                            f.push(Finding::new(
                                &format!("lessons/{SOURCE_LOCALE}/{id}.md"),
                                format!("glossary term `{t}`: `{key}` missing in i18n/{loc}/glossary.ftl"),
                            ));
                        }
                    }
                }
            }
        }
    }

    f
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn sorted_sources(ps: Option<Vec<expr::Placeholder>>) -> Vec<String> {
    let mut v: Vec<String> = ps
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.source)
        .collect();
    v.sort();
    v
}

/// A prerequisite cycle, if any (DFS over the source-locale front matter).
fn find_cycle(lib: &Library) -> Option<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn visit(
        lib: &Library,
        id: &str,
        marks: &mut BTreeMap<String, Mark>,
        stack: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        match marks.get(id) {
            Some(Mark::Done) => return None,
            Some(Mark::Visiting) => {
                let start = stack.iter().position(|s| s == id).unwrap_or(0);
                let mut cycle = stack[start..].to_vec();
                cycle.push(id.to_owned());
                return Some(cycle);
            }
            None => {}
        }
        marks.insert(id.to_owned(), Mark::Visiting);
        stack.push(id.to_owned());
        if let Some(l) = lib.lesson(id, SOURCE_LOCALE) {
            for p in &l.meta.prerequisites {
                if let Some(c) = visit(lib, p, marks, stack) {
                    return Some(c);
                }
            }
        }
        stack.pop();
        marks.insert(id.to_owned(), Mark::Done);
        None
    }
    let mut marks = BTreeMap::new();
    for id in lib.lessons.keys() {
        if let Some(c) = visit(lib, id, &mut marks, &mut Vec::new()) {
            return Some(c);
        }
    }
    None
}
