//! Export a solved `PostFlopGame` to Taful's format, truncated at the river
//! (spec §7.5, D11): flop decisions, one chance node per closed flop line
//! (turn cards, with suit-isomorphic turns stored as mirrors), turn
//! decisions, and `End` where the river would be dealt.

use std::collections::HashMap;

use postflop_solver::{Action as SAction, PostFlopGame};
use taful_core::action::Action;
use taful_core::cards::{Card, Hand, NUM_COMBOS};
use taful_core::config::{BB_CHIPS, Chips, GameConfig};
use taful_core::iso::SuitPerm;
use taful_solver::format::{
    Branch, ChanceNode, Child, Domain, Header, LibraryFile, Node, NodeAction, ev_scale_for,
    quantize_ev, quantize_strategy,
};

use taful_core::profile::Profile;

use super::manifest::Manifest;
use super::{Job, SOLVER_ID, SolveReport};

struct RawNode {
    player: usize,
    /// Solver hand index of each live row.
    rows: Vec<usize>,
    pot: Chips,
    actions: Vec<NodeAction>,
    live: Vec<u16>,
    /// `[live][action]`.
    strategy: Vec<f32>,
    /// `[live][action]`, bb.
    ev: Vec<f32>,
}

struct Ex<'a> {
    job: &'a Job<'a>,
    prune_reach_pct: f32,
    min_combo_reach: f32,
    /// Per player: root weight per solver hand index, and the root total of
    /// normalized weights.
    root_weights: [Vec<f32>; 2],
    root_total: [f32; 2],
    /// Per player: solver hand index → Taful combo.
    combos: [Vec<u16>; 2],
    stabilizer: Vec<SuitPerm>,
    nodes: Vec<Option<RawNode>>,
    /// Interpreter history of each node.
    histories: Vec<Vec<usize>>,
    chance: Vec<ChanceNode>,
    /// Per node history: the largest combo-pair mass any profile (bot
    /// under the profile, responder under the reference) brings to it, on
    /// the normalized weights' scale; empty without profiles (#45).
    profile_mass: HashMap<Vec<usize>, f64>,
    /// Whether profiles are listed (then every node has a mass).
    profiles: bool,
    pruned: usize,
    /// Nodes kept only because a profile reaches them.
    kept_for_profiles: usize,
}

pub(crate) fn map_action(a: SAction) -> Result<Action, String> {
    let chips = |x: i32| Chips::try_from(x).map_err(|e| e.to_string());
    Ok(match a {
        SAction::Fold => Action::Fold,
        SAction::Check => Action::Check,
        SAction::Call => Action::Call,
        SAction::Bet(x) => Action::Bet(chips(x)?),
        SAction::Raise(x) => Action::Raise(chips(x)?),
        SAction::AllIn(_) => Action::AllIn,
        other => return Err(format!("unexpected action {other:?}")),
    })
}

