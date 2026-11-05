//! Lessons (spec §12.1): Markdown with TOML front matter between `+++`
//! lines, rendered to a small UI-free block model.
//!
//! Glossary terms are Markdown links with the `term:` scheme:
//! `[pot odds](term:pot-odds)`. The renderer turns them into [`Span`]s that
//! carry the term id; the UI shows the definition as a tooltip. Any other
//! link renders as its text (lessons do not link out).

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use crate::expr::{self, ExprError, Placeholder, SpotProvider};

/// What completes a lesson and unlocks the next ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Unlock {
    /// Pass the quiz with this id.
    Quiz { quiz: String },
    /// Get `count` decisions of a spot template at `min_grade` or better
    /// (spot templates arrive with the strategy library).
    Drill {
        drill: String,
        min_grade: String,
        count: u32,
    },
}

/// Front matter. Every locale's file must agree on everything but `title`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontMatter {
    pub id: String,
    pub version: u32,
    pub title: String,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub glossary: Vec<String>,
    pub unlock: Option<Unlock>,
    /// Reading time shown in the skill tree, in minutes.
    #[serde(default)]
    pub minutes: Option<u32>,
}

/// One lesson file in one locale.
#[derive(Debug, Clone, PartialEq)]
pub struct Lesson {
    pub locale: String,
    pub meta: FrontMatter,
    /// Markdown body with placeholders unexpanded.
    pub body: String,
}

impl Lesson {
    /// Splits and parses a lesson file.
    pub fn parse(locale: &str, text: &str) -> Result<Lesson, String> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let rest = text
            .strip_prefix("+++\n")
            .or_else(|| text.strip_prefix("+++\r\n"))
            .ok_or("missing `+++` front matter at the top")?;
        let end = rest
            .find("\n+++")
            .ok_or("unterminated front matter (no closing `+++`)")?;
        let meta: FrontMatter =
            toml::from_str(&rest[..end]).map_err(|e| format!("front matter: {e}"))?;
        let body = rest[end + 4..].trim_start_matches(['\r', '\n']).to_owned();
        Ok(Lesson {
            locale: locale.to_owned(),
            meta,
            body,
        })
    }

    /// Placeholders in the body.
    pub fn placeholders(&self) -> Result<Vec<Placeholder>, ExprError> {
        expr::find_placeholders(&self.body)
    }

    /// Renders the body: placeholders expanded, Markdown parsed.
    pub fn render(&self, spots: Option<&dyn SpotProvider>) -> Result<Document, Vec<String>> {
        let (text, errors) = expr::substitute(&self.body, &self.locale, spots);
        if !errors.is_empty() {
            return Err(errors
                .into_iter()
                .map(|(src, e)| format!("`{{{{ {src} }}}}`: {e}"))
                .collect());
        }
        Ok(render_markdown(&text))
    }

    /// Term ids linked from the body (`term:` links).
    pub fn linked_terms(&self) -> Vec<String> {
        let mut out = Vec::new();
        for ev in Parser::new(&self.body) {
            if let Event::Start(Tag::Link { dest_url, .. }) = ev
                && let Some(t) = dest_url.strip_prefix("term:")
                && !out.iter().any(|o| o == t)
            {
                out.push(t.to_owned());
            }
        }
        out
    }
}

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    /// Glossary term id when the span is a `term:` link.
    pub term: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph(Vec<Span>),
    /// One entry per item; nested lists are flattened into their item.
    List {
        ordered: bool,
        items: Vec<Vec<Span>>,
    },
    Quote(Vec<Span>),
    Code(String),
    Rule,
}

/// A rendered lesson.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub blocks: Vec<Block>,
}

