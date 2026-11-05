use proptest::prelude::*;

use super::*;
use crate::cards::parse_cards;
use crate::config::{Blinds, Format, Rake, Stacks};
use crate::eval::RsPokerEvaluator;

const EVAL: RsPokerEvaluator = RsPokerEvaluator;

fn hand(s: &str) -> Hand {
    s.parse().unwrap()
}

fn cfg(stacks: Vec<Chips>) -> GameConfig {
    let n = stacks.len() as u8;
    GameConfig {
        table_size: n,
        format: Format::Cash,
        blinds: Blinds {
            sb: 50,
            bb: 100,
            ante: 0,
            ante_kind: AnteKind::None,
        },
        stacks: Stacks::PerSeat { chips: stacks },
        rake: Rake::ZERO,
    }
}

fn holes(hs: &[&str]) -> Vec<Option<Hand>> {
    hs.iter().map(|h| Some(hand(h))).collect()
}

fn play(mut s: GameState, acts: &[Action]) -> GameState {
    for &a in acts {
        s = s
            .apply(a)
            .unwrap_or_else(|e| panic!("{a} at {:?}: {e}", s.phase()));
    }
    s
}

/// Deal `board` street by street, checking down any betting in between.
fn run_out(mut s: GameState, board: &str) -> GameState {
    let cards = parse_cards(board).unwrap();
    loop {
        s = match s.phase() {
            Phase::NeedsBoard(st) => s
                .deal_board(&cards[s.board().len()..st.board_len()])
                .unwrap(),
            Phase::ToAct(_) => s.apply(Action::Check).unwrap(),
            Phase::Finished => return s,
        };
    }
}

fn conserved(s: &GameState, st: &Settlement) {
    let before: Chips = s.initial_stacks().iter().sum();
    let after: Chips = st.final_stacks.iter().sum();
    assert_eq!(before, after + st.rake);
}

#[test]
fn six_max_blinds_and_order() {
    let s = GameState::new(GameConfig::cash(6, 100), 0, 1).unwrap();
    assert_eq!(s.position(0), Position::Btn);
    assert_eq!(s.players()[1].street_commit, 50);
    assert_eq!(s.players()[2].street_commit, 100);
    assert_eq!(s.to_act(), Some(3)); // UTG
    assert_eq!(s.position(3), Position::Utg);
    assert_eq!(s.pot(), 150);
}

#[test]
fn heads_up_button_posts_sb_and_acts_first() {
    let s = GameState::new(GameConfig::cash(2, 100), 1, 1).unwrap();
    assert_eq!(s.players()[1].street_commit, 50);
    assert_eq!(s.players()[0].street_commit, 100);
    assert_eq!(s.to_act(), Some(1));
    // Limp, BB checks: flop, BB acts first.
    let s = play(s, &[Action::Call, Action::Check]);
    assert_eq!(s.phase(), Phase::NeedsBoard(Street::Flop));
    let s = s.deal_next().unwrap();
    assert_eq!(s.to_act(), Some(0));
    assert_eq!(s.history().to_string(), "BTN:c,BB:x/");
}

#[test]
fn bb_gets_option_after_limps() {
    let s = GameState::new(GameConfig::cash(3, 100), 0, 1).unwrap();
    let s = play(s, &[Action::Call, Action::Call]);
    assert_eq!(s.to_act(), Some(2));
    let b = s.bounds().unwrap();
    assert_eq!(b.to_call, 0);
    assert_eq!(b.min_to, Some(200));
}

#[test]
fn fold_around_returns_uncalled_bet() {
    let s = GameState::new(GameConfig::cash(6, 100), 0, 1).unwrap();
    let s = play(
        s,
        &[
            Action::Fold,
            Action::Fold,
            Action::Fold,
            Action::Raise(250),
            Action::Fold,
            Action::Fold,
        ],
    );
    assert_eq!(s.phase(), Phase::Finished);
    let st = s.settle(&EVAL).unwrap();
    assert!(!st.showdown);
    assert_eq!(st.net[0], 150);
    assert_eq!(st.net[1], -50);
    assert_eq!(st.net[2], -100);
    conserved(&s, &st);
    assert_eq!(
        s.history().to_string(),
        "UTG:f,HJ:f,CO:f,BTN:r250,SB:f,BB:f"
    );
}