impl Ex<'_> {
    fn all_in(&self, game: &PostFlopGame) -> bool {
        let stack = self.job.geometry.stack as i32;
        game.total_bet_amount().iter().any(|&b| b >= stack)
    }

    /// Visit the node at `history`; returns how its parent points at it.
    fn visit(
        &mut self,
        game: &mut PostFlopGame,
        history: &mut Vec<usize>,
    ) -> Result<Child, String> {
        game.apply_history(history);
        if game.is_terminal_node() {
            return Ok(Child::End);
        }
        if game.is_chance_node() {
            // River chance (turn closed) or an all-in runout: the library
            // ends here; the app solves rivers on demand (D11).
            if game.current_board().len() >= 4 || self.all_in(game) {
                return Ok(Child::End);
            }
            return self.turn_chance(game, history);
        }
        game.cache_normalized_weights();
        let p = game.current_player();
        let normalized: f32 = game.normalized_weights(p).iter().sum();
        // A node stays when the reference or any profile reaches it (#45):
        // lines a profile takes far more often than the reference (a
        // maniac's raises) must be in the library to be dealt against it.
        let threshold = self.prune_reach_pct * self.root_total[p];
        if !history.is_empty() && 100.0 * normalized < threshold {
            let by_profile = match self.profile_mass.get(history.as_slice()) {
                Some(&m) => 100.0 * m as f32 >= threshold,
                None if self.profiles => {
                    return Err(format!(
                        "node {history:?} not measured by the profile reach pass (a stored \
                         turn is not one of the solver's own turn cards?)"
                    ));
                }
                None => false,
            };
            if !by_profile {
                self.pruned += 1;
                return Ok(Child::Pruned);
            }
            self.kept_for_profiles += 1;
        }
        let idx = self.nodes.len();
        self.nodes.push(None);
        self.histories.push(history.clone());

        let s_actions = game.available_actions();
        let k = s_actions.len();
        let strategy = game.strategy();
        let ev = game.expected_values_detail(p);
        let weights = game.weights(p).to_vec();
        let n = weights.len();
        let bets = game.total_bet_amount();
        let pot = self.job.geometry.pot + (bets[0] + bets[1]) as Chips;

        let mut rows: Vec<(u16, usize)> = (0..n)
            .filter(|&h| {
                let root = self.root_weights[p][h];
                weights[h] > 0.0 && root > 0.0 && weights[h] >= self.min_combo_reach * root
            })
            .map(|h| (self.combos[p][h], h))
            .collect();
        rows.sort_unstable();
        let mut node = RawNode {
            player: p,
            rows: rows.iter().map(|r| r.1).collect(),
            pot,
            actions: Vec::with_capacity(k),
            live: rows.iter().map(|r| r.0).collect(),
            strategy: Vec::with_capacity(rows.len() * k),
            ev: Vec::with_capacity(rows.len() * k),
        };
        for &(_, h) in &rows {
            for a in 0..k {
                node.strategy.push(strategy[a * n + h]);
                node.ev.push(ev[a * n + h] / BB_CHIPS as f32);
            }
        }
        for (a, &sa) in s_actions.iter().enumerate() {
            history.push(a);
            let child = self.visit(game, history)?;
            history.pop();
            node.actions.push(NodeAction {
                action: map_action(sa)?,
                child,
            });
        }
        self.nodes[idx] = Some(node);
        Ok(Child::Node(idx as u32))
    }

    fn turn_chance(
        &mut self,
        game: &mut PostFlopGame,
        history: &mut Vec<usize>,
    ) -> Result<Child, String> {
        let possible = game.possible_cards();
        let mut branches: Vec<(Card, Branch)> = Vec::new();
        for i in 0..52u8 {
            if possible & (1u64 << i) == 0 {
                continue;
            }
            let card = Card::from_index(i).expect("< 52");
            // The smallest card this one maps to under a flop symmetry.
            let (of, perm) = self
                .stabilizer
                .iter()
                .map(|p| (p.card(card), *p))
                .min_by_key(|(c, _)| *c)
                .expect("identity is in the stabilizer");
            if of < card {
                branches.push((card, Branch::Mirror { of, perm }));
                continue;
            }
            history.push(usize::from(i));
            let child = self.visit(game, history)?;
            history.pop();
            game.apply_history(history);
            branches.push((
                card,
                match child {
                    Child::Node(n) => Branch::Node(n),
                    Child::Pruned | Child::End => Branch::Pruned,
                    Child::Chance(_) => return Err("chance after chance".into()),
                },
            ));
        }
        // A mirror of a pruned branch is pruned too.
        for b in 0..branches.len() {
            if let Branch::Mirror { of, .. } = branches[b].1
                && matches!(
                    branches.iter().find(|(c, _)| *c == of),
                    Some((_, Branch::Pruned))
                )
            {
                branches[b].1 = Branch::Pruned;
            }
        }
        self.chance.push(ChanceNode { branches });
        Ok(Child::Chance((self.chance.len() - 1) as u32))
    }
}