impl Document {
    /// Plain text of the whole document (tests, search).
    pub fn plain_text(&self) -> String {
        let spans = |s: &[Span]| s.iter().map(|s| s.text.as_str()).collect::<String>();
        self.blocks
            .iter()
            .map(|b| match b {
                Block::Heading { spans: s, .. } | Block::Paragraph(s) | Block::Quote(s) => spans(s),
                Block::List { items, .. } => items
                    .iter()
                    .map(|i| spans(i))
                    .collect::<Vec<_>>()
                    .join("\n"),
                Block::Code(c) => c.clone(),
                Block::Rule => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[derive(Default)]
struct Style {
    bold: u32,
    italic: u32,
    term: Option<String>,
}

fn push(spans: &mut Vec<Span>, style: &Style, text: &str, code: bool) {
    let span = Span {
        text: text.to_owned(),
        bold: style.bold > 0,
        italic: style.italic > 0,
        code,
        term: style.term.clone(),
    };
    match spans.last_mut() {
        Some(last)
            if last.bold == span.bold
                && last.italic == span.italic
                && last.code == span.code
                && last.term == span.term =>
        {
            last.text.push_str(text)
        }
        _ => spans.push(span),
    }
}

/// Markdown → blocks. Unsupported constructs (tables, images, HTML) render
/// as their text.
pub fn render_markdown(text: &str) -> Document {
    let mut blocks = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut style = Style::default();
    let mut list: Option<(bool, Vec<Vec<Span>>)> = None;
    let mut list_depth = 0u32;
    let mut in_quote = false;
    let mut code: Option<String> = None;
    let opts = Options::ENABLE_STRIKETHROUGH;
    for ev in Parser::new_ext(text, opts) {
        match ev {
            Event::Start(Tag::Emphasis) => style.italic += 1,
            Event::End(TagEnd::Emphasis) => style.italic = style.italic.saturating_sub(1),
            Event::Start(Tag::Strong) => style.bold += 1,
            Event::End(TagEnd::Strong) => style.bold = style.bold.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) => {
                style.term = dest_url.strip_prefix("term:").map(str::to_owned);
            }
            Event::End(TagEnd::Link) => style.term = None,
            Event::Start(Tag::List(start)) => {
                if list_depth == 0 {
                    list = Some((start.is_some(), Vec::new()));
                }
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                if list_depth == 0
                    && let Some((ordered, mut items)) = list.take()
                {
                    if !spans.is_empty() {
                        items.push(std::mem::take(&mut spans));
                    }
                    blocks.push(Block::List { ordered, items });
                }
            }
            Event::Start(Tag::Item) => {
                if !spans.is_empty()
                    && let Some((_, items)) = list.as_mut()
                {
                    items.push(std::mem::take(&mut spans));
                }
            }
            Event::End(TagEnd::Item) => {
                if let Some((_, items)) = list.as_mut()
                    && !spans.is_empty()
                {
                    items.push(std::mem::take(&mut spans));
                }
            }
            Event::Start(Tag::BlockQuote(_)) => in_quote = true,
            Event::End(TagEnd::BlockQuote(_)) => {
                in_quote = false;
                if !spans.is_empty() {
                    blocks.push(Block::Quote(std::mem::take(&mut spans)));
                }
            }
            Event::Start(Tag::CodeBlock(_)) => code = Some(String::new()),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(c) = code.take() {
                    blocks.push(Block::Code(c.trim_end().to_owned()));
                }
            }
            Event::Start(Tag::Heading { .. }) => {}
            Event::End(TagEnd::Heading(level)) => {
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    _ => 3,
                };
                blocks.push(Block::Heading {
                    level,
                    spans: std::mem::take(&mut spans),
                });
            }
            Event::End(TagEnd::Paragraph) => {
                if list.is_some() {
                    // Item text; a paragraph break inside an item becomes a space.
                    push(&mut spans, &style, " ", false);
                } else if in_quote {
                    if !spans.is_empty() {
                        push(&mut spans, &style, " ", false);
                    }
                } else if !spans.is_empty() {
                    blocks.push(Block::Paragraph(std::mem::take(&mut spans)));
                }
            }
            Event::Text(t) => match code.as_mut() {
                Some(c) => c.push_str(&t),
                None => push(&mut spans, &style, &t, false),
            },
            Event::Code(t) => push(&mut spans, &style, &t, true),
            Event::SoftBreak | Event::HardBreak => push(&mut spans, &style, " ", false),
            Event::Rule => blocks.push(Block::Rule),
            _ => {}
        }
    }
    // Trim trailing spaces left by paragraph ends inside items/quotes.
    for b in &mut blocks {
        let trim = |s: &mut Vec<Span>| {
            if let Some(last) = s.last_mut() {
                let t = last.text.trim_end().to_owned();
                last.text = t;
                if last.text.is_empty() {
                    s.pop();
                }
            }
        };
        match b {
            Block::List { items, .. } => items.iter_mut().for_each(trim),
            Block::Quote(s) => trim(s),
            _ => {}
        }
    }
    if !spans.is_empty() {
        blocks.push(Block::Paragraph(spans));
    }
    Document { blocks }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"+++
id = "pot-odds-101"
version = 3
title = "Pot odds"
prerequisites = ["outs-101"]
tags = ["math", "postflop"]
glossary = ["pot-odds", "equity"]
unlock = { quiz = "pot-odds-101-quiz" }
+++

# Pot odds

Facing a half-pot bet you need {{ calc: pot_odds(pot=100, bet=50) | pct(1) }} [equity](term:equity) to call.

- one **bold** item
- two *italic*

> A quote
> on two lines.
"#;

    #[test]
    fn parses_and_renders() {
        let l = Lesson::parse("en-US", SAMPLE).unwrap();
        assert_eq!(l.meta.id, "pot-odds-101");
        assert_eq!(l.meta.version, 3);
        assert_eq!(
            l.meta.unlock,
            Some(Unlock::Quiz {
                quiz: "pot-odds-101-quiz".into()
            })
        );
        assert_eq!(l.placeholders().unwrap().len(), 1);
        assert_eq!(l.linked_terms(), vec!["equity".to_owned()]);
        let doc = l.render(None).unwrap();
        assert_eq!(doc.blocks.len(), 4, "{doc:#?}");
        assert!(matches!(&doc.blocks[0], Block::Heading { level: 1, .. }));
        let Block::Paragraph(p) = &doc.blocks[1] else {
            panic!()
        };
        assert_eq!(p[0].text, "Facing a half-pot bet you need 25.0% ");
        assert_eq!(p[1].term.as_deref(), Some("equity"));
        let Block::List { ordered, items } = &doc.blocks[2] else {
            panic!("{doc:#?}")
        };
        assert!(!ordered);
        assert_eq!(items.len(), 2);
        assert!(items[0].iter().any(|s| s.bold && s.text == "bold"));
        let Block::Quote(q) = &doc.blocks[3] else {
            panic!()
        };
        assert_eq!(
            q.iter().map(|s| s.text.as_str()).collect::<String>(),
            "A quote on two lines."
        );

        let pt = Lesson::parse("pt-BR", SAMPLE).unwrap();
        assert!(pt.render(None).unwrap().plain_text().contains("25,0%"));
    }

    #[test]
    fn bad_front_matter() {
        assert!(Lesson::parse("en-US", "# no front matter").is_err());
        assert!(Lesson::parse("en-US", "+++\nid = \"x\"\n").is_err());
        assert!(Lesson::parse("en-US", "+++\nid = 1\n+++\n").is_err());
        assert!(
            Lesson::parse(
                "en-US",
                "+++\nid = \"x\"\nversion = 1\ntitle = \"t\"\nbogus = 1\n+++\n"
            )
            .is_err()
        );
    }

    #[test]
    fn render_errors_are_reported() {
        let l = Lesson::parse(
            "en-US",
            "+++\nid = \"x\"\nversion = 1\ntitle = \"t\"\n+++\n{{ calc: nope() }}",
        )
        .unwrap();
        assert_eq!(l.render(None).unwrap_err().len(), 1);
    }

    #[test]
    fn nested_lists_flatten() {
        let d = render_markdown("- a\n  - b\n  - c\n- d\n\nafter\n");
        let Block::List { items, .. } = &d.blocks[0] else {
            panic!("{d:#?}")
        };
        let texts: Vec<String> = items
            .iter()
            .map(|i| i.iter().map(|s| s.text.as_str()).collect())
            .collect();
        assert_eq!(texts, ["a", "b", "c", "d"]);
        assert!(matches!(&d.blocks[1], Block::Paragraph(_)));
    }

    #[test]
    fn ordered_lists_and_code() {
        let d = render_markdown("1. a\n2. b\n\n```\nx = 1\n```\n\n---\n");
        assert!(matches!(&d.blocks[0], Block::List { ordered: true, items } if items.len() == 2));
        assert_eq!(d.blocks[1], Block::Code("x = 1".into()));
        assert_eq!(d.blocks[2], Block::Rule);
    }
}
