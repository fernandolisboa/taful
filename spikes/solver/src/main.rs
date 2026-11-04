//! Spike §9.2: solve 6-max 100bb flops (BTN vs BB SRP, or SB vs BTN 3-bet pot with SPOT=3bp) with
//! postflop-solver and measure tree size, solve time, peak memory and the
//! serialized size of the result.
//!
//! Usage: spike-solver <board> <tree: small|medium|large> [target_pct_pot] [compress: 0|1] [estimate-only: 0|1]
//! A 3-, 4- or 5-card board starts the tree on the flop, turn or river.

use postflop_solver::*;
use std::time::Instant;

// Approximate, hand-written ranges (not copied from any proprietary source). Chip unit: 1bb = 100.
struct Spot {
    pot: i32,
    stack: i32,
    oop: &'static str,
    ip: &'static str,
}

// SRP: BTN opens 2.5bb, BB calls, SB folds -> pot 5.5bb, 97.5bb behind.
const SRP_BTN_BB: Spot = Spot {
    pot: 550,
    stack: 9750,
    oop: "JJ-22,AJs-A2s,KJs-K2s,QJs-Q4s,J6s+,T6s+,96s+,85s+,74s+,63s+,53s+,43s,AJo-A2o,KQo-K7o,Q8o+,J8o+,T8o+,98o,87o",
    ip: "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,75s+,64s+,54s,A2o+,K8o+,Q9o+,J9o+,T9o",
};

// 3-bet pot: BTN opens 2.5bb, SB 3-bets to 11bb, BB folds, BTN calls -> pot 23bb, 89bb behind.
const TBP_SB_BTN: Spot = Spot {
    pot: 2300,
    stack: 8900,
    oop: "TT+,AJs+,KJs+,QJs,A5s-A3s,76s,65s,AQo+",
    ip: "99-44,ATs-A6s,KTs+,QTs+,JTs,T9s,98s,87s,76s,AJo,KQo",
};

fn sizes(bet: &str, raise: &str) -> BetSizeOptions {
    BetSizeOptions::try_from((bet, raise)).unwrap()
}

fn tree(kind: &str, initial_state: BoardState, pot: i32, stack: i32) -> TreeConfig {
    // small: one size per street. medium: two sizes + all-in. large: three sizes + all-in, two raise sizes.
    let (flop, turn, river) = match kind {
        "small" => (sizes("33%", "3x"), sizes("75%", "3x"), sizes("75%", "3x")),
        "medium" => (
            sizes("33%, 75%", "3x"),
            sizes("50%, 100%, a", "3x"),
            sizes("50%, 100%, a", "3x"),
        ),
        "large" => (
            sizes("25%, 50%, 100%", "2.5x, 4x"),
            sizes("33%, 75%, 125%, a", "2.5x, 4x"),
            sizes("33%, 75%, 125%, a", "2.5x, 4x"),
        ),
        _ => panic!("unknown tree kind"),
    };
    TreeConfig {
        initial_state,
        starting_pot: pot,
        effective_stack: stack,
        rake_rate: 0.0,
        rake_cap: 0.0,
        flop_bet_sizes: [flop.clone(), flop],
        turn_bet_sizes: [turn.clone(), turn],
        river_bet_sizes: [river.clone(), river],
        turn_donk_sizes: None,
        river_donk_sizes: None,
        add_allin_threshold: 1.5,
        force_allin_threshold: 0.15,
        merging_threshold: 0.1,
    }
}

