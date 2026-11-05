//! Postflop build manifest (spec §7.1, §7.3).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use taful_core::action::Line;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Preflop chart that supplies the ranges, relative to the manifest.
    pub preflop_chart: String,
    /// Flop order file (`library/flops.txt`), relative to the manifest.
    pub flops: String,
    /// How many flops of the order to solve.
    pub flop_count: usize,
    pub solve: Solve,
    pub trees: BTreeMap<String, Tree>,
    pub spots: Vec<Spot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Solve {
    /// Stop when exploitability ≤ this percent of the starting pot.
    pub target_exploitability_pct: f32,
    pub max_iterations: u32,
    /// Exploitability is computed every this many iterations.
    pub check_every: u32,
    /// 16-bit storage in the solver (half the memory, same answers, §9.2).
    pub compression: bool,
    /// Drop decision nodes where the actor's range keeps less than this
    /// percent of its combos (spec §7.5).
    pub prune_reach_pct: f32,
    /// Store a combo at a node only if the actor plays its line to the node
    /// at least this often (0..1).
    pub min_combo_reach: f32,
}

pub use taful_solver::postflop::TreeSpec as Tree;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spot {
    /// Directory name in the library, e.g. `btn-bb-srp`.
    pub id: String,
    /// Preflop line in the chart's notation.
    pub line: String,
    /// Key into `trees`.
    pub tree: String,
}

impl Spot {
    pub fn line(&self) -> Result<Line, String> {
        self.line
            .parse()
            .map_err(|e| format!("spot {}: line: {e:?}", self.id))
    }
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let m: Manifest = toml::from_str(text).map_err(|e| e.to_string())?;
        m.validate()?;
        Ok(m)
    }

    fn validate(&self) -> Result<(), String> {
        let s = &self.solve;
        if s.target_exploitability_pct.is_nan()
            || s.target_exploitability_pct <= 0.0
            || s.max_iterations == 0
            || s.check_every == 0
        {
            return Err("solve: target, max_iterations and check_every must be positive".into());
        }
        if !(0.0..100.0).contains(&s.prune_reach_pct) || !(0.0..1.0).contains(&s.min_combo_reach) {
            return Err("solve: prune_reach_pct in [0, 100), min_combo_reach in [0, 1)".into());
        }
        let mut ids = std::collections::HashSet::new();
        for spot in &self.spots {
            if !ids.insert(&spot.id) {
                return Err(format!("duplicate spot id {}", spot.id));
            }
            if !spot
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err(format!("spot id {:?}: use [a-z0-9-]", spot.id));
            }
            if !self.trees.contains_key(&spot.tree) {
                return Err(format!("spot {}: unknown tree {}", spot.id, spot.tree));
            }
            spot.line()?;
        }
        for (name, t) in &self.trees {
            for (street, [b, r]) in [("flop", &t.flop), ("turn", &t.turn), ("river", &t.river)] {
                postflop_solver::BetSizeOptions::try_from((b.as_str(), r.as_str()))
                    .map_err(|e| format!("tree {name} {street}: {e}"))?;
            }
            // The app re-solves rivers with these sizes (spec §7.7), so they
            // must be in the subset Taful's river solver reads.
            taful_solver::river::RiverTree::from_spec(t)
                .map_err(|e| format!("tree {name} river (in-app solver): {e}"))?;
        }
        Ok(())
    }

    /// BLAKE3 of the manifest's canonical TOML.
    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(toml::to_string(self).expect("serializable").as_bytes()).as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_manifest_parses() {
        let m = Manifest::parse(include_str!("../../manifests/postflop-6max-100bb.toml")).unwrap();
        assert_eq!(m.spots.len(), 5);
        assert_eq!(m.flop_count, 49);
    }
}
