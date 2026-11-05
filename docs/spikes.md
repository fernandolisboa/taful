# Taful — Spike Results

Results of the early spikes in [brief.md](brief.md) §9. Spike code lives in [`spikes/`](../spikes) and is throwaway: it is not part of the workspace and is not reused by the app.

---

## §9.2 Solver: `postflop-solver`

**Date:** 2026-11-04 · **Code:** [`spikes/solver`](../spikes/solver) · **Verdict: passes, with constraints.** The solver works and produces trustworthy output, so the stack does not need revisiting. It does constrain how the library is built, stored and shipped (see "Implications").

### License and maintenance

| Question | Finding |
|---|---|
| License | **AGPL-3.0-or-later** (confirmed in `Cargo.toml` and `LICENSE`). |
| Maintenance | **Development suspended since October 2023.** The author moved to a commercial solver (README, upstream issue #46). Last commit `9d1509f`, 2023-10-01. There are several community forks, but none is clearly maintained or canonical. |
| Builds today? | **Not out of the box.** Upstream requires `bincode = "2.0.0-rc.3"`. Semver resolves that to bincode 2.0.x, which has breaking API changes, so the crate fails to compile. Fixed by pinning `bincode` and `bincode_derive` to `=2.0.0-rc.3` (see the spike's `Cargo.toml`). This kind of bitrot will happen again. |
| Size | ~12.5k lines of Rust, 106 `unsafe` sites. Small enough to fork and maintain ourselves. |

### Setup

- Hardware: cloud container, 4 vCPU (Intel Xeon @ 2.10GHz), 15GB RAM, Rust 1.97, `rayon` multithreading.
- Spots (6-max, 100bb, no rake, hand-written approximate ranges; not copied from any proprietary source):
  - **SRP BTN vs BB:** pot 5.5bb, 97.5bb behind (SPR ≈ 18). BTN ~45% open, BB wide flat range.
  - **3-bet pot SB vs BTN:** pot 23bb, 89bb behind (SPR ≈ 3.9).
- Bet trees (same for both players; `add_allin_threshold` 1.5, `force_allin_threshold` 0.15):
  - **small:** flop 33%; turn 75%; river 75%; raise 3x.
  - **medium:** flop 33% / 75%; turn and river 50% / 100% / all-in; raise 3x.
  - **large:** flop 25% / 50% / 100%; turn and river 33% / 75% / 125% / all-in; raises 2.5x / 4x.
- Target accuracy: exploitability ≤ **0.5% of the pot** (checked every 10 iterations).
- "Compressed" means the solver's 16-bit storage mode instead of 32-bit floats.

### Results

**Flop solves**

| Spot | Board | Tree | Storage | Memory | Solve time | Iterations | Exploitability |
|---|---|---|---|---|---|---|---|
| SRP | Ks7d2c | small | f32 | 5.8GB | 334s (5.6 min) | 150 | 0.42% pot |
| SRP | Ks7d2c | small | 16-bit | 3.0GB (peak 6.0GB RSS) | 286s (4.8 min) | 120 | 0.43% pot |
| SRP | Td9d6h | medium | 16-bit | 8.3GB (peak 8.4GB RSS) | **2112s (35 min)** | 300 | 0.49% pot |
| 3BP | Td9d6h | medium | f32 | 0.7GB | 38s | 130 | 0.40% pot |
| 3BP | Ks7d2c | large | f32 | 3.5GB | 302s (5 min) | 230 | 0.47% pot |

**Memory estimates only** (the solver reports these before allocating):

| Spot | Board | small | medium | large |
|---|---|---|---|---|
| SRP | Ks7d2c | 5.8GB / 3.0GB | 27.5GB / 14.0GB | 211GB / 107GB |
| SRP | Td9d6h | 3.5GB / 1.8GB | 16.3GB / 8.3GB | 125GB / 63GB |
| 3BP | Ks7d2c | — | 1.1GB / 0.6GB | 3.5GB / 1.9GB |
| 3BP | Td9d6h | — | 0.7GB / 0.4GB | 2.2GB / 1.2GB |

(f32 / 16-bit.) An SRP medium tree on Ks7d2c with 16-bit storage (14GB) does not fit this 15GB container. A medium-tree SRP solve at full 32-bit precision was killed by the OOM killer.

**On-demand subgames** (SRP, medium tree, full flop ranges carried to the street; real ranges after flop action are narrower, so these are upper bounds):

| Start | Board | Memory | Solve time |
|---|---|---|---|
| Turn | Ks7d2c 8h | 97MB | 5.6s |
| River | Ks7d2c 8h 3s | < 1MB | < 0.1s (large tree: 0.1s) |

**Serialization** (`save_data_to_file` with zstd level 3; `set_target_storage_mode` truncates the tree before saving):

| Spot / tree | Full tree | Truncated after turn | Flop only |
|---|---|---|---|
| SRP Ks7d2c small, f32 | 1.5GB (2.8GB raw) | 30MB | 0.1MB |
| SRP Ks7d2c small, 16-bit | 664MB | 14MB | 0.1MB |
| SRP Td9d6h medium, 16-bit | not saved (would double peak memory) | 25MB | 0.1MB |
| 3BP Td9d6h medium, f32 | 156MB | 4.6MB | < 0.1MB |
| 3BP Ks7d2c large, f32 | 946MB | 20MB | 0.1MB |

Loading a full SRP tree takes 15–20s. Loading a turn-truncated file takes 0.1–0.25s.

**Sanity checks on the output:** the strategies look like poker. BB checks ~100% on K72r vs a BTN open. On T96tt, BB checks 77% and leads 23% with two sizes. The 3-bettor c-bets 100% on K72r at mixed sizes. OOP equity and EV are in plausible ranges. The 16-bit and 32-bit runs of the same spot give the same root strategy and EV.

### Implications

1. **The library build is an offline batch job.** A realistic SRP tree (medium) takes ~35–60 min per flop on 4 cores and needs 8–14GB in 16-bit mode, so roughly a **32GB machine** for dry boards. 3-bet pots are 10–50x cheaper, so they can afford the large tree.
2. **Full trees cannot ship.** They are 0.15–1.5GB per flop even compressed, which is ~250GB for the D2 library. **Turn-truncated trees can ship** (5–30MB per flop), and so can **flop-only** data (~0.1MB).
3. **Full-hand mode (D4) therefore needs river solving inside the app.** River subgames solve in under 0.1s, so this is cheap and invisible to the user. On-demand turn solves (~5s natively) were considered to cut download size and rejected for now (D11).
4. **Licensing follows from point 3.** Linking `postflop-solver` into the app makes the app a derivative work under AGPL. For personal use this changes nothing. If Taful is ever distributed (stores, web), its source must be published under AGPL. The alternative is to write our own river-only solver (DCFR on a single street is small) and keep `postflop-solver` in the offline build tool only. **Decided: own river solver** (D11 in [decisions.md](decisions.md)).
5. **Store the library in Taful's own format, not the solver's binary format.** The offline tool exports per-node strategies and EVs. This keeps the shipped data independent of an unmaintained crate's serialization (which already broke once on `bincode`), and it lets the reader live in `taful-solver` with no AGPL dependency.
6. **Maintenance:** depend on a **fork we control** (pinned dependencies, the bincode fix, and any later fixes) rather than on upstream. The code base is small enough for that.

The numbers for D2 and D8 are in [decisions.md](decisions.md) under each decision's "Solver spike results" amendment.

---

## §9.3 Hand evaluator and equity

**Date:** 2026-11-04 · **Code:** [`spikes/evaluator`](../spikes/evaluator) · **Verdict: passes.** Use `rs_poker`.

### Candidates

| Crate | Version | License | Last release | Algorithm |
|---|---|---|---|---|
| `rs_poker` | 5.1.0 | Apache-2.0 | 2026-08-13 | perfect-hash 7-card lookup, incremental `SevenCardAccum` |
| `aya_poker` | 0.1.0 | MIT / Apache-2.0 / Zlib | 2023-10-31 (only release) | OMPEval port, compile-time perfect hash |
| `poker` | 0.7.0 | MIT | 2025-06-15 | `treys` port: best 5 of 21 combinations, hash-map lookups |

### Results (single thread, release build)

| Test | `rs_poker` | `aya_poker` | `poker` |
|---|---|---|---|
| Pairwise ordering vs the other two, 1M random 7-card pairs | 0 disagreements | 0 disagreements | 0 disagreements |
| Random 7-card hands (cache-cold) | 44–48 M/s | 54–55 M/s | 6 M/s |
| Exhaustive: all 133,784,560 7-card hands | 1.0s | 1.0s | not run |
| Category counts vs the known 7-card table | **exact match** | **exact match** | — |

Equity on top of `rs_poker`:

| Test | Result | Time |
|---|---|---|
| AsAh vs KsKh, exhaustive (1,712,304 boards) | 82.64% | 23ms |
| AhKh vs QsQd, exhaustive | 46.21% | 24ms |
| 7s2d vs AsKd, exhaustive | 31.87% | 24ms |
| AhKh vs a random hand, Monte Carlo, 10M samples | 67.03% | 514ms |

These agree with commonly published figures (for example, AKs vs a random hand ≈ 67%, AA vs KK ≈ 82%).

### Recommendation

- **Use `rs_poker`** with `default-features = false`, which pulls in only `rand` and `thiserror`. It is the only fast option that is actively maintained, its speed is within 20% of the fastest, and its license is permissive. `aya_poker` is slightly faster but has had one release, in 2023. `poker` is ~8x slower.
- Hide it behind a small evaluator trait in `taful-core` so it can be swapped without touching callers.
- Promote the spike's checks into `taful-core`'s test suite: the exhaustive category-count test (1s in release; run it in release mode in CI, or mark it `#[ignore]` for debug runs), cross-checks against a second evaluator, and pinned exhaustive equities.
- **Equity is not a bottleneck.** Exact heads-up preflop equity takes ~25ms single-threaded, and Monte Carlo runs ~20M samples/s per thread. Exact preflop range vs range (every board × every combo in both ranges, ~1–2 billion evaluations) would take tens of seconds, so use Monte Carlo there (≈0.1% error in well under a second) or exact enumeration once the board is known (postflop, it is fast). That is enough for the calc trainer, lessons (D5 computed numbers) and CI.
- The solver has its own internal evaluator, so this choice does not affect the solver.

## §9.4 Solver benchmark on the build machine

Run 2026-11-04 on the author's PC: AMD Ryzen 9 9950X3D (16 cores / 32 threads, SMT on), 32GB single-channel DDR5, Windows 11. Same `spikes/solver` binary and trees as §9.2, target exploitability 0.5% pot, normal priority, nothing heavy running alongside.

| Run | Wall | Solve | Peak RAM | Exploitability | Spike machine | Speedup |
|---|---|---|---|---|---|---|
| SRP Ks7d2c small, 16-bit | 39.7s | 35.0s, 120 iters | 4.3GB | 0.43% pot | 286s | 8.2x |
| 3BP Ks7d2c large, f32 | 82.9s | 74.0s, 230 iters | 5.9GB | 0.47% pot | 302s | 4.1x |
| SRP Td9d6h medium, 16-bit | 244.6s | 242.9s, 300 iters | 8.0GB | 0.49% pot | 2112s | 8.7x |
| Same, `RAYON_NUM_THREADS=16` | 293.5s | 292.1s, 300 iters | 8.0GB | 0.49% pot | 2112s | 7.2x |
| SRP Ks7d2c medium, estimate only | – | – | est. 14.0GB (16-bit), 27.5GB (f32) | – | – | – |

- Peak RAM is the polled working set and includes the spike's save/reload check, so it sits above the solver's own estimate. `peak_rss` is not reported on Windows.
- Speedup compares solve times. SMT helps: 32 threads cut solve time ~17% vs 16 (~20% more throughput), with identical results.
- The 3BP run scales only ~4x even excluding overhead (~9s). It is the only f32 run, so it is likely memory-bandwidth bound on single-channel DDR5. The 16-bit SRP solves, which dominate the build, scale ~8–9x.
- Dry-board SRP medium (~14GB in 16-bit) fits in 32GB only with other heavy apps closed, so SRP solves run at night. 3-bet pots (~6GB) also fit during the day.
- Budget derived in [spec.md](spec.md) §7.3.
