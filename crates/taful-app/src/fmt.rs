//! Locale-aware display helpers for the UI: numbers, cards, times.

use taful_store::Millis;

use crate::i18n::Locale;

/// `x` with `decimals` decimals and grouped thousands (pt-BR: `1.234,5`).
pub fn number(x: f64, decimals: u8, locale: Locale) -> String {
    taful_content::format::number(x, decimals, locale.tag())
}

/// A number with up to `max_decimals` decimals, trailing zeros dropped.
pub fn number_trim(x: f64, max_decimals: u8, locale: Locale) -> String {
    let s = number(x, max_decimals, locale);
    let dec = if locale.tag().starts_with("pt") {
        ','
    } else {
        '.'
    };
    if s.contains(dec) {
        s.trim_end_matches('0').trim_end_matches(dec).to_owned()
    } else {
        s
    }
}

/// Card text with suit symbols: `AhKs` → `A♥ K♠`. Text that is not cards
/// is returned unchanged.
pub fn cards(s: &str) -> String {
    let Ok(cs) = taful_core::cards::parse_cards(s) else {
        return s.to_owned();
    };
    cs.iter()
        .map(|c| {
            let t = c.to_string();
            let (rank, suit) = t.split_at(1);
            let sym = match suit {
                "s" => '♠',
                "h" => '♥',
                "d" => '♦',
                _ => '♣',
            };
            format!("{rank}{sym}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Range text with spaces after commas, for wrapping.
pub fn range(s: &str) -> String {
    s.replace(',', ", ")
}

/// Local date and time (`05/11/2026 09:30` in pt-BR, `2026-11-05 09:30`
/// otherwise).
pub fn datetime(ms: Millis, locale: Locale) -> String {
    let Ok(ts) = jiff::Timestamp::from_millisecond(ms) else {
        return String::new();
    };
    let z = ts.to_zoned(jiff::tz::TimeZone::system());
    let pattern = if locale == Locale::PtBr {
        "%d/%m/%Y %H:%M"
    } else {
        "%Y-%m-%d %H:%M"
    };
    z.strftime(pattern).to_string()
}

/// Parses a number typed by the user: either decimal separator, optional
/// `%` or `:1` suffix, surrounding spaces.
pub fn parse_number(s: &str) -> Option<f64> {
    let t = s
        .trim()
        .trim_end_matches('%')
        .trim_end_matches(":1")
        .trim()
        .replace(',', ".");
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|x| x.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(number(1234.5, 1, Locale::PtBr), "1.234,5");
        assert_eq!(number_trim(25.0, 1, Locale::EnUs), "25");
        assert_eq!(number_trim(33.3, 1, Locale::PtBr), "33,3");
        assert_eq!(cards("AhKs"), "A♥ K♠");
        assert_eq!(cards("nope"), "nope");
        assert_eq!(parse_number(" 33,3% "), Some(33.3));
        assert_eq!(parse_number("17.7:1"), Some(17.7));
        assert_eq!(parse_number("-"), None);
        assert_eq!(parse_number(""), None);
        assert!(!datetime(1_762_340_000_000, Locale::PtBr).is_empty());
    }
}
