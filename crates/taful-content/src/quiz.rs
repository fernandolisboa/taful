//! Quizzes (spec §12.1): one TOML file per quiz under `content/quizzes/`,
//! every user-facing string given per locale.
//!
//! ```toml
//! id = "pot-odds-101-quiz"
//! version = 1
//! lesson = "pot-odds-101"
//! pass = 0.8
//!
//! [[questions]]
//! id = "half-pot"
//! kind = "numeric"
//! prompt = { en-US = "…", pt-BR = "…" }
//! answer = "{{ calc: pot_odds(pot=100, bet=50) | pct(1) }}"
//! tolerance = 1.0
//!
//! [[questions]]
//! id = "mdf"
//! kind = "choice"
//! prompt = { en-US = "…", pt-BR = "…" }
//! choices = [{ en-US = "…", pt-BR = "…" }, …]
//! answer = 1
//! ```
//!
//! A numeric answer is a placeholder; the value to type is the formatted
//! number (percentage points for `pct`), compared within `tolerance`.
//! Strings may contain placeholders too.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::expr::{self, Placeholder, SpotProvider};

/// A string in every locale, keyed by BCP 47 tag.
pub type Localized = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quiz {
    pub id: String,
    pub version: u32,
    /// Lesson the quiz unlocks.
    pub lesson: String,
    /// Score (0..1) needed to pass.
    #[serde(default = "default_pass")]
    pub pass: f64,
    pub questions: Vec<Question>,
}

fn default_pass() -> f64 {
    0.8
}

// No `deny_unknown_fields` here: serde does not support it with `flatten`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    /// Unique within the quiz; with the quiz id, the review-item key.
    pub id: String,
    pub prompt: Localized,
    #[serde(flatten)]
    pub kind: QuestionKind,
    /// Shown after answering.
    #[serde(default)]
    pub explanation: Option<Localized>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuestionKind {
    Choice {
        choices: Vec<Localized>,
        /// Index of the right choice.
        answer: usize,
    },
    Numeric {
        /// A placeholder (`{{ calc: … | pct(1) }}`).
        answer: String,
        /// Accepted distance, in the formatter's unit.
        tolerance: f64,
    },
}

/// The user's answer to one question.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Answer {
    Choice(usize),
    Numeric(f64),
}

/// A question ready to show in one locale.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedQuestion {
    pub id: String,
    pub prompt: String,
    pub choices: Vec<String>,
    /// For numeric questions: the exact value and its display text.
    pub numeric: Option<(f64, String)>,
    pub tolerance: f64,
    pub answer_index: Option<usize>,
    pub explanation: Option<String>,
}

impl RenderedQuestion {
    pub fn is_correct(&self, a: Answer) -> bool {
        match (a, self.answer_index, self.numeric.as_ref()) {
            (Answer::Choice(i), Some(right), _) => i == right,
            (Answer::Numeric(x), _, Some((v, _))) => {
                x.is_finite() && (x - v).abs() <= self.tolerance + 1e-9
            }
            _ => false,
        }
    }

    /// Display text of the right answer.
    pub fn answer_text(&self) -> String {
        match (self.answer_index, &self.numeric) {
            (Some(i), _) => self.choices.get(i).cloned().unwrap_or_default(),
            (_, Some((_, t))) => t.clone(),
            _ => String::new(),
        }
    }
}

impl Quiz {
    pub fn parse(text: &str) -> Result<Quiz, String> {
        let q: Quiz = toml::from_str(text).map_err(|e| e.to_string())?;
        if !(q.pass > 0.0 && q.pass <= 1.0) {
            return Err(format!("pass must be in (0, 1], got {}", q.pass));
        }
        Ok(q)
    }

    pub fn question(&self, id: &str) -> Option<&Question> {
        self.questions.iter().find(|q| q.id == id)
    }

    /// Every localized string with its location, for the checks.
    pub fn strings(&self) -> Vec<(String, &Localized)> {
        let mut out = Vec::new();
        for q in &self.questions {
            out.push((format!("{}.prompt", q.id), &q.prompt));
            if let Some(e) = &q.explanation {
                out.push((format!("{}.explanation", q.id), e));
            }
            if let QuestionKind::Choice { choices, .. } = &q.kind {
                for (i, c) in choices.iter().enumerate() {
                    out.push((format!("{}.choices[{i}]", q.id), c));
                }
            }
        }
        out
    }

    /// Every placeholder source per locale, for snapshots and parity checks.
    /// Numeric answers count in every locale.
    pub fn placeholders(&self, locale: &str) -> Result<Vec<Placeholder>, String> {
        let mut out = Vec::new();
        for (loc, s) in self.strings() {
            let text = s
                .get(locale)
                .ok_or_else(|| format!("{loc}: missing `{locale}`"))?;
            out.extend(expr::find_placeholders(text).map_err(|e| format!("{loc}: {e}"))?);
        }
        for q in &self.questions {
            if let QuestionKind::Numeric { answer, .. } = &q.kind {
                out.extend(
                    expr::find_placeholders(answer).map_err(|e| format!("{}.answer: {e}", q.id))?,
                );
            }
        }
        Ok(out)
    }

