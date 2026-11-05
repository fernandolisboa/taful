//! Formatters (spec §12.2): `pct(d)`, `bb(d)`, `ratio`, `int`, `cards`, with
//! localized numbers (pt-BR uses a decimal comma and a dot for thousands).

use taful_core::cards::parse_cards;

use crate::expr::{ExprError, Rendered, Value};

/// A `| name(arg)` suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formatter {
    /// Fraction as a percentage with `d` decimals: `0.25` → `25.0%`.
    Pct(u8),
    /// Big blinds with `d` decimals: `2.5 bb`.
    Bb(u8),
    /// `x:1` with `d` decimals (default 1).
    Ratio(u8),
    /// Rounded whole number with thousands separators.
    Int,
    /// Card text with suit symbols: `AhKs` → `A♥ K♠`.
    Cards,
}

impl Formatter {
    pub fn new(name: &str, arg: Option<u8>) -> Result<Formatter, ExprError> {
        let no_arg = |f: Formatter| match arg {
            None => Ok(f),
            Some(_) => Err(ExprError(format!("formatter `{name}` takes no argument"))),
        };
        match name {
            "pct" => Ok(Formatter::Pct(arg.unwrap_or(0))),
            "bb" => Ok(Formatter::Bb(arg.unwrap_or(1))),
            "ratio" => Ok(Formatter::Ratio(arg.unwrap_or(1))),
            "int" => no_arg(Formatter::Int),
            "cards" => no_arg(Formatter::Cards),
            other => Err(ExprError(format!(
                "unknown formatter `{other}` (pct, bb, ratio, int, cards)"
            ))),
        }
    }
}

/// Decimal and thousands separators for a locale tag.
fn separators(locale: &str) -> (char, char) {
    if locale.starts_with("pt") {
        (',', '.')
    } else {
        ('.', ',')
    }
}

/// `x` with `decimals` decimals and grouped thousands, in `locale`.
pub fn number(x: f64, decimals: u8, locale: &str) -> String {
    let (dec, group) = separators(locale);
    let s = format!("{:.*}", decimals as usize, x.abs());
    let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(group);
        }
        grouped.push(c);
    }
    // No "-0.0".
    let negative = x < 0.0 && s.bytes().any(|b| matches!(b, b'1'..=b'9'));
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&grouped);
    if !frac.is_empty() {
        out.push(dec);
        out.push_str(frac);
    }
    out
}

/// Formats an evaluated value.
pub fn apply(f: Option<&Formatter>, v: &Value, locale: &str) -> Result<Rendered, ExprError> {
    let numeric = |x: f64, text: String| {
        Ok(Rendered {
            text,
            value: Some(x),
        })
    };
    match (f, v) {
        (Some(Formatter::Pct(d)), Value::Num(x)) => {
            numeric(100.0 * x, format!("{}%", number(100.0 * x, *d, locale)))
        }
        (Some(Formatter::Bb(d)), Value::Num(x)) => {
            numeric(*x, format!("{} bb", number(*x, *d, locale)))
        }
        (Some(Formatter::Ratio(d)), Value::Num(x)) => {
            numeric(*x, format!("{}:1", number(*x, *d, locale)))
        }
        (Some(Formatter::Int), Value::Num(x)) => numeric(x.round(), number(x.round(), 0, locale)),
        (None, Value::Num(x)) => numeric(*x, number(*x, 2, locale)),
        (Some(Formatter::Cards), Value::Text(t)) => {
            let cards = parse_cards(t).map_err(|e| ExprError(e.to_string()))?;
            let text = cards
                .iter()
                .map(|c| {
                    let s = c.to_string();
                    let (rank, suit) = s.split_at(1);
                    let sym = match suit {
                        "s" => '♠',
                        "h" => '♥',
                        "d" => '♦',
                        _ => '♣',
                    };
                    format!("{rank}{sym}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            Ok(Rendered { text, value: None })
        }
        (None, Value::Text(t)) => Ok(Rendered {
            text: t.clone(),
            value: None,
        }),
        (Some(Formatter::Cards), Value::Num(_)) => {
            Err(ExprError("`cards` formats card text, not a number".into()))
        }
        (Some(_), Value::Text(_)) => Err(ExprError("this formatter needs a number".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(number(1234567.891, 2, "en-US"), "1,234,567.89");
        assert_eq!(number(1234567.891, 2, "pt-BR"), "1.234.567,89");
        assert_eq!(number(0.5, 0, "en-US"), "0");
        assert_eq!(number(-0.01, 1, "en-US"), "0.0");
        assert_eq!(number(-2.5, 1, "pt-BR"), "-2,5");
        assert_eq!(number(999.0, 0, "en-US"), "999");
        assert_eq!(number(1000.0, 0, "en-US"), "1,000");
    }
}
