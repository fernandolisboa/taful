//! Validation of Taful's river solver against postflop-solver (spec §7.3,
//! D11): on fixed river spots both must build the same tree and agree on
//! per-hand root EVs and range EVs within tolerance.

#[cfg(test)]
mod tests {
    use postflop_solver::{
        Action as SAction, ActionTree, BoardState, CardConfig, PostFlopGame, TreeConfig,
        compute_exploitability, finalize, solve_step,
    };
    use taful_core::action::{Action, Line};
    use taful_core::cards::{Card, Hand, parse_cards};
    use taful_core::config::Rake;
    use taful_solver::postflop::line_range;
    use taful_solver::preflop::PreflopChart;
    use taful_solver::river::{
        DcfrRiverSolver, RiverSpot, RiverTree, SolveOptions, SubgameSolver, parse_sizes,
    };

    use crate::postflop::{bet_sizes, solver_range};

    fn chart() -> PreflopChart {
        PreflopChart::decode(include_bytes!(
            "../../../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop.tafl"
        ))
        .unwrap()
    }

    struct Case {
        board: &'static str,
        line: &'static str,
        pot: i32,
        stack: i32,
        bets: &'static str,
        raises: &'static str,
    }

    fn run(case: &Case) {
        let chart = chart();
        let line: Line = case.line.parse().unwrap();
        let g = taful_solver::postflop::geometry(chart.config(), &line).unwrap();
        let ranges = [
            line_range(&chart, &line, g.oop).unwrap(),
            line_range(&chart, &line, g.ip).unwrap(),
        ];
        let board = parse_cards(case.board).unwrap();
        let sizes = [case.bets.to_string(), case.raises.to_string()];

        // postflop-solver.
        let tree = TreeConfig {
            initial_state: BoardState::River,
            starting_pot: case.pot,
            effective_stack: case.stack,
            rake_rate: 0.0,
            rake_cap: 0.0,
            flop_bet_sizes: Default::default(),
            turn_bet_sizes: Default::default(),
            river_bet_sizes: [bet_sizes(&sizes).unwrap(), bet_sizes(&sizes).unwrap()],
            turn_donk_sizes: None,
            river_donk_sizes: None,
            add_allin_threshold: 1.5,
            force_allin_threshold: 0.15,
            merging_threshold: 0.1,
        };
        let cards = CardConfig {
            range: [solver_range(&ranges[0]), solver_range(&ranges[1])],
            flop: [board[0].index(), board[1].index(), board[2].index()],
            turn: board[3].index(),
            river: board[4].index(),
        };
        let mut game = PostFlopGame::with_config(cards, ActionTree::new(tree).unwrap()).unwrap();
        game.allocate_memory(false);
        let target = case.pot as f32 * 0.001;
        for i in 0..3000 {
            solve_step(&game, i);
            if (i + 1) % 20 == 0 && compute_exploitability(&game) <= target {
                break;
            }
        }
        finalize(&mut game);
        game.cache_normalized_weights();

        // Taful.
        let spot = RiverSpot {
            board: board.clone().try_into().unwrap(),
            ranges: ranges.clone(),
            pot: case.pot as u64,
            stack: case.stack as u64,
            tree: RiverTree {
                bets: parse_sizes(case.bets, false).unwrap(),
                raises: parse_sizes(case.raises, true).unwrap(),
                add_allin_threshold: 1.5,
                force_allin_threshold: 0.15,
                merging_threshold: 0.1,
            },
            rake: Rake::ZERO,
        };
        let sol = DcfrRiverSolver
            .solve(
                &spot,
                &SolveOptions {
                    target_pct: 0.1,
                    max_iterations: 3000,
                    check_every: 20,
                },
            )
            .unwrap();
        assert!(
            sol.exploitability_pct() <= 0.1,
            "taful expl {}",
            sol.exploitability_pct()
        );

        // Same root actions.
        let theirs: Vec<Action> = game
            .available_actions()
            .into_iter()
            .map(|a| match a {
                SAction::Fold => Action::Fold,
                SAction::Check => Action::Check,
                SAction::Call => Action::Call,
                SAction::Bet(x) => Action::Bet(x as u64),
                SAction::Raise(x) => Action::Raise(x as u64),
                SAction::AllIn(_) => Action::AllIn,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(sol.nodes[0].actions, theirs, "root actions");

        // Per-hand root EVs (OOP and IP) and range EVs.
        let pot = case.pot as f32;
        for p in 0..2 {
            let ev = game.expected_values(p);
            let w = game.normalized_weights(p);
            let mut range_theirs = 0.0;
            let mut wsum = 0.0;
            let mut max_diff = 0.0f32;
            let mut sum_diff = 0.0f32;
            let mut n = 0;
            for (i, &(a, b)) in game.private_cards(p).iter().enumerate() {
                if w[i] <= 0.0 {
                    continue;
                }
                range_theirs += ev[i] * w[i];
                wsum += w[i];
                let h =
                    Hand::new(Card::from_index(a).unwrap(), Card::from_index(b).unwrap()).unwrap();
                let Some(hi) = sol.hand_index(p, h) else {
                    continue;
                };
                // Root EV of a hand = its strategy-weighted action EVs (OOP)
                // or, for IP, not at the root: compare through the range EV.
                if p == 0 {
                    let st = sol.strategy(0, h).unwrap();
                    let evs = sol.ev(0, h).unwrap();
                    let mine: f32 = st.iter().zip(&evs).map(|(s, e)| s * e).sum();
                    let d = (mine - ev[i]).abs();
                    max_diff = max_diff.max(d);
                    sum_diff += d;
                    n += 1;
                }
                let _ = hi;
            }
            let range_theirs = range_theirs / wsum;
            let range_mine = sol.root_ev[p];
            assert!(
                (range_theirs - range_mine).abs() < 0.005 * pot,
                "player {p} range EV: postflop-solver {range_theirs:.1}, taful {range_mine:.1}"
            );
            if p == 0 {
                let mean = sum_diff / n as f32;
                assert!(mean < 0.01 * pot, "mean per-hand EV diff {mean:.1} chips");
                assert!(
                    max_diff < 0.06 * pot,
                    "max per-hand EV diff {max_diff:.1} chips"
                );
            }
        }
    }

    #[test]
    fn river_matches_postflop_solver_srp() {
        run(&Case {
            board: "Td9d6hQc2s",
            line: "UTG:f,HJ:f,CO:f,BTN:r250,SB:f,BB:c",
            pot: 1200,
            stack: 9150,
            bets: "50%, 100%, a",
            raises: "3x",
        });
    }

    #[test]
    fn river_matches_postflop_solver_3bp() {
        run(&Case {
            board: "AcKdQh7s7d",
            line: "UTG:f,HJ:f,CO:f,BTN:r250,SB:r950,BB:f,BTN:c",
            pot: 6000,
            stack: 7000,
            bets: "33%, 75%, 125%, a",
            raises: "2.5x, 4x",
        });
    }
}
