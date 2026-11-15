//! Target stats per profile and the search that moves a profile's knobs
//! toward them.
//!
//! Each tuned stat is driven mainly by one knob, so the search is
//! coordinate-wise: bisect one knob until its stat hits the target with the
//! other knobs fixed, then the next, and repeat the rounds until every
//! tuned stat sits inside its band (the knobs interact a little: opening
//! wider also raises VPIP). Preflop first, since the flop stats depend on
//! the preflop range.

use std::collections::BTreeMap;

use serde::Deserialize;
use taful_core::profile::{MAX_MULT, Profile};

/// `[target, tolerance]`.
pub type Band = [f64; 2];

/// Target HUD stats of one profile (`tools/profile-stats/targets.toml`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Targets {
    pub vpip: Band,
    pub pfr: Band,
    pub threebet: Band,
    pub fold_to_cbet: Band,
    pub af: Band,
}

pub fn load_targets(s: &str) -> Result<BTreeMap<String, Targets>, String> {
    toml::from_str(s).map_err(|e| e.to_string())
}

pub fn within(v: Option<f64>, b: Band) -> bool {
    v.is_some_and(|v| (v - b[0]).abs() <= b[1])
}

/// Find `x` in `[lo, hi]` with `f(x)` closest to `target`, assuming `f`
/// grows with `x` (`log` = search on a log scale). `steps` halvings.
pub fn bisect(
    lo: f64,
    hi: f64,
    log: bool,
    steps: usize,
    target: f64,
    mut f: impl FnMut(f64) -> Option<f64>,
) -> f64 {
    type Map = fn(f64) -> f64;
    let (to, from): (Map, Map) = if log {
        (f64::ln, f64::exp)
    } else {
        (|x| x, |x| x)
    };
    let (mut a, mut b) = (to(lo), to(hi));
    let mut best = (f64::INFINITY, from((a + b) / 2.0));
    for _ in 0..steps {
        let m = (a + b) / 2.0;
        let x = from(m);
        let Some(v) = f(x) else { break };
        if (v - target).abs() < best.0 {
            best = ((v - target).abs(), x);
        }
        if v < target {
            a = m;
        } else {
            b = m;
        }
    }
    best.1
}

/// Round to 3 significant decimals so the TOMLs stay readable.
pub fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Lowest multiplier the search tries.
pub const MIN_MULT: f64 = 0.05;

/// The knobs in their search order with the range they move in.
pub fn preflop_knobs() -> [(&'static str, f64, f64); 3] {
    [
        ("threebet_mult", MIN_MULT, MAX_MULT),
        ("vpip_mult", MIN_MULT, MAX_MULT),
        ("pfr_mult", MIN_MULT, MAX_MULT),
    ]
}

pub fn set_preflop(p: &mut Profile, knob: &str, v: f64) {
    match knob {
        "threebet_mult" => p.preflop.threebet_mult = v,
        "vpip_mult" => p.preflop.vpip_mult = v,
        "pfr_mult" => p.preflop.pfr_mult = v,
        _ => unreachable!("knob {knob}"),
    }
}

/// Rewrite `key = value` lines of `section` in a profile TOML, keeping the
/// comments and layout.
pub fn rewrite_toml(text: &str, section: &str, values: &[(&str, f64)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut current = String::new();
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            current = t[1..t.len() - 1].trim().to_string();
        }
        let mut replaced = false;
        if current == section {
            for (key, v) in values {
                let Some(rest) = t.strip_prefix(key) else {
                    continue;
                };
                if !rest.trim_start().starts_with('=') {
                    continue;
                }
                let indent = &line[..line.len() - line.trim_start().len()];
                let nl = if line.ends_with('\n') { "\n" } else { "" };
                out.push_str(&format!("{indent}{key} = {}{nl}", fmt_num(*v)));
                replaced = true;
                break;
            }
        }
        if !replaced {
            out.push_str(line);
        }
    }
    out
}

fn fmt_num(v: f64) -> String {
    let s = format!("{:.3}", round3(v));
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.contains('.') { s } else { format!("{s}.0") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bisect_finds_the_target_of_a_growing_function() {
        let x = bisect(0.05, 5.0, true, 30, 2.0, |x| Some(x * x));
        assert!((x - 2f64.sqrt()).abs() < 1e-4, "{x}");
        let y = bisect(-1.0, 1.0, false, 30, 0.25, Some);
        assert!((y - 0.25).abs() < 1e-4, "{y}");
    }

    #[test]
    fn rewrite_keeps_comments_and_other_sections() {
        let text = "# c\nid = \"x\"\n\n[preflop]\nvpip_mult = 1.8\npfr_mult = 1.3\n\n[postflop]\n# shift\naggression = -0.4\nvpip_mult = 9\n";
        let out = rewrite_toml(text, "preflop", &[("vpip_mult", 2.25), ("pfr_mult", 0.5)]);
        assert_eq!(
            out,
            "# c\nid = \"x\"\n\n[preflop]\nvpip_mult = 2.25\npfr_mult = 0.5\n\n[postflop]\n# shift\naggression = -0.4\nvpip_mult = 9\n"
        );
        let out = rewrite_toml(&out, "postflop", &[("aggression", -0.1234)]);
        assert!(out.contains("aggression = -0.123\n"));
    }

    #[test]
    fn shipped_targets_parse_and_cover_every_profile() {
        let t = load_targets(include_str!("../targets.toml")).expect("parses");
        for id in ["nit", "tag", "calling-station", "maniac"] {
            assert!(t.contains_key(id), "{id}");
        }
    }
}