#[test]
fn min_raise_and_illegal_actions() {
    let s = GameState::new(GameConfig::cash(6, 100), 0, 1).unwrap();
    assert!(s.apply(Action::Check).is_err());
    assert!(s.apply(Action::Bet(300)).is_err());
    assert!(s.apply(Action::Raise(150)).is_err());
    let s = s.apply(Action::Raise(300)).unwrap(); // +200
    let b = s.bounds().unwrap();
    assert_eq!(b.min_to, Some(500));
    assert!(s.apply(Action::Raise(450)).is_err());
    assert!(s.apply(Action::Raise(10_001)).is_err());
    let s = s.apply(Action::Raise(10_000)).unwrap();
    assert_eq!(s.history().streets[0][1].1, Action::AllIn);
}

#[test]
fn short_all_in_does_not_reopen() {
    // A (seat 3) has 10,000; B (seat 4) 380; C (seat 5) 10,000. Button 0.
    let mut stacks = vec![10_000; 6];
    stacks[4] = 380;
    let s = GameState::new(cfg(stacks), 0, 1).unwrap();
    // UTG opens 300 (+200), HJ all-in 380 (+80, incomplete), CO folds, BTN, SB, BB fold.
    let s = play(
        s,
        &[
            Action::Raise(300),
            Action::AllIn,
            Action::Fold,
            Action::Fold,
            Action::Fold,
            Action::Fold,
        ],
    );
    // Back to UTG: may only call or fold.
    assert_eq!(s.to_act(), Some(3));
    let b = s.bounds().unwrap();
    assert_eq!(b.to_call, 80);
    assert_eq!(b.min_to, None);
    assert!(s.apply(Action::Raise(1000)).is_err());
    assert!(s.apply(Action::AllIn).is_err());
    let s = s.apply(Action::Call).unwrap();
    assert_eq!(s.phase(), Phase::NeedsBoard(Street::Flop));
}

#[test]
fn short_all_ins_that_add_up_to_a_full_raise_reopen() {
    let mut stacks = vec![10_000; 6];
    stacks[4] = 450; // +150 over a 300 open: incomplete (needs +200)
    stacks[5] = 520; // +70 more: cumulative +220 over 300 ≥ 200 → reopens
    let s = GameState::new(cfg(stacks), 0, 1).unwrap();
    let s = play(
        s,
        &[
            Action::Raise(300),
            Action::AllIn,
            Action::AllIn,
            Action::Fold,
            Action::Fold,
            Action::Fold,
        ],
    );
    assert_eq!(s.to_act(), Some(3));
    let b = s.bounds().unwrap();
    assert_eq!(b.min_to, Some(520 + 200));
}

#[test]
fn three_way_side_pots() {
    // Seats: 0 BTN (1000), 1 SB (3000), 2 BB (5000). All in preflop.
    let s = GameState::new_with_holes(
        cfg(vec![1000, 3000, 5000]),
        0,
        1,
        &holes(&["AsAh", "KsKh", "QsQh"]),
    )
    .unwrap();
    let s = play(s, &[Action::AllIn, Action::AllIn, Action::Call]);
    let s = run_out(s, "2c7d9cJd3h");
    assert_eq!(s.phase(), Phase::Finished);
    let st = s.settle(&EVAL).unwrap();
    assert!(st.showdown);
    assert_eq!(st.pots.len(), 2); // BB's extra 2000 is uncalled and returned
    assert_eq!(
        (st.pots[0].amount, st.pots[0].winners.clone()),
        (3000, vec![0])
    );
    assert_eq!(
        (st.pots[1].amount, st.pots[1].winners.clone()),
        (4000, vec![1])
    );
    assert_eq!(st.final_stacks, vec![3000, 4000, 2000]);
    conserved(&s, &st);
}

