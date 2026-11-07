//! Bots (spec §9, D8): the reference bot and profiled bots.
//!
//! A bot picks an action at a preflop chart node for its hand by sampling a
//! strategy with the hand's seeded RNG, so hands replay exactly. The
//! reference bot samples the chart; a profiled bot samples the profile's
//! transform of the chart (`taful_core::profile`), locked at every node it
//! acts. The build tool computes best responses against the very same
//! transform (it reads the same chart data), which is what spot mode grades
//! against.
//!
//! Postflop spots against a profile do not go through this trait: the
//! library file stores the profile's flop and turn strategy next to hero's
//! best-response EVs, and the river applies the same transform to the
//! in-app solve (`walk.rs`, #27). Full-hand mode (#28, `hand.rs`) samples
//! this trait preflop and the walk's profile data postflop.

use rand::seq::IndexedRandom;
use rand_chacha::ChaCha8Rng;
use taful_core::action::Action;
use taful_core::cards::Hand;
use taful_core::profile::Profile;
use taful_core::range::HandClass;
use taful_solver::preflop::{NodeId, PreflopChart};

/// A preflop opponent (spec §9).
pub trait Bot {
    /// Action frequencies at `node` for `hand`, in the node's action order;
    /// `None` if the chart holds no data for the hand.
    fn strategy(&self, chart: &PreflopChart, node: NodeId, hand: Hand) -> Option<Vec<f32>>;

    /// Samples an action at `node` for `hand`.
    fn act(
        &self,
        chart: &PreflopChart,
        node: NodeId,
        hand: Hand,
        rng: &mut ChaCha8Rng,
    ) -> Option<Action> {
        let freqs = self.strategy(chart, node, hand)?;
        let idx: Vec<usize> = (0..freqs.len()).collect();
        let &a = idx.choose_weighted(rng, |&i| freqs[i]).ok()?;
        Some(chart.node(node).actions[a].action)
    }
}

/// Plays the reference strategy.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReferenceBot;

impl Bot for ReferenceBot {
    fn strategy(&self, chart: &PreflopChart, node: NodeId, hand: Hand) -> Option<Vec<f32>> {
        chart.strategy(node, hand)
    }
}

/// Plays a profile's transform of the reference strategy.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfiledBot {
    profile: Profile,
}

impl ProfiledBot {
    pub fn new(profile: Profile) -> ProfiledBot {
        ProfiledBot { profile }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// The profile's strategy at `node` for every hand class, row-major
    /// `169 × actions`.
    pub fn node_strategy(&self, chart: &PreflopChart, node: NodeId) -> Vec<f32> {
        let n = chart.node(node);
        let k = n.actions.len();
        let scale = chart.file().header.ev_scale;
        let mut strategy = Vec::with_capacity(HandClass::COUNT * k);
        let mut ev = Vec::with_capacity(HandClass::COUNT * k);
        for c in 0..HandClass::COUNT {
            strategy.extend(n.strategy(c).unwrap_or_else(|| vec![0.0; k]));
            ev.extend(n.ev(c, scale).unwrap_or_else(|| vec![0.0; k]));
        }
        let actions: Vec<Action> = n.actions.iter().map(|a| a.action).collect();
        self.profile
            .preflop_strategy(&actions, &strategy, &ev, &chart.acting_range(node))
    }
}

impl Bot for ProfiledBot {
    fn strategy(&self, chart: &PreflopChart, node: NodeId, hand: Hand) -> Option<Vec<f32>> {
        let k = chart.node(node).actions.len();
        let c = HandClass::of(hand).index();
        chart.node(node).strategy(c)?;
        let all = self.node_strategy(chart, node);
        Some(all[c * k..(c + 1) * k].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;
    use crate::preflop::PreflopTrainer;

    fn trainer() -> PreflopTrainer {
        PreflopTrainer::builtin().unwrap()
    }

    /// Range share of each action at `node` under `rows` (169 × k).
    fn range_share(chart: &PreflopChart, node: NodeId, rows: &[f32]) -> Vec<f32> {
        let k = chart.node(node).actions.len();
        let range = chart.acting_range(node);
        (0..k)
            .map(|a| {
                (0..HandClass::COUNT)
                    .map(|c| range[c] * rows[c * k + a])
                    .sum()
            })
            .collect()
    }

    #[test]
    fn the_app_and_the_build_tool_agree_on_every_profile_strategy() {
        // The best-response chart stores the profile strategy the build
        // tool locked; the app's ProfiledBot must compute the same one.
        let t = trainer();
        assert_eq!(t.profiles().len(), 4);
        for p in t.profiles() {
            for node in 0..t.chart().len() {
                let ours = p.bot.node_strategy(t.chart(), node);
                let k = t.chart().node(node).actions.len();
                for c in 0..HandClass::COUNT {
                    let stored = p.chart.node(node).strategy(c).unwrap();
                    for a in 0..k {
                        assert!(
                            (ours[c * k + a] - stored[a]).abs() < 0.01,
                            "{} node {node} class {c}: {} vs {}",
                            p.bot.profile().id,
                            ours[c * k + a],
                            stored[a]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn profiles_deviate_the_way_their_names_say() {
        let t = trainer();
        let chart = t.chart();
        let root = chart.root(); // UTG, unopened
        let fold = chart.node(root).action_index(Action::Fold).unwrap();
        let open = |id: &str| {
            1.0 - range_share(
                chart,
                root,
                &t.profile(id).unwrap().bot.node_strategy(chart, root),
            )[fold]
        };
        let reference = 1.0 - range_share(chart, root, &ReferenceBot.node_rows(chart, root))[fold];
        assert!(open("nit") < reference, "{} !< {reference}", open("nit"));
        assert!(
            open("maniac") > reference,
            "{} !> {reference}",
            open("maniac")
        );
        // No limps in the tree: a station's loose limps are loose opens.
        assert!(open("calling-station") > reference);
        // Facing a BTN open, the station calls more and 3-bets less than
        // the reference big blind.
        let node = match chart.find(&"UTG:f,HJ:f,CO:f,BTN:r250,SB:f".parse().unwrap()) {
            taful_solver::preflop::Lookup::Node(n) => n,
            other => panic!("{other:?}"),
        };
        let n = chart.node(node);
        let (f, c) = (
            n.action_index(Action::Fold).unwrap(),
            n.action_index(Action::Call).unwrap(),
        );
        let station = range_share(
            chart,
            node,
            &t.profile("calling-station")
                .unwrap()
                .bot
                .node_strategy(chart, node),
        );
        let refr = range_share(chart, node, &ReferenceBot.node_rows(chart, node));
        assert!(station[f] < refr[f], "{station:?} vs {refr:?}");
        let raise = |v: &[f32]| 1.0 - v[f] - v[c];
        assert!(raise(&station) < raise(&refr), "{station:?} vs {refr:?}");
    }

    #[test]
    fn bots_are_deterministic_under_a_seed() {
        let t = trainer();
        let bot = &t.profile("maniac").unwrap().bot;
        let hand: Hand = "Ts9s".parse().unwrap();
        let draw = |seed: u64| {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            (0..20)
                .map(|_| bot.act(t.chart(), t.chart().root(), hand, &mut rng))
                .collect::<Vec<_>>()
        };
        assert_eq!(draw(7), draw(7));
        assert!(draw(7).iter().all(Option::is_some));
    }

    impl ReferenceBot {
        fn node_rows(&self, chart: &PreflopChart, node: NodeId) -> Vec<f32> {
            (0..HandClass::COUNT)
                .flat_map(|c| chart.node(node).strategy(c).unwrap())
                .collect()
        }
    }
}