fn quantize_range(weights: &[f32]) -> Vec<u8> {
    weights
        .iter()
        .map(|&w| (w.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect()
}

/// Exports a finalized game; with `profiles`, also runs their inline best
/// responses (`super::profiles`) and stores them as v3 sections.
pub fn export(
    game: &mut PostFlopGame,
    job: &Job,
    config: &GameConfig,
    m: &Manifest,
    report: &SolveReport,
    manifest_hash: [u8; 32],
    profiles: &[Profile],
) -> Result<LibraryFile, String> {
    game.back_to_root();
    game.cache_normalized_weights();
    let combos = [0, 1].map(|p| {
        game.private_cards(p)
            .iter()
            .map(|&(a, b)| {
                let h = Hand::new(
                    Card::from_index(a).expect("< 52"),
                    Card::from_index(b).expect("< 52"),
                )
                .expect("distinct");
                h.combo() as u16
            })
            .collect::<Vec<u16>>()
    });
    let root_weights = [game.weights(0).to_vec(), game.weights(1).to_vec()];
    let root_total = [0, 1].map(|p| game.normalized_weights(p).iter().sum::<f32>());
    // With profiles, a reach-only pass per profile and side decides, with
    // the reference reach, which nodes are kept (#45).
    let shape = if profiles.is_empty() {
        None
    } else {
        Some(super::profiles::Shape::of(game)?)
    };
    let mut profile_mass: HashMap<Vec<usize>, f64> = HashMap::new();
    if let Some(shape) = &shape {
        for profile in profiles {
            for bot in 0..2 {
                let pass = super::profiles::Pass {
                    game,
                    shape,
                    profile,
                    bot,
                    responder: super::profiles::Responder::Best,
                    keep: None,
                };
                for (h, m) in pass.reach() {
                    let e = profile_mass.entry(h).or_insert(0.0);
                    *e = e.max(m);
                }
            }
        }
        game.back_to_root();
        game.cache_normalized_weights();
    }
    let mut ex = Ex {
        job,
        prune_reach_pct: m.solve.prune_reach_pct,
        min_combo_reach: m.solve.min_combo_reach,
        root_weights,
        root_total,
        combos,
        stabilizer: job.flop.stabilizer(),
        nodes: Vec::new(),
        histories: Vec::new(),
        chance: Vec::new(),
        profile_mass,
        profiles: !profiles.is_empty(),
        pruned: 0,
        kept_for_profiles: 0,
    };
    let mut history = Vec::new();
    ex.visit(game, &mut history)?;
    game.back_to_root();
    if !profiles.is_empty() {
        eprintln!(
            "    {} nodes, {} kept only for profiles, {} pruned",
            ex.nodes.len(),
            ex.kept_for_profiles,
            ex.pruned
        );
    }

    let raw: Vec<RawNode> = ex
        .nodes
        .into_iter()
        .map(|n| n.ok_or("unfinished node"))
        .collect::<Result<_, _>>()?;
    let sections = if let Some(shape) = &shape {
        let kept: Vec<super::profiles::Kept> = raw
            .iter()
            .zip(&ex.histories)
            .map(|(n, h)| super::profiles::Kept {
                history: h.clone(),
                actor: n.player,
                rows: n.rows.clone(),
                actions: n.actions.len(),
            })
            .collect();
        super::profiles::sections(
            game,
            shape,
            &kept,
            &ex.combos,
            &ex.root_weights,
            m.solve.min_combo_reach,
            profiles,
        )?
    } else {
        Vec::new()
    };
    let max_ev = raw
        .iter()
        .flat_map(|n| n.ev.iter())
        .fold(0.0f32, |m, e| m.max(e.abs()));
    let ev_scale = ev_scale_for(max_ev);
    let positions = [job.geometry.oop, job.geometry.ip];
    let expl_chips = report.exploitability.max(0.0);
    let nodes = raw
        .into_iter()
        .map(|n| {
            let k = n.actions.len();
            Node {
                player: positions[n.player],
                pot: n.pot,
                exploitability_pct: 100.0 * expl_chips / n.pot as f32,
                live: n.live,
                strategy: n.strategy.chunks(k).flat_map(quantize_strategy).collect(),
                ev: n.ev.iter().map(|&e| quantize_ev(e, ev_scale)).collect(),
                actions: n.actions,
            }
        })
        .collect();
    debug_assert_eq!(job.ranges[0].len(), NUM_COMBOS);
    let file = LibraryFile {
        header: Header {
            game_config: config.to_toml(),
            config_key: config.key().0,
            line: job.line.to_string(),
            board: job.flop.cards().to_vec(),
            bet_tree: job.tree.to_toml(),
            domain: Domain::Combos,
            root_ranges: vec![
                (job.geometry.oop, quantize_range(&job.ranges[0])),
                (job.geometry.ip, quantize_range(&job.ranges[1])),
            ],
            exploitability_pct: 100.0 * expl_chips / job.geometry.pot as f32,
            ev_scale,
            solver_id: SOLVER_ID.into(),
            manifest_hash,
        },
        nodes,
        chance: ex.chance,
        profiles: sections,
    };
    file.validate().map_err(|e| e.to_string())?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use postflop_solver::PostFlopGame;
    use taful_core::action::{Action, Line};
    use taful_core::cards::{Card, Hand};
    use taful_core::iso::Flop;
    use taful_core::range::Range;
    use taful_solver::format::{Branch, LibraryFile};
    use taful_solver::postflop::{PostflopFile, Step, geometry};
    use taful_solver::preflop::PreflopChart;

    use super::super::manifest::{Manifest, Tree};
    use super::super::{Job, build_game, solve};
    use super::export;

    fn small_tree() -> Tree {
        Tree {
            flop: ["50%".into(), String::new()],
            turn: ["75%".into(), String::new()],
            river: ["75%".into(), String::new()],
            add_allin_threshold: 1.5,
            force_allin_threshold: 0.15,
            merging_threshold: 0.1,
        }
    }

    fn solved(flop: &str) -> (PostFlopGame, LibraryFile, Flop) {
        solved_with(flop, &[])
    }

    fn solved_with(
        flop: &str,
        profiles: &[taful_core::profile::Profile],
    ) -> (PostFlopGame, LibraryFile, Flop) {
        solved_pruned(flop, profiles, 0.0)
    }

    fn solved_pruned(
        flop: &str,
        profiles: &[taful_core::profile::Profile],
        prune_reach_pct: f32,
    ) -> (PostFlopGame, LibraryFile, Flop) {
        let chart = PreflopChart::decode(include_bytes!(
            "../../../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop.tafl"
        ))
        .unwrap();
        let line: Line = "UTG:f,HJ:f,CO:f,BTN:r250,SB:r950,BB:f,BTN:c"
            .parse()
            .unwrap();
        let g = geometry(chart.config(), &line).unwrap();
        let r = |s: &str| s.parse::<Range>().unwrap().weights().to_vec();
        let tree = small_tree();
        let flop: Flop = flop.parse::<Flop>().unwrap().canonical().0;
        let job = Job {
            tree: &tree,
            line,
            geometry: g,
            ranges: [r("QQ+,AK,T9s,JTs,A5s"), r("JJ-88,AQs,KQs,98s,T8s,AJo")],
            flop,
        };
        let mut m =
            Manifest::parse(include_str!("../../manifests/postflop-6max-100bb.toml")).unwrap();
        m.solve.prune_reach_pct = prune_reach_pct;
        m.solve.min_combo_reach = 0.0;
        m.solve.max_iterations = 60;
        let mut game = build_game(&job).unwrap();
        game.allocate_memory(false);
        let report = solve(&mut game, &m.solve, job.geometry.pot as f32);
        let file = export(
            &mut game,
            &job,
            chart.config(),
            &m,
            &report,
            [0; 32],
            profiles,
        )
        .unwrap();
        (game, file, flop)
    }

    #[test]
    fn export_roundtrips_and_mirrors_match_the_solver() {
        // Two-tone: the two off-suits are interchangeable, so half the turns
        // that are neither the flush suit nor the flop's other suit mirror.
        let (mut game, file, flop) = solved("Td9d6h");
        let bytes = file.encode();
        let pf = PostflopFile::decode(&bytes).unwrap();
        assert_eq!(pf.flop(), flop);
        let ch = &file.chance[0];
        let mirrors = ch
            .branches
            .iter()
            .filter(|(_, b)| matches!(b, Branch::Mirror { .. }))
            .count();
        assert!(mirrors > 5, "{mirrors} mirrored turns");

        // Real play on the canonical flop itself: check-check, then every
        // turn; compare the file's turn strategy with the solver's.
        let root = pf.root(flop).unwrap();
        let first = pf.node(&root).actions[0].action;
        assert_eq!(first, Action::Check);
        let Step::Node(ip) = pf.step(&root, Action::Check) else {
            panic!()
        };
        let Step::Chance(k, perm) = pf.step(&ip, Action::Check) else {
            panic!()
        };
        for &(card, _) in ch.branches.iter() {
            let Step::Node(c) = pf.deal(k, perm, card) else {
                continue;
            };
            game.apply_history(&[0, 0, usize::from(card.index())]);
            game.cache_normalized_weights();
            let p = game.current_player();
            let s = game.strategy();
            let n = game.private_cards(p).len();
            let mut checked = 0;
            for (i, &(a, b)) in game.private_cards(p).iter().enumerate() {
                let h =
                    Hand::new(Card::from_index(a).unwrap(), Card::from_index(b).unwrap()).unwrap();
                if game.weights(p)[i] <= 0.0 {
                    continue;
                }
                let mine = pf.strategy(&c, h).expect("live combo stored");
                let k = mine.len();
                for (x, m) in mine.iter().enumerate() {
                    let theirs = s[x * n + i];
                    assert!(
                        (m - theirs).abs() < 0.02,
                        "{card} {h}: {m} vs {theirs} ({k} actions)"
                    );
                }
                checked += 1;
            }
            assert!(checked > 0);
        }
        // A non-canonical member of the orbit reads the same tree through
        // its suit permutation.
        let swap = taful_core::iso::SuitPerm::swap(
            taful_core::cards::Suit::CLUBS,
            taful_core::cards::Suit::DIAMONDS,
        );
        let real = flop.permuted(&swap);
        let r = pf.root(real).unwrap();
        let h: Hand = "QsQh".parse().unwrap();
        assert_eq!(
            pf.strategy(&r, swap.hand(h)),
            pf.strategy(&root, h),
            "same strategy for the relabeled hand"
        );
    }

    /// #27 acceptance: a build with a profile writes best-response EVs
    /// that read back from the encoded file within quantization error of an
    /// independent best-response pass, at the flop root, after a check and
    /// on every stored turn after check-check, for both sides.
    #[test]
    fn profile_evs_read_back_within_quantization() {
        use std::collections::HashMap;

        use super::super::profiles::{Pass, Recorded, Responder, Shape};

        let station = taful_core::profile::Profile::from_toml(include_str!(
            "../../../../content/bots/calling-station.toml"
        ))
        .unwrap();
        let (mut game, file, flop) = solved_with("Td9d6h", std::slice::from_ref(&station));
        assert_eq!(file.profiles.len(), 1);
        let ev_scale = file.profiles[0].ev_scale;
        let pf = PostflopFile::decode(&file.encode()).unwrap();
        let idx = pf.profile_index("calling-station").unwrap();
        let shape = Shape::of(&mut game).unwrap();
        let passes: Vec<HashMap<Vec<usize>, Recorded>> = (0..2)
            .map(|bot| {
                Pass {
                    game: &game,
                    shape: &shape,
                    profile: &station,
                    bot,
                    responder: Responder::Best,
                    keep: None,
                }
                .run()
                .0
            })
            .collect();

        let root = pf.root(flop).unwrap();
        let Step::Node(ip) = pf.step(&root, Action::Check) else {
            panic!()
        };
        let Step::Chance(k, perm) = pf.step(&ip, Action::Check) else {
            panic!()
        };
        let mut at = vec![(Vec::new(), root), (vec![0], ip)];
        for &(card, ref b) in &file.chance[0].branches {
            if !matches!(b, Branch::Node(_)) {
                continue;
            }
            if let Step::Node(c) = pf.deal(k, perm, card) {
                at.push((vec![0, 0, usize::from(card.index())], c));
            }
        }
        let (mut checked, mut max_diff) = (0, 0.0f32);
        for (hist, c) in &at {
            game.apply_history(hist);
            let p = game.current_player();
            let Some(Recorded::Responder { ev }) = passes[p ^ 1].get(hist) else {
                panic!("{hist:?} not recorded")
            };
            let n = game.private_cards(p).len();
            for (i, &(a, b)) in game.private_cards(p).iter().enumerate() {
                let h =
                    Hand::new(Card::from_index(a).unwrap(), Card::from_index(b).unwrap()).unwrap();
                let Some(stored) = pf.profile_ev(c, idx, h) else {
                    continue;
                };
                for (x, m) in stored.iter().enumerate() {
                    let d = (m - ev[x * n + i]).abs();
                    max_diff = max_diff.max(d);
                    checked += 1;
                }
            }
        }
        game.back_to_root();
        eprintln!("{checked} profile EVs, max difference {max_diff} bb (scale {ev_scale})");
        assert!(checked > 500, "{checked}");
        assert!(
            max_diff <= ev_scale * 0.5 + 1e-4,
            "{max_diff} vs scale {ev_scale}"
        );
    }

    /// #45 acceptance: a line the reference prunes but a profile plays
    /// often (the maniac's bets and raises) is kept, with the profile's
    /// data, and the profile build keeps every node the reference keeps.
    #[test]
    fn a_profile_keeps_the_lines_it_plays_often() {
        use taful_core::action::Action;
        use taful_solver::format::Child;

        let maniac = taful_core::profile::Profile::from_toml(include_str!(
            "../../../../content/bots/maniac.toml"
        ))
        .unwrap();
        let (_, reference, _) = solved_pruned("Td9d6h", &[], 1.0);
        let (_, with, flop) = solved_pruned("Td9d6h", std::slice::from_ref(&maniac), 1.0);
        assert!(reference.nodes.len() < with.nodes.len());

        /// One move from the root: an action or a stored turn card.
        #[derive(Clone, Copy)]
        enum Move {
            Act(Action),
            Deal(Card),
        }
        /// Walks both trees side by side; collects the nodes only `with`
        /// keeps (the first of each such subtree) with the action into it
        /// and the moves from the root.
        struct Rescued {
            node: u32,
            into: Action,
            line: Vec<Move>,
        }
        fn pair(
            r: Child,
            w: Child,
            into: Action,
            line: Vec<Move>,
            files: (&LibraryFile, &LibraryFile),
            out: &mut Vec<Rescued>,
        ) {
            match (r, w) {
                (Child::Node(i), Child::Node(j)) => nodes(Some(i), j, line, files, out),
                (Child::Pruned, Child::Node(j)) => {
                    out.push(Rescued {
                        node: j,
                        into,
                        line,
                    });
                }
                (Child::Chance(i), Child::Chance(j)) => {
                    let (rb, wb) = (
                        &files.0.chance[i as usize].branches,
                        &files.1.chance[j as usize].branches,
                    );
                    assert_eq!(rb.len(), wb.len());
                    for ((rc, r), (wc, w)) in rb.iter().zip(wb) {
                        assert_eq!(rc, wc);
                        let mut line = line.clone();
                        line.push(Move::Deal(*wc));
                        match (r, w) {
                            (Branch::Node(i), Branch::Node(j)) => {
                                nodes(Some(*i), *j, line, files, out);
                            }
                            (Branch::Pruned, Branch::Node(j)) => out.push(Rescued {
                                node: *j,
                                into,
                                line,
                            }),
                            (Branch::Mirror { .. }, Branch::Mirror { .. })
                            | (Branch::Pruned, Branch::Pruned | Branch::Mirror { .. }) => {}
                            (r, w) => panic!("reference kept {r:?}, profile build {w:?}"),
                        }
                    }
                }
                (Child::End, Child::End) | (Child::Pruned, Child::Pruned) => {}
                (r, w) => panic!("reference {r:?}, profile build {w:?}"),
            }
        }
        fn nodes(
            r: Option<u32>,
            w: u32,
            line: Vec<Move>,
            files: (&LibraryFile, &LibraryFile),
            out: &mut Vec<Rescued>,
        ) {
            let r = r.expect("both kept");
            let (rn, wn) = (&files.0.nodes[r as usize], &files.1.nodes[w as usize]);
            assert_eq!(rn.actions.len(), wn.actions.len());
            for (ra, wa) in rn.actions.iter().zip(&wn.actions) {
                assert_eq!(ra.action, wa.action);
                let mut next = line.clone();
                next.push(Move::Act(wa.action));
                pair(ra.child, wa.child, wa.action, next, files, out);
            }
        }
        let mut rescued = Vec::new();
        nodes(Some(0), 0, Vec::new(), (&reference, &with), &mut rescued);
        assert!(!rescued.is_empty(), "nothing kept for the maniac");
        let aggressive = |a: Action| matches!(a, Action::Bet(_) | Action::Raise(_) | Action::AllIn);
        assert!(
            rescued.iter().any(|r| aggressive(r.into)),
            "no line kept after a bet or raise"
        );
        let section = &with.profiles[0];
        for r in &rescued {
            let node = &section.nodes[r.node as usize];
            assert!(
                !node.ev.is_empty() || !node.live.is_empty(),
                "node {} kept without profile data",
                r.node
            );
        }

        // The app reaches each kept node after a bet or raise and reads the
        // profile's data there.
        let pf = PostflopFile::decode(&with.encode()).unwrap();
        let idx = pf.profile_index("maniac").unwrap();
        for r in rescued.iter().filter(|r| aggressive(r.into)) {
            let mut c = pf.root(flop).unwrap();
            let mut chance = None;
            for &m in &r.line {
                let step = match (m, chance.take()) {
                    (Move::Act(a), None) => pf.step(&c, a),
                    (Move::Deal(card), Some((k, perm))) => pf.deal(k, perm, card),
                    _ => panic!("move out of order"),
                };
                match step {
                    Step::Node(next) => c = next,
                    Step::Chance(k, perm) => chance = Some((k, perm)),
                    _ => panic!("left the library on the way to node {}", r.node),
                }
            }
            assert_eq!(c.node, r.node);
            let stored = (0..taful_core::cards::NUM_COMBOS)
                .filter_map(Hand::from_combo)
                .filter(|&h| {
                    pf.profile_ev(&c, idx, h).is_some() || pf.profile_strategy(&c, idx, h).is_some()
                })
                .count();
            assert!(stored > 0, "no profile rows at node {}", r.node);
        }
        eprintln!(
            "{} nodes reference-only, {} with the maniac, {} subtrees kept for it",
            reference.nodes.len(),
            with.nodes.len(),
            rescued.len()
        );
    }

    /// A neutral profile (it plays the reference) keeps exactly the nodes
    /// the reference keeps: the profile reach is measured on the export's
    /// own scale, end to end.
    #[test]
    fn a_neutral_profile_keeps_the_reference_tree() {
        let mut neutral = taful_core::profile::Profile::from_toml(include_str!(
            "../../../../content/bots/tag.toml"
        ))
        .unwrap();
        neutral.postflop = taful_core::profile::PostflopDeviation {
            fold_to_cbet: 0.0,
            fold_to_raise: 0.0,
            bluff_value_shift: 0.0,
            aggression: 0.0,
        };
        neutral.sizing.prefer = taful_core::profile::Sizing::Reference;
        let (_, reference, _) = solved_pruned("Td9d6h", &[], 1.0);
        let (_, with, _) = solved_pruned("Td9d6h", std::slice::from_ref(&neutral), 1.0);
        assert_eq!(reference.nodes.len(), with.nodes.len());
        for (r, w) in reference.nodes.iter().zip(&with.nodes) {
            assert_eq!(r.actions, w.actions);
            assert_eq!(r.live, w.live);
        }
        assert_eq!(reference.chance, with.chance);
    }
}