#[test]
fn side_pot_with_folded_money() {
    // 4 players; seat 3 (UTG? no: 4-max = BTN SB BB CO) is short.
    let s = GameState::new_with_holes(
        cfg(vec![5000, 5000, 5000, 600]),
        0,
        1,
        &holes(&["2c2d", "KsKh", "QsQh", "AsAh"]),
    )
    .unwrap();
    // CO (3) all-in 600, BTN (0) raises to 2000, SB folds, BB calls, BTN... round closes.
    let s = play(
        s,
        &[
            Action::AllIn,
            Action::Raise(2000),
            Action::Fold,
            Action::Call,
        ],
    );
    let s = run_out(s, "3c7d9cJd8h");
    let st = s.settle(&EVAL).unwrap();
    // Main: 600 × 3 + SB 50 = 1850 → AA. Side: 1400 × 2 = 2800 → QQ.
    assert_eq!(st.pots[0].amount, 1850);
    assert_eq!(st.pots[0].winners, vec![3]);
    assert_eq!(st.pots[1].amount, 2800);
    assert_eq!(st.pots[1].winners, vec![2]);
    conserved(&s, &st);
}

#[test]
fn split_pot_odd_chip_goes_left_of_button() {
    // Blinds 5/10 with a per-player ante of 1 make an odd pot.
    let mut c = cfg(vec![1000; 3]);
    c.blinds = Blinds {
        sb: 5,
        bb: 10,
        ante: 1,
        ante_kind: AnteKind::PerPlayer,
    };
    let s = GameState::new_with_holes(c, 0, 1, &holes(&["2c3d", "2d3c", "4h5h"])).unwrap();
    // BTN calls, SB calls, BB checks → pot 33; the board plays, so all three split.
    let s = play(s, &[Action::Call, Action::Call, Action::Check]);
    let s = run_out(s, "AsKsQsJsTs");
    let st = s.settle(&EVAL).unwrap();
    assert_eq!(st.pots[0].amount, 33);
    assert_eq!(st.pots[0].winners, vec![1, 2, 0]);
    assert_eq!(st.final_stacks, vec![1000, 1000, 1000]);
    // Now a two-way split of an odd pot: fold the BB.
    let mut c = cfg(vec![1000; 3]);
    c.blinds = Blinds {
        sb: 5,
        bb: 10,
        ante: 1,
        ante_kind: AnteKind::PerPlayer,
    };
    let s = GameState::new_with_holes(c, 0, 1, &holes(&["2c3d", "2d3c", "4h5h"])).unwrap();
    let s = play(s, &[Action::Raise(20), Action::Call, Action::Fold]);
    let s = run_out(s, "AsKsQsJsTs");
    let st = s.settle(&EVAL).unwrap();
    // 20 + 20 + 10 + 3 antes = 53 → 27 to SB (left of button), 26 to BTN.
    assert_eq!(st.pots[0].amount, 53);
    assert_eq!(st.pots[0].winners, vec![1, 0]);
    assert_eq!(st.final_stacks[1] - (1000 - 21), 27);
    assert_eq!(st.final_stacks[0] - (1000 - 21), 26);
    conserved(&s, &st);
}

#[test]
fn rake_and_no_flop_no_drop() {
    let mut c = GameConfig::cash(2, 100);
    c.rake = Rake {
        rate_bps: 500,
        cap: Some(300),
        no_flop_no_drop: true,
    };
    // Preflop fold: no rake.
    let s = GameState::new(c.clone(), 0, 3).unwrap();
    let s = play(s, &[Action::Raise(300), Action::Fold]);
    assert_eq!(s.settle(&EVAL).unwrap().rake, 0);
    // All-in preflop: 20,000 pot; 5% capped at 300. Flop is dealt, so rake applies.
    let s = GameState::new(c, 0, 3).unwrap();
    let s = play(s, &[Action::AllIn, Action::Call]);
    let mut s = s;
    while let Phase::NeedsBoard(_) = s.phase() {
        s = s.deal_next().unwrap();
    }
    let st = s.settle(&EVAL).unwrap();
    assert_eq!(st.rake, 300);
    conserved(&s, &st);
}

