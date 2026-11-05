//! Source-level checks over `apps/desktop/src` (spec §15.4, §15.5, §17):
//!
//! - No color literal outside `theme.rs`: every color comes from a token.
//! - Every Fluent id the UI names exists in the source locale.

use std::fs;
use std::path::{Path, PathBuf};

use taful_app::i18n::{Locale, Localizer};

/// The one file allowed to build colors.
const THEME_MODULE: &str = "theme.rs";

/// Call prefixes that build a color in GPUI, plus its named-color helpers.
const COLOR_CALLS: &[&str] = &[
    "rgb(",
    "rgba(",
    "hsla(",
    "hsl(",
    "Rgba {",
    "Hsla {",
    "black()",
    "white()",
    "red()",
    "green()",
    "blue()",
    "yellow()",
    "opaque_grey(",
    "transparent_black()",
    "transparent_white()",
];

fn sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut out);
    assert!(!out.is_empty());
    out
}

/// Strips `//` comments so prose may mention `rgb(` or `#fff`. A `//` inside
/// a string literal (a URL) is kept.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(i, line)| (i + 1, strip_comment(line)))
}

fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let (mut in_str, mut escaped) = (false, false);
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            _ if escaped => escaped = false,
            b'\\' if in_str => escaped = true,
            b'"' => in_str = !in_str,
            b'/' if !in_str && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Finds `#rgb`, `#rrggbb` or `#rrggbbaa` and `0xrrggbb` literals.
fn hex_color(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    for (i, _) in line.match_indices(['#', 'x']) {
        let prefix_ok = bytes[i] == b'#' || (i > 0 && bytes[i - 1] == b'0');
        if !prefix_ok {
            continue;
        }
        let digits = line[i + 1..]
            .bytes()
            .take_while(|b| b.is_ascii_hexdigit() || *b == b'_')
            .filter(|b| *b != b'_')
            .count();
        let end = line[i + 1..]
            .bytes()
            .position(|b| !(b.is_ascii_hexdigit() || b == b'_'))
            .map_or(line.len(), |p| i + 1 + p);
        let hash_len_ok = bytes[i] == b'#' && matches!(digits, 3 | 6 | 8);
        let hex_len_ok = bytes[i] == b'x' && matches!(digits, 6 | 8);
        if hash_len_ok || hex_len_ok {
            // `0x`: the `0` before `x` is ASCII, so `i - 1` is a char boundary.
            let start = if bytes[i] == b'x' { i - 1 } else { i };
            return Some(&line[start..end]);
        }
    }
    None
}

#[test]
fn no_color_literals_outside_theme_module() {
    let mut hits = Vec::new();
    for path in sources() {
        if path.file_name().is_some_and(|n| n == THEME_MODULE) {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        for (n, line) in code_lines(&text) {
            let found = COLOR_CALLS
                .iter()
                .find(|c| line.contains(*c))
                .map(|c| c.to_string())
                .or_else(|| hex_color(line).map(str::to_owned));
            if let Some(what) = found {
                hits.push(format!("{}:{n}: `{what}`", path.display()));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "color literals outside {THEME_MODULE} (use a theme token):\n{}",
        hits.join("\n")
    );
}

#[test]
fn hex_detector() {
    assert!(hex_color(r##"bg("#0f5257")"##).is_some());
    assert!(hex_color("let c = 0x0f5257;").is_some());
    assert!(hex_color("let c = 0x0f_52_57;").is_some());
    assert!(hex_color(r##"bg("#fff")"##).is_some());
    assert!(hex_color("#[derive(Debug)]").is_none());
    assert!(hex_color("let n = 0x10;").is_none());
    assert!(hex_color("let s = \"#1 seat\";").is_none());
    assert_eq!(hex_color("\"Mão#abc\""), Some("#abc"));
}

#[test]
fn comment_stripping_keeps_urls() {
    let line = r#".child(link("https://taful.filx.app", tr("nav-x"))) // note"#;
    assert_eq!(
        strip_comment(line),
        r#".child(link("https://taful.filx.app", tr("nav-x"))) "#
    );
    assert_eq!(
        strip_comment(r#"let s = "a\"//b"; // c"#),
        r#"let s = "a\"//b"; "#
    );
}

/// String literals that look like Fluent ids (`lower-case-with-dashes`).
fn fluent_like_literals(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (n, line) in code_lines(text) {
        let mut parts = line.split('"');
        parts.next();
        while let (Some(lit), rest) = (parts.next(), parts.next()) {
            let looks_like_id = lit.contains('-')
                && lit.starts_with(|c: char| c.is_ascii_lowercase())
                && lit
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if looks_like_id {
                out.push((n, lit.to_owned()));
            }
            if rest.is_none() {
                break;
            }
        }
    }
    out
}

#[test]
fn every_fluent_id_in_the_ui_exists() {
    let i18n = Localizer::new(Locale::SOURCE).unwrap();
    let mut missing = Vec::new();
    let mut seen = 0;
    for path in sources() {
        let text = fs::read_to_string(&path).unwrap();
        for (n, id) in fluent_like_literals(&text) {
            seen += 1;
            if !i18n.has(&id) {
                missing.push(format!("{}:{n}: `{id}`", path.display()));
            }
        }
    }
    assert!(seen > 0, "the scan found no Fluent ids at all");
    assert!(
        missing.is_empty(),
        "ids not in i18n/{}:\n{}",
        Locale::SOURCE.tag(),
        missing.join("\n")
    );
}
