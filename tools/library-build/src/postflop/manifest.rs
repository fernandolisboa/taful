//! Postflop build manifest (spec §7.1, §7.3).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use std::path::Path;

use taful_core::action::Line;
use taful_core::profile::Profile;

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
    /// Bot profiles whose best responses each solve computes inline (D8,
    /// #27). Part of the hash, with every listed profile's contents: a
    /// changed profile means a new build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Profiles>,
}

pub use crate::preflop::manifest::Profiles;

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
    /// With profiles: the weight at which the solve seeds every combo a
    /// profile brings to the flop that the reference never does, so the
    /// solve plays it and the profile's best responses start from the
    /// profile's own preflop range (#48). Tiny, so the reference barely
    /// moves; it costs memory and time in proportion to the wider ranges
    /// (docs/spikes.md). Without it the best responses still start from
    /// the profile's range, restricted to the reference's combos, and a
    /// bot holding another combo leaves the library (`bot-off-range`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_seed: Option<f32>,
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
        if s.profile_seed.is_some_and(|x| !(x > 0.0 && x <= 0.01)) {
            return Err("solve: profile_seed in (0, 0.01]".into());
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

    /// BLAKE3 of the manifest's canonical TOML and, when it lists
    /// profiles, each one's canonical TOML (`profiles` = what
    /// [`load_profiles`](Self::load_profiles) returned). Without profiles
    /// this is the hash of the first-slice files.
    pub fn hash(&self, profiles: &[Profile]) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(toml::to_string(self).expect("serializable").as_bytes());
        for p in profiles {
            h.update(b"\n");
            h.update(p.to_toml().as_bytes());
        }
        *h.finalize().as_bytes()
    }

    /// The listed profiles, from `<manifest dir>/<profiles.dir>/<id>.toml`.
    pub fn load_profiles(&self, manifest_dir: &Path) -> Result<Vec<Profile>, String> {
        self.profiles
            .as_ref()
            .map_or(Ok(Vec::new()), |set| set.load(manifest_dir))
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

    #[test]
    fn profiles_move_the_hash_and_none_keeps_it() {
        let text = include_str!("../../manifests/fixture.toml");
        let m = Manifest::parse(text).unwrap();
        assert!(m.profiles.is_none());
        // Without profiles: the plain BLAKE3 of the canonical TOML, as the
        // first-slice files were built.
        let plain = *blake3::hash(toml::to_string(&m).unwrap().as_bytes()).as_bytes();
        assert_eq!(m.hash(&[]), plain);
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests");
        let with = Manifest::parse(include_str!("../../manifests/fixture-profiles.toml")).unwrap();
        let profiles = with.load_profiles(&dir).unwrap();
        assert_eq!(profiles.len(), 4);
        let mut edited = profiles.clone();
        edited[0].postflop.aggression += 0.1;
        assert_ne!(with.hash(&profiles), with.hash(&edited));
        assert_ne!(with.hash(&profiles), m.hash(&[]));
    }
}