#[test]
fn big_blind_ante_is_dead_money() {
    let mut c = cfg(vec![10_000; 3]);
    c.blinds = Blinds {
        sb: 50,
        bb: 100,
        ante: 100,
        ante_kind: AnteKind::BigBlind,
    };
    let s = GameState::new(c, 0, 1).unwrap();
    assert_eq!(s.pot(), 250);
    let s = play(s, &[Action::Fold, Action::Fold]);
    let st = s.settle(&EVAL).unwrap();
    // BB wins SB's 50 + its own ante back.
    assert_eq!(st.net, vec![0, -50, 50]);
}

#[test]
fn replay_is_deterministic() {
    let run = |seed| {
        let mut s = GameState::new(GameConfig::cash(6, 100), 2, seed).unwrap();
        let tree = BetTree::default_srp();
        let mut i = 0;
        while s.phase() != Phase::Finished {
            s = match s.phase() {
                Phase::NeedsBoard(_) => s.deal_next().unwrap(),
                _ => {
                    let la = legal_actions(&s, &tree);
                    i += 1;
                    s.apply(la[i % la.len()]).unwrap()
                }
            };
        }
        (
            s.history().to_string(),
            s.board().to_vec(),
            s.settle(&EVAL).unwrap(),
        )
    };
    assert_eq!(run(42), run(42));
    assert_ne!(run(42).1, run(43).1);
}

#[test]
fn legal_actions_from_tree() {
    let tree = BetTree::default_srp();
    let s = GameState::new(GameConfig::cash(6, 100), 0, 1).unwrap();
    assert_eq!(
        legal_actions(&s, &tree),
        vec![Action::Fold, Action::Call, Action::Raise(250)]
    );
    let s = play(
        s,
        &[
            Action::Fold,
            Action::Fold,
            Action::Fold,
            Action::Raise(250),
            Action::Fold,
        ],
    );
    // BB vs open: 3x = 750.
    assert_eq!(
        legal_actions(&s, &tree),
        vec![Action::Fold, Action::Call, Action::Raise(750)]
    );
    let s = play(s, &[Action::Call]).deal_next().unwrap();
    // Flop pot 550: 33% = 182 (rounded), 75% = 413.
    assert_eq!(s.pot(), 550);
    assert_eq!(
        legal_actions(&s, &tree),
        vec![Action::Check, Action::Bet(182), Action::Bet(413)]
    );
    let s = s.apply(Action::Bet(182)).unwrap();
    // Raise 3x = 546; allin offered.
    assert_eq!(
        legal_actions(&s, &tree),
        vec![
            Action::Fold,
            Action::Call,
            Action::Raise(546),
            Action::AllIn
        ]
    );
}

#[test]
fn every_position_layout_runs() {
    for n in 2..=9u8 {
        for button in 0..usize::from(n) {
            let s = GameState::new(GameConfig::cash(n, 100), button, 7).unwrap();
            assert!(s.to_act().is_some());
        }
    }
}

// ---- property tests -------------------------------------------------------------