    /// Renders one question for `locale`.
    pub fn render_question(
        &self,
        q: &Question,
        locale: &str,
        spots: Option<&dyn SpotProvider>,
    ) -> Result<RenderedQuestion, Vec<String>> {
        let mut errors = Vec::new();
        let mut text = |what: &str, s: &Localized| -> String {
            let Some(raw) = s.get(locale) else {
                errors.push(format!("{}.{what}: missing `{locale}`", q.id));
                return String::new();
            };
            let (out, errs) = expr::substitute(raw, locale, spots);
            errors.extend(
                errs.into_iter()
                    .map(|(src, e)| format!("{}.{what}: `{{{{ {src} }}}}`: {e}", q.id)),
            );
            out
        };
        let prompt = text("prompt", &q.prompt);
        let explanation = q.explanation.as_ref().map(|e| text("explanation", e));
        let (choices, answer_index, numeric, tolerance) = match &q.kind {
            QuestionKind::Choice { choices, answer } => {
                let cs: Vec<String> = choices.iter().map(|c| text("choice", c)).collect();
                if *answer >= cs.len() {
                    errors.push(format!("{}: answer {answer} out of range", q.id));
                }
                if cs.len() < 2 {
                    errors.push(format!("{}: a choice question needs 2+ choices", q.id));
                }
                (cs, Some(*answer), None, 0.0)
            }
            QuestionKind::Numeric { answer, tolerance } => {
                let numeric = match expr::find_placeholders(answer) {
                    Ok(ps) if ps.len() == 1 && ps[0].span == (0..answer.trim_end().len()) => {
                        match expr::render(&ps[0].source, locale, spots) {
                            Ok(r) => match r.value {
                                Some(v) => Some((v, r.text)),
                                None => {
                                    errors.push(format!("{}.answer: not a number", q.id));
                                    None
                                }
                            },
                            Err(e) => {
                                errors.push(format!("{}.answer: {e}", q.id));
                                None
                            }
                        }
                    }
                    _ => {
                        errors.push(format!(
                            "{}.answer: must be exactly one `{{{{ calc: … }}}}` placeholder",
                            q.id
                        ));
                        None
                    }
                };
                if !(tolerance.is_finite() && *tolerance >= 0.0) {
                    errors.push(format!("{}: tolerance must be finite and >= 0", q.id));
                }
                (Vec::new(), None, numeric, *tolerance)
            }
        };
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(RenderedQuestion {
            id: q.id.clone(),
            prompt,
            choices,
            numeric,
            tolerance,
            answer_index,
            explanation,
        })
    }

    /// Score (0..1) of a full set of answers, in question order.
    pub fn score(rendered: &[RenderedQuestion], answers: &[Answer]) -> f64 {
        if rendered.is_empty() {
            return 0.0;
        }
        let right = rendered
            .iter()
            .zip(answers)
            .filter(|(q, a)| q.is_correct(**a))
            .count();
        right as f64 / rendered.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIZ: &str = r#"
id = "pot-odds-101-quiz"
version = 1
lesson = "pot-odds-101"

[[questions]]
id = "half-pot"
kind = "numeric"
prompt = { en-US = "Pot 100, bet 50. Equity needed?", pt-BR = "Pote 100, aposta 50. Equity necessária?" }
answer = "{{ calc: pot_odds(pot=100, bet=50) | pct(1) }}"
tolerance = 1.0

[[questions]]
id = "pick"
kind = "choice"
prompt = { en-US = "MDF vs pot?", pt-BR = "MDF contra pote?" }
choices = [{ en-US = "{{ calc: mdf(pot=100, bet=100) | pct(0) }}", pt-BR = "{{ calc: mdf(pot=100, bet=100) | pct(0) }}" }, { en-US = "33%", pt-BR = "33%" }]
answer = 0
explanation = { en-US = "pot / (pot + bet)", pt-BR = "pote / (pote + aposta)" }
"#;

    #[test]
    fn parses_renders_scores() {
        let q = Quiz::parse(QUIZ).unwrap();
        assert_eq!(q.pass, 0.8);
        let r: Vec<_> = q
            .questions
            .iter()
            .map(|x| q.render_question(x, "pt-BR", None).unwrap())
            .collect();
        assert_eq!(r[0].numeric, Some((25.0, "25,0%".into())));
        assert_eq!(r[1].choices[0], "50%");
        assert!(r[0].is_correct(Answer::Numeric(24.5)));
        assert!(!r[0].is_correct(Answer::Numeric(27.0)));
        assert!(!r[0].is_correct(Answer::Choice(0)));
        assert_eq!(
            Quiz::score(&r, &[Answer::Numeric(25.0), Answer::Choice(1)]),
            0.5
        );
        assert_eq!(q.placeholders("en-US").unwrap().len(), 2);
        assert_eq!(r[1].answer_text(), "50%");
    }

    #[test]
    fn bad_numeric_answer() {
        let bad = QUIZ.replace(
            "answer = \"{{ calc: pot_odds(pot=100, bet=50) | pct(1) }}\"",
            "answer = \"about {{ calc: pot_odds(pot=100, bet=50) | pct(1) }}\"",
        );
        let q = Quiz::parse(&bad).unwrap();
        assert!(q.render_question(&q.questions[0], "en-US", None).is_err());
        let q = Quiz::parse(&QUIZ.replace("answer = 0", "answer = 5")).unwrap();
        assert!(q.render_question(&q.questions[1], "en-US", None).is_err());
    }
}