fn peak_rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
        })
        .map_or(f64::NAN, |kb| kb / 1024.0)
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flop = args.get(1).map_or("Ks7d2c", String::as_str);
    let kind = args.get(2).map_or("medium", String::as_str);
    let spot = if std::env::var("SPOT").as_deref() == Ok("3bp") {
        TBP_SB_BTN
    } else {
        SRP_BTN_BB
    };
    let (pot, stack) = (spot.pot, spot.stack);
    let target_pct: f32 = args.get(3).map_or(0.5, |s| s.parse().unwrap());
    let compress = args.get(4).is_some_and(|s| s == "1");
    let estimate_only = args.get(5).is_some_and(|s| s == "1");
    let (flop_str, turn, river, state) = match flop.len() {
        6 => (flop, NOT_DEALT, NOT_DEALT, BoardState::Flop),
        8 => (
            &flop[..6],
            card_from_str(&flop[6..8]).unwrap(),
            NOT_DEALT,
            BoardState::Turn,
        ),
        10 => (
            &flop[..6],
            card_from_str(&flop[6..8]).unwrap(),
            card_from_str(&flop[8..10]).unwrap(),
            BoardState::River,
        ),
        _ => panic!("board must have 3, 4 or 5 cards"),
    };

    let card_config = CardConfig {
        range: [spot.oop.parse().unwrap(), spot.ip.parse().unwrap()], // [OOP, IP]
        flop: flop_from_str(flop_str).unwrap(),
        turn,
        river,
    };

    let t0 = Instant::now();
    let action_tree = ActionTree::new(tree(kind, state, pot, stack)).unwrap();
    let mut game = PostFlopGame::with_config(card_config, action_tree).unwrap();
    let (mem, mem_c) = game.memory_usage();
    if estimate_only {
        println!(
            "board={flop} tree={kind} | est_mem={:.0}MB est_mem_compressed={:.0}MB",
            mb(mem),
            mb(mem_c)
        );
        return;
    }
    game.allocate_memory(compress);
    let build = t0.elapsed();

    let target = pot as f32 * target_pct / 100.0;
    let t1 = Instant::now();
    let mut iters = 0;
    let mut expl = f32::INFINITY;
    while iters < 2000 {
        solve_step(&game, iters);
        iters += 1;
        if iters % 10 == 0 {
            expl = compute_exploitability(&game);
            if expl <= target {
                break;
            }
        }
    }
    finalize(&mut game);
    let solve_time = t1.elapsed();

    game.cache_normalized_weights();
    let oop_eq = compute_average(&game.equity(0), game.normalized_weights(0));
    let oop_ev = compute_average(&game.expected_values(0), game.normalized_weights(0));
    let root_actions = game.available_actions();
    let strategy = game.strategy();
    let n = game.private_cards(0).len();
    let freqs: Vec<String> = root_actions
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let w = game.normalized_weights(0);
            let f: f32 =
                (0..n).map(|h| strategy[i * n + h] * w[h]).sum::<f32>() / w.iter().sum::<f32>();
            format!("{a:?}={:.1}%", 100.0 * f)
        })
        .collect();

    let dir = std::env::temp_dir();
    let mut sizes_out = Vec::new();
    for (label, mode, level) in [
        ("full_zstd", BoardState::River, Some(3)),
        ("turn_zstd", BoardState::Turn, Some(3)),
        ("flop_zstd", BoardState::Flop, Some(3)),
    ] {
        // Loading a second copy of a multi-GB tree would double peak memory.
        if mode < state || (mode == BoardState::River && mem_c > 4 << 30) {
            continue;
        }
        game.set_target_storage_mode(mode).unwrap();
        let path = dir.join(format!("spike-{flop}-{kind}-{label}.bin"));
        let ts = Instant::now();
        save_data_to_file(&game, flop, &path, level).unwrap();
        let save_t = ts.elapsed();
        let size = std::fs::metadata(&path).unwrap().len();
        let tl = Instant::now();
        let (_g, _m): (PostFlopGame, String) = load_data_from_file(&path, None).unwrap();
        let load_t = tl.elapsed();
        std::fs::remove_file(&path).unwrap();
        sizes_out.push(format!(
            "{label}={:.1}MB(save {:.2}s, load {:.2}s)",
            mb(size),
            save_t.as_secs_f64(),
            load_t.as_secs_f64()
        ));
    }

    println!(
        "board={flop} tree={kind} compress={compress} | est_mem={:.0}MB est_mem_compressed={:.0}MB | build={:.2}s solve={:.1}s iters={iters} expl={:.3}bb ({:.2}% pot) | peak_rss={:.0}MB | OOP eq={:.1}% ev={:.2}bb | root: {} | files: {}",
        mb(mem),
        mb(mem_c),
        build.as_secs_f64(),
        solve_time.as_secs_f64(),
        expl / 100.0,
        100.0 * expl / pot as f32,
        peak_rss_mb(),
        100.0 * oop_eq,
        oop_ev / 100.0,
        freqs.join(" "),
        sizes_out.join(" "),
    );
}