fn arb_config() -> impl Strategy<Value = (GameConfig, usize)> {
    (2u8..=9, 0u8..3, 0u32..800, any::<bool>())
        .prop_flat_map(|(n, ante_kind, rake_bps, nfnd)| {
            (
                Just(n),
                prop::collection::vec(1u64..30_000, usize::from(n)),
                1u64..200,
                Just(ante_kind),
                0u64..60,
                Just(rake_bps),
                Just(nfnd),
                0..usize::from(n),
            )
        })
        .prop_map(|(n, stacks, bb, ante_kind, ante, rake_bps, nfnd, button)| {
            let ante_kind = match ante_kind {
                0 => AnteKind::None,
                1 => AnteKind::PerPlayer,
                _ => AnteKind::BigBlind,
            };
            let c = GameConfig {
                table_size: n,
                format: Format::Cash,
                blinds: Blinds {
                    sb: bb / 2,
                    bb,
                    ante: if ante_kind == AnteKind::None { 0 } else { ante },
                    ante_kind,
                },
                stacks: Stacks::PerSeat { chips: stacks },
                rake: Rake {
                    rate_bps: rake_bps,
                    cap: Some(500),
                    no_flop_no_drop: nfnd,
                },
            };
            (c, button)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(if cfg!(debug_assertions) { 400 } else { 5000 }))]

    /// Random valid configs and random legal play: chips are conserved, stacks
    /// never go negative, the hand terminates, and every pot is consistent.
    #[test]
    fn engine_invariants((config, button) in arb_config(), seed in any::<u64>(), choices in prop::collection::vec(any::<u32>(), 200)) {
        let total: Chips = (0..usize::from(config.table_size)).map(|s| config.stack(s)).sum();
        let mut s = GameState::new(config, button, seed).unwrap();
        let mut steps = 0;
        let mut k = 0;
        while s.phase() != Phase::Finished {
            steps += 1;
            prop_assert!(steps < 500, "no termination");
            // Chips in play are conserved at every step.
            let in_play: Chips = s.players().iter().map(|p| p.stack).sum::<Chips>() + s.pot();
            prop_assert_eq!(in_play, total);
            s = match s.phase() {
                Phase::NeedsBoard(_) => s.deal_next().unwrap(),
                Phase::ToAct(seat) => {
                    let b = s.bounds().unwrap();
                    prop_assert_eq!(b.seat, seat);
                    let c = choices[k % choices.len()];
                    k += 1;
                    // Mix tree actions with arbitrary amounts inside the bounds.
                    let action = match (c % 6, b.min_to, b.max_to) {
                        (0, _, _) if b.to_call > 0 => Action::Fold,
                        (1 | 2, _, _) => if b.to_call > 0 { Action::Call } else { Action::Check },
                        (3, Some(lo), Some(hi)) => {
                            let to = lo + u64::from(c / 6) % (hi - lo + 1);
                            if b.is_bet { Action::Bet(to) } else { Action::Raise(to) }
                        }
                        (4, Some(_), Some(_)) => Action::AllIn,
                        _ => {
                            let la = legal_actions(&s, &BetTree::default_srp());
                            la[(c as usize / 6) % la.len()]
                        }
                    };
                    match s.apply(action) {
                        Ok(n) => n,
                        Err(e) => return Err(TestCaseError::fail(format!("{action} rejected: {e}"))),
                    }
                }
                Phase::Finished => unreachable!(),
            };
        }
        let st = s.settle(&EVAL).unwrap();
        prop_assert_eq!(st.final_stacks.iter().sum::<Chips>() + st.rake, total);
        let pots: Chips = st.pots.iter().map(|p| p.amount).sum();
        prop_assert_eq!(pots, s.pot());
        for p in &st.pots {
            prop_assert!(!p.winners.is_empty());
            prop_assert!(p.winners.iter().all(|w| p.eligible.contains(w)));
            prop_assert!(p.eligible.iter().all(|&e| !s.players()[e].folded));
        }
        // Each pot's eligible set shrinks as pots go up.
        for w in st.pots.windows(2) {
            prop_assert!(w[1].eligible.iter().all(|e| w[0].eligible.contains(e)));
        }
    }

    /// Every action `legal_actions` offers is accepted by `apply`.
    #[test]
    fn legal_actions_are_accepted(seed in any::<u64>(), n in 2u8..=9, picks in prop::collection::vec(any::<u16>(), 60)) {
        let tree = BetTree::default_srp();
        let mut s = GameState::new(GameConfig::cash(n, 100), 0, seed).unwrap();
        let mut i = 0;
        while s.phase() != Phase::Finished && i < picks.len() {
            s = match s.phase() {
                Phase::NeedsBoard(_) => s.deal_next().unwrap(),
                _ => {
                    let la = legal_actions(&s, &tree);
                    for &a in &la {
                        prop_assert!(s.apply(a).is_ok(), "{} offered but rejected", a);
                    }
                    let a = la[usize::from(picks[i]) % la.len()];
                    i += 1;
                    s.apply(a).unwrap()
                }
            };
        }
    }
}
