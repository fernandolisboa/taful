# Taful — Specification

> **Status:** draft for review, 2026-11-04. Derived from [brief.md](brief.md), [decisions.md](decisions.md) (D1–D13) and [spikes.md](spikes.md) / [spikes/9.1-gpui-table.md](spikes/9.1-gpui-table.md). All three §9 spikes passed, so the stack stands.
>
> **Precedence:** decisions.md > this spec > brief.md. Items marked **(proposed)** are choices this spec makes that were not grilled; they stand unless changed in review. Open items are listed in §19 and need a decision before the milestone that depends on them.

---

## 1. Purpose and scope

Taful is a personal, local-first No-Limit Hold'em coach for Windows desktop. It runs one closed loop:

```
play / simulate → grade (EV loss) → review → detect leak → targeted drill → spaced repetition → play again
```

**Product scope (D1):** all of NLHE: cash HU / 6-max / 9-max, short / 100bb / deep stacks, MTT and SNG at every speed, rake. PLO is out.

**This spec covers in detail:** the architecture for the whole scope, and the **first vertical slice** (cash 6-max 100bb, rake 0) end to end. Later formats and features are specified at the interface level only: the slice must not need rework to add them.

**Non-goals:** real-money play; any assistance during live online play (overlays, table reading, HUD import from a running client); a backend before a second client exists (D9).

### 1.1 First-slice acceptance

The first slice is done when, on Windows, the user can:

1. Work through the ~40-lesson skill tree; each lesson unlocks the next through a quiz or drill.
2. Train every mental-math exercise type with timing and see the exact calculation next to the shortcut.
3. Drill preflop decisions for every 6-max node and postflop decisions for the 5 D2 configurations in spot mode, graded by EV loss.
4. Play full hands vs the reference bot, with every hero decision graded and a hand review afterwards.
5. Have every graded mistake, failed quiz and slow calc answer land in one FSRS queue that schedules reviews.
6. Switch layout presets and themes, and get automatic rotating backups.

---

## 2. Glossary

| Term | Meaning |
|---|---|
| `GameConfig` | Every parameter that changes strategy: table size, stacks, blinds/antes, rake, format, structure (§5.2). |
| Line | The preflop action sequence that leads to a postflop configuration, e.g. `BTN open 2.5, BB call`. |
| Configuration | `GameConfig` + line. Identifies one postflop solve family (e.g. "BTN vs BB SRP, 100bb"). |
| Spot | Configuration + board + postflop action history: one node of a solved tree. |
| Spot template | A spot with the board replaced by a *board family* (§7.4). The unit of spaced repetition. |
| Library | The pre-solved reference data: preflop charts + postflop trees per configuration and flop. |
| Reference strategy | Per-node action frequencies and per-combo action EVs from the library or the river solver. |
| EV loss | `max_a EV(combo, a) − EV(combo, chosen)` at a node, in bb (§8). |
| Exploitability | The solver's distance from equilibrium for a spot, stored per spot (D3). |

---

## 3. System overview

Three pipelines, one shared core:

```
                 ┌──────────────────────── offline (author's PC) ─────────────────────────────────────────┐
 preflop solver ─┤ tools/library-build  ──(links postflop-solver fork, AGPL)──▶  library files (Taful fmt) │──▶ static host / local dir
                 └─────────────────────────────────────────────────────────────────────────────────────────┘
 content/ (Markdown + TOML) ──▶ taful-content (render + checks, run in CI and in app)

 apps/desktop (GPUI) ──▶ taful-app (use cases) ──▶ taful-core   (rules, engine, eval, equity, grading, calc, SRS mapping)
                                              ├─▶ taful-solver (library reader, river solver, best response)
                                              ├─▶ taful-content
                                              └─▶ taful-store  (SQLite, migrations, backups)
```

- The app never links `postflop-solver` (D11).
- Everything below `apps/desktop` is UI-free, so web (WASM) and Android (uniffi) are UI-only work later.

---

## 4. Workspace

```
taful/
├─ crates/
│  ├─ taful-core/       # domain model, rules engine, evaluator trait, equity, grading, calc functions,
│  │                    # FSRS rating mapping, range math, ICM interface. No IO, no UI, WASM-safe.
│  ├─ taful-solver/     # library format (read + write), river solver, best response, SubgameSolver trait.
│  │                    # WASM-safe; no AGPL code.
│  ├─ taful-content/    # lesson/quiz/drill loader, expression language, renderer, content checks.
│  ├─ taful-store/      # SQLite schema, migrations, repositories, backups. Not WASM (web gets its own store).
│  └─ taful-app/        # application services (sessions, review queue, library fetch/cache, settings,
│                       # i18n, theme tokens); the only crate the UI talks to. UI-free, async-runtime-agnostic.
├─ apps/
│  └─ desktop/          # GPUI 0.2.2 (pinned), binary `taful`. Views, layout model, token → GPUI color mapping.
├─ tools/
│  ├─ library-build/    # offline: preflop solve, postflop batch solves, export to Taful format,
│  │                    # river-solver validation. Depends on our postflop-solver fork (AGPL).
│  ├─ content-check/    # CLI wrapper over taful-content checks, used by CI.
│  └─ arch-check/       # CI: crate dependency direction + no AGPL/GPL in shipped graphs (cargo metadata).
├─ content/
│  ├─ lessons/{en-US,pt-BR}/*.md
│  ├─ quizzes/*.toml
│  ├─ skill-tree.toml
│  ├─ bots/*.toml       # bot profiles (after first slice)
│  ├─ layouts/*.json
│  └─ themes/*.json
├─ i18n/{en-US,pt-BR}/*.ftl
└─ docs/
```

- **Dependency rule:** `core ← solver ← app ← desktop`, `core ← content ← app`, `store ← app`. `core` depends on nothing in the workspace. Enforced by a CI check on `cargo metadata`.
- `tools/library-build` is a workspace member but is never a dependency of anything shipped. A CI check fails if any shipped crate's dependency graph contains `postflop-solver` (D11).
- The `spikes/` folder stays outside the workspace and is deleted once the slice reaches M4.
- **i18n and theme tokens live in `taful-app`** (amended in M0): both are pure data that every client (desktop, web, Android) needs, so keeping them UI-free avoids redoing them per client. The desktop app only maps tokens to GPUI colors (`apps/desktop/src/theme.rs`).
- **Product name** lives in one constant (`taful_app::PRODUCT_NAME`) plus the `-product` Fluent term (a test keeps them equal); crate names use `taful-` only because renaming crates is cheap (D10).

### 4.1 Key dependencies

| Need | Choice | Notes |
|---|---|---|
| UI | `gpui = "=0.2.2"` | Verified on Windows (spike §9.1). Linux/macOS break on `xattr`; irrelevant for now. |
| Hand evaluator | `rs_poker` 5.x, `default-features = false` | Behind `HandEvaluator` trait (spike §9.3). |
| Spaced repetition | `fsrs` 6.x (`fsrs-rs`) | **BSD-3-Clause** (crates.io, v6.6.2 released 2026-08-29; checked 2026-11-04). Closes brief §8 open item. |
| RNG | `rand_chacha` (`ChaCha8Rng`) | Seedable, portable across platforms and WASM. |
| SQLite | `rusqlite` (bundled) + `rusqlite_migration` **(proposed)** | Backup API used for snapshots. |
| i18n | Fluent (`fluent-bundle`) **(proposed)** | pt-BR first, en-US source. |
| Serialization | `serde` + `toml` / `serde_json` | Content, layouts, themes, configs. |
| Compression | `zstd` | Library files. |
| Postflop solves (offline) | fork of `postflop-solver` with `bincode =2.0.0-rc.3` pin | Fork lives under the author's GitHub account. |

---

## 5. Domain model (`taful-core`)

### 5.1 Cards and ranges

- `Card(u8)` 0..52, `Rank`, `Suit`; `Hand = [Card; 2]` normalized high card first; `Board` = 0, 3, 4 or 5 cards.
- `Combo` index 0..1326 (canonical ordering shared with the library format).
- `Range = [f32; 1326]` weights in [0, 1], plus parsing/printing of standard notation (`AKs, TT+, A5s-A2s, KQo:0.5`) and the 13×13 grid view.
- `HandEvaluator` trait (`rank7(&[Card; 7]) -> HandRank`) with the `rs_poker` implementation. A second implementation (naive best-of-21) exists only in tests for cross-checking.
- Equity: exact enumeration (combo vs combo, range vs range on a known board) and seeded Monte Carlo (preflop range vs range). API returns equity plus, for Monte Carlo, the standard error.

### 5.2 `GameConfig` (D1)

```rust
pub struct GameConfig {
    pub table_size: u8,                 // 2..=9
    pub format: Format,                 // Cash | Mtt(TournamentSpec) | Sng(TournamentSpec)
    pub blinds: Blinds,                 // sb, bb, ante, ante_kind (none | per-player | big-blind ante)
    pub stacks: Stacks,                 // Uniform(bb) | PerSeat(Vec<chips>)
    pub rake: Rake,                     // percent, cap (bb), no_flop_no_drop; default zero
}
pub struct TournamentSpec { pub payouts: Vec<f64>, pub structure: BlindStructure /* speed */, pub players_left: u32 }
```

- All amounts are **integer chips** with `bb = 100 chips` for cash, so 2.5bb opens and 33% bets have exact representations. Displayed in bb.
- `GameConfig::key()` is a stable content hash of the canonical TOML form; it is the first component of every library key.
- No code path matches on `table_size == 6` or `stacks == 100bb`. A property test runs the engine over random valid configs.

### 5.3 Positions, actions, lines

- `Position` derived from `table_size` and button seat (`UTG, UTG1, …, LJ, HJ, CO, BTN, SB, BB`; HU uses `BTN/SB` + `BB`).
- `Action = Fold | Check | Call | Bet(chips) | Raise(to_chips) | AllIn`.
- `Line` = ordered preflop actions; `ActionHistory` = per-street actions. Both have a canonical string form used in keys and logs (e.g. `BTN:r250,SB:f,BB:c`).
- Sizing abstraction: a `BetTree` lists the allowed sizes per street and raise depth (as % pot or raise multiplier + all-in thresholds), mirroring the solver's tree config.

### 5.4 Rules engine

- `GameState` is an immutable value; `apply(state, action) -> Result<GameState, IllegalAction>`; `legal_actions(state, &BetTree) -> Vec<Action>`.
- Implements full NLHE rules: posting, min-raise (incl. incomplete all-in raises not reopening action), all-ins, **side pots for any number of players**, showdown, split pots, odd chip to first seat left of button, rake per `Rake`.
- Deterministic dealing from a `ChaCha8Rng` seeded per hand. The seed is stored with the hand; replaying seed + hero actions reproduces the hand exactly (D4).
- The engine knows nothing about strategy. Spot mode and full-hand mode are both drivers over the same engine (§10).

### 5.5 Equity model (formats)

```rust
pub trait EquityModel { fn to_bb_equiv(&self, stacks_after: &[Chips], hero: Seat) -> f64; }
```

- `ChipEv` (cash; identity in bb).
- `Icm { payouts }` (MTT/SNG; Malmuth–Harville; $EV converted back to bb-equivalents by the current bb in $-per-chip at the hero's stack, D3). Implemented with the tournament modules, interface fixed now.

---

## 6. Reference strategy: preflop

The first slice needs **full 6-max 100bb rake-0 charts for RFI, vs open, vs 3-bet, vs 4-bet, squeeze and blind vs blind (D2)**, with **per-combo action EVs**, because grading is EV-based (D3).

**Decided (D12):** an **own offline preflop solver** in `tools/library-build`:

- CFR (DCFR) over the full 6-max preflop tree with a fixed sizing set (opens 2.5bb, SB open 3bb; 3-bets 3x IP / 3.5–4x OOP; 4-bets ~2.2x; 5-bet all-in).
- Postflop is abstracted: a terminal node's value is `pot × equity × realization(position, pot type, SPR bucket)`.
- Realization factors start from published-style defaults and are then **calibrated from the postflop library**: for each solved configuration, each combo's root EV ÷ (pot × raw equity) averaged over the flop subset. Iterate: charts v0 → postflop solves → calibrate → charts v1. One iteration is expected to be enough; the build tool reports the chart diff.
- Output: frequencies and per-combo EVs per node, in the library format (§7.5). Exploitability is reported per node and stored.
- Nothing is copied from proprietary sources.

Push/fold and ICM charts for tournaments are computed separately (Nash push/fold is exact and cheap) when the tournament work starts.

---

## 7. Reference strategy: postflop library

### 7.1 Contents (D2, amended)

| Config | Pot type | Tree | Flops |
|---|---|---|---|
| BTN vs BB | SRP | medium | ~49 |
| CO vs BB | SRP | medium | ~49 |
| SB vs BB | SRP | medium | ~49 |
| BTN vs SB | 3-bet pot | large | ~49 |
| CO vs BTN | 3-bet pot | large | ~49 |

Trees as measured in spike §9.2 (medium: flop 33/75%; turn and river 50/100%/all-in; raise 3x. Large: flop 25/50/100%; turn and river 33/75/125%/all-in; raises 2.5x/4x). Target exploitability ≤ 0.5% pot. Ranges come from the preflop charts (§6).

### 7.2 Flop subset

- ~49 canonical flops per configuration, chosen by **weighted stratified sampling** over the 1,755 suit-isomorphic flops on texture features (high card, pairedness, suitedness, connectedness). Each subset flop carries a weight = number of real flops it represents, so aggregate statistics stay unbiased.
- The same subset is used for all five configurations so lessons and drills can compare them.

### 7.3 Build tool (`tools/library-build`)

- Input: a build manifest (TOML) listing configs, ranges source, tree, flop subset, accuracy target.
- Resumable batch job, one output file per (configuration, flop); a finished file is skipped on rerun. Logs time, memory, iterations, exploitability per solve.
- Per solve, while the full tree is in memory: export the **turn-truncated** strategy and EVs (D11), and, after the first slice, per-profile best-response EVs (D8).
- Validation: the own river solver is checked against `postflop-solver` on a fixed set of river spots (EVs and exploitability within tolerance, D11). Runs in the tool's tests, not in app CI.
- Hardware: ≥ 32GB RAM for SRP dry boards (spike §9.2). Runs on the author's PC (Q4), one solve at a time, at low priority and pausable.
- Measured on that PC (Ryzen 9 9950X3D, 32 threads, 2026-11-04; [spikes.md](spikes.md) §9.4): SRP medium ~4–7 min per flop (Td9d6h: 245s, 8GB peak), 3-bet pot large ~1.5 min per flop (83s, 6GB peak). About 7–9x the 4-vCPU spike machine on 16-bit SRP solves (~4x on the f32 3-bet-pot run); SMT cuts solve time ~17%.
- Budget: the first slice (245 solves) takes ~16 h, one night plus a few hours. Full coverage (5 × 1,755 = 8,775 solves) takes ~430–700 h of solving, about 3–4 weeks running 24/7 or 6–10 weeks of nights and weekends.

### 7.4 Boards outside the subset

**Decided (D13): deal postflop only from the library.** Whenever a hand reaches a solved configuration, the flop is drawn from the subset flops (with a random suit isomorph, weighted by §7.2 weights), conditioned on the hole cards already dealt. Grading stays exact and there is no mapping error.

- A *board family* (for spot templates, D6) = subset flops in the same texture class. Re-instantiating a template picks another flop from the family.
- Mapping a real random flop to a "nearest" library flop is rejected: hero's actual hand strength changes with the board, so the grade would be wrong in exactly the hands that matter.
- Variety grows as the library grows; the subset is a manifest parameter. The goal is all 1,755 canonical flops (100% of real flops), added in order of real-flop weight after the first ~49, ties broken by stratified sampling over texture classes (§7.2) (D13).

### 7.5 Library format (Taful's own, D11)

One file per (configuration, flop), `zstd`-compressed, little-endian:

```
header   magic "TAFL", format_version u16, game_config_key [u8;32], line string, flop [u8;3],
         bet_tree descriptor, ranges at root (2 × [u8;1326] quantized), exploitability_pct f32,
         ev_scale f32 (bb per EV unit), solver id + build manifest hash
nodes    flat array in DFS order; per node:
           player u8, action list (kind u8 + size u16 in chips), child offsets u32,
           strategy: [u8; n_actions × n_live_combos]  (frequencies × 255, renormalized on read)
           ev:       [i16; n_actions × n_live_combos] (× ev_scale), acting player only
chance   turn cards: per turn card, offset of the turn subtree (truncated at river start)
```

- Only combos with non-zero reach are stored (live-combo index per node).
- Nodes whose reach < 0.1% for both players are dropped; reaching one at play time is treated as "left the library" (D4).
- Expected size: 2–4x smaller than the solver's 16-bit turn-truncated files, so roughly 1–2GB for the D2 library. Measured in M4; if it misses, the next lever is dropping turn EVs for non-acting combos.
- Preflop charts use the same container with a single "preflop" pseudo-board.
- `taful-solver` is the only reader and writer. Format changes bump `format_version`; the reader supports the current and previous version.

### 7.6 Delivery and cache (D11)

- `LibrarySource` trait with two implementations: `LocalDir(path)` and `Http(base_url)`. The HTTP layout is static files: `{base}/library/v{format}/{config_key}/{flop}.tafl` plus a `manifest.json` listing every file with size and BLAKE3 hash.
- The desktop cache lives in the OS cache dir (not in the user DB; never backed up or synced, D9). Files are verified against the manifest hash before use. LRU eviction above a configurable cap (default 10GB).
- Prefetch packs = manifest subsets (e.g. "BTN vs BB SRP"), downloadable from settings.
- Offline: cached spots work; an uncached spot ends the hand early with a message (D11).
- **(proposed)** For the first slice the base URL points at a local folder produced by the build tool (single user, single machine). The static host is picked when the web client starts (D9/D10); any static file host works with this layout.

### 7.7 River solver (`taful-solver`, D11)

- Input: board (5 cards), both ranges with reach weights at river start, pot, effective stack, river bet tree, rake.
- Algorithm: DCFR on a single street; hand strengths precomputed once per board with the evaluator and sorted, so terminal showdown values are O(n) per node with prefix sums.
- Stop at exploitability ≤ 0.3% pot or 1,000 iterations; target < 100 ms native (spike: postflop-solver < 0.1 s).
- Output: same node layout as §7.5, in memory.
- `SubgameSolver` trait so a turn solver (rejected for now) can be added later without touching callers.
- River ranges come from propagating root ranges through the stored flop and turn strategies along the actual line.

### 7.8 Best response (D8)

- `taful-solver::best_response(tree, locked_strategy) -> per-combo EVs` for river subtrees (on demand). Flop/turn best-response EVs per profile are precomputed by the build tool (D8 amendment). Partial locks are not supported.

---

## 8. Grading (D3)

```rust
pub enum Grade { Best, Correct, Inaccuracy, Mistake, Blunder }
pub struct Thresholds { pub version: u32, pub best_min_freq: f32, pub abs_bb: [f64; 3], pub rel_pot_pct: [f64; 3] }
pub fn grade(ev_loss_bb: f64, pot_bb: f64, chosen_freq: f32, tolerance_bb: f64, t: &Thresholds) -> Grade
```

- `ev_loss = max_a EV(combo, a) − EV(combo, chosen)`, from the reference node for hero's exact combo.
- `Best` if `chosen_freq ≥ 0.10` or `ev_loss ≤ tolerance_bb`, where `tolerance_bb = exploitability_pct × pot` of that spot.
- Otherwise each scale yields a grade and the **worse** one wins. Defaults: abs `[0.1, 0.5, 2.0]` bb, rel `[2, 5, 15]` % of pot before the decision.
- Thresholds load from `config/grading.toml`; changing them never requires a code change. `version` is bumped with any change and stored with each graded decision (the "thresholds version" in §14.2).
- Property tests: monotone in `ev_loss`; never stricter than `tolerance_bb`; `Best` whenever the chosen action has ≥ 10% frequency.
- Every graded decision is persisted with the full context (§14) so a later threshold change can regrade history.
- **Explanation payload:** action frequencies at the node (for hero's combo and for the whole range), EV per action for hero's combo, range-level stats (equity, EV, nut advantage) and, vs profiled bots, the GTO vs exploitative line and their EV gap (D8).

---

## 9. Bots (D8)

- `Bot` trait: `act(state, rng) -> Action`.
- `ReferenceBot`: samples the reference strategy at the node for its actual combo (seeded). First slice.
- `ProfiledBot` (right after): a `Profile` TOML (`content/bots/*.toml`) of deviation parameters, applied as a transform over the reference strategy, then renormalized:

```toml
id = "calling-station"
[preflop]   vpip_mult = 1.8  pfr_mult = 0.6  threebet_mult = 0.4
[postflop]  fold_to_cbet = -0.25  fold_to_raise = -0.20  bluff_value_shift = -0.3  aggression = -0.4
[sizing]    prefer = "small"
```

- Grading vs a profiled bot uses best-response EVs against that profile (§7.8). Profiles are deterministic under a seed.

---

## 10. Simulator (D4)

One `HandDriver` over the engine; the two modes are configurations of it.

### 10.1 Spot mode

- Input: a `SpotTemplate` (config, line, postflop history, board family, hero seat) + seed.
- The driver instantiates a board from the family and hero's combo from hero's range at that node (weighted by reach, so drills see realistic hands), renders the table state, takes one hero decision, grades it, shows the explanation.
- Sources of spots: lesson drills, the review queue, "train this configuration" menus, and drills from mistakes.

### 10.2 Full-hand mode

- Hero picks a seat (or "random"); villains are bots (§9).
- Preflop: every hero decision is graded against the preflop charts.
- If the preflop line reaches a library configuration, the flop is dealt per §7.4 and play continues street by street; turn nodes come from the library, rivers from the river solver.
- Hero's sizing buttons are exactly the sizes in the tree (D4).
- If the line leaves the library (unsolved configuration, multiway pot, pruned node, uncached file offline), the hand ends with a message; the line is logged to `solve_candidates` (§14) for future builds.
- End of hand: review screen with every decision's grade, EV loss and explanation, plus the hand's total EV loss; each decision graded below `Correct` becomes a review item (D6).

### 10.3 Optional live coach (wave 2)

A setting that shows the grade immediately after each decision in full-hand mode instead of at the end.

---

## 11. Mental-math trainer

Exercise types for the first slice, each a generator in `taful-core::calc` (seeded) plus an exact solver:

| Exercise | Prompt | Shortcut shown | Exact answer |
|---|---|---|---|
| Outs | hole cards + board | count by draw type, discount for dirty outs | enumeration |
| Rule of 2 and 4 | outs + street | `outs × 2` / `outs × 4` (with the >8-outs correction) | exact hit probability |
| Pot odds | pot, bet | `bet / (pot + 2·bet)` | same, exact |
| MDF | bet size as % pot | `pot / (pot + bet)` | same |
| Alpha | bluff size | `bet / (pot + bet)` | same |
| SPR | stacks + pot | `stack / pot` | same |
| Equity vs range | hand, board, villain range | heuristic bands | exact enumeration |

- Answers are numeric with a tolerance per exercise (e.g. ±2 percentage points for equity).
- Timed: a per-exercise target time from config. Rating: wrong → Again; right but over target → Hard; right within target → Good; right within half the target → Easy.
- Every attempt is stored; calc items share the FSRS queue (D6).

---

## 12. Lessons and content (D5)

### 12.1 Files

```
content/lessons/en-US/pot-odds-101.md
content/lessons/pt-BR/pot-odds-101.md
```

```markdown
+++
id = "pot-odds-101"
version = 3
title = "Pot odds"
prerequisites = ["outs-101"]
tags = ["math", "postflop"]
glossary = ["pot-odds", "equity"]
unlock = { quiz = "pot-odds-101-quiz" }      # or { drill = "<spot-template-id>", min_grade = "correct", count = 5 }
+++

Facing a half-pot bet you need {{ calc: pot_odds(pot=100, bet=50) | pct(1) }} equity to call.
```

- Source in en-US; pt-BR translations are AI-drafted like the source. Both locales share id, version and the exact set of placeholders.
- `content/skill-tree.toml` orders lessons and groups them into tracks. The first track has ~40 lessons up to intermediate cash 6-max 100bb.
- Quizzes are TOML: multiple choice, numeric (with tolerance), range-painting (wave 2) and spot questions (a `SpotTemplate` id). Numeric answers are placeholders too.

### 12.2 Expression language

- Syntax: `{{ calc: fn(arg=value, …) | formatter(args) }}`, plus `{{ spot: <id>.freq("BTN", "bet33") }}` for reference-strategy citations.
- Only whitelisted pure functions from `taful-core` (calc, equity, combinatorics, chart lookups). No variables, loops or user code. Parsed with a small hand-written parser; unknown function or bad argument = render error.
- Formatters: `pct(d)`, `bb(d)`, `ratio`, `int`, `cards`. Localized number formatting (pt-BR uses comma decimals).

### 12.3 Checks (CI and app)

`tools/content-check` fails the build when:

1. Any placeholder errors in any locale.
2. A rendered value differs from the pinned snapshot in `content/lessons/.snapshots/<id>.toml` (drift). Snapshots are updated explicitly with `content-check --bless`.
3. Locales disagree on id, version or placeholder set.
4. The skill tree has a cycle, a missing prerequisite, or a lesson with no unlock.
5. A glossary term is undefined in `i18n/*/glossary.ftl`.

### 12.4 Versioning

- Progress is keyed by `(lesson id, version)`. A text-only edit keeps the version; a change to the quiz/drill bumps it. Review items keep pointing at the version they were created from until their next review, then move to the current one.

---

## 13. Spaced repetition (D6)

- `fsrs` crate, default parameters at start, desired retention 0.9 (setting).
- Grade → rating: best → Easy, correct → Good, inaccuracy → Hard, mistake/blunder → Again. Calc ratings per §11. Quiz: right → Good, wrong → Again.
- **Item kinds:** `spot_template`, `calc_exercise` (exercise type + parameter band), `lesson_quiz`. One queue, one daily session that mixes kinds.
- **Creation:** a graded decision below `Correct` creates or updates the item for its spot template (dedup by template key: config + line + history + texture class + hero position). Full-hand mode feeds items per decision (D6).
- **Leak detection:** aggregate EV loss per (configuration, street, node type, texture class) over a rolling window; the top leaks are shown on the home screen with a "drill this" action that creates a focused session from that bucket.
- The full review log is stored; parameter re-optimization runs locally on demand once there are ≥ 1,000 reviews (`fsrs` optimizer).

---

## 14. Persistence (`taful-store`, D9)

### 14.1 Rules

- One SQLite file in the user data dir. WAL mode.
- **Every table:** `id TEXT PRIMARY KEY` (ULID), `created_at`, `updated_at`, `deleted_at` (nullable; soft delete). A schema test fails the build if any table violates this (D9).
- Migrations are forward-only, numbered, embedded in the binary, applied at startup inside a transaction after an automatic pre-migration backup.

### 14.2 Tables (first slice)

| Table | Holds |
|---|---|
| `settings` | key/value JSON (locale, desired retention, backup folder, active layout/theme, library base URL). |
| `lesson_progress` | lesson id, version, state (locked/available/done), best quiz score. |
| `quiz_attempts` | quiz id, version, answers, score, duration. |
| `calc_attempts` | exercise type, params (JSON), answer, correct value, duration, rating. |
| `hands` | mode, `GameConfig` key + TOML, seed, hero seat, bot ids, library format version, ended reason, total EV loss. |
| `decisions` | hand id (nullable for spot mode), node key, hero combo, chosen action, EV per action, frequencies, pot, EV loss, grade, thresholds version. |
| `review_items` | kind, template key / exercise key, FSRS memory state, due, last review. |
| `review_log` | item id, reviewed at, rating, elapsed days, source (decision/attempt id). |
| `layouts` | name, schema version, JSON. Built-ins are not stored; only user copies. |
| `themes` | name, schema version, JSON. |
| `solve_candidates` | config key, line, history, count, last seen. |

### 14.3 Backups

- Every 24 h of app use and on exit (at most once per hour), a snapshot via SQLite's backup API to the user-chosen folder, named `taful-YYYYMMDD-HHMMSS.sqlite`. Keep the last 7 daily + 4 weekly. Manual export/import from settings; import validates the schema version and runs migrations.
- Secrets (Claude API key, wave 3) live in Windows Credential Manager, never in the DB or backups.

---

## 15. UI (`apps/desktop`)

### 15.1 Screens (first slice)

1. **Home:** due reviews, top leaks, continue lesson, quick actions.
2. **Skill tree:** tracks and lessons with lock state.
3. **Lesson reader:** rendered Markdown, glossary tooltips, quiz at the end.
4. **Table:** spot and full-hand modes; action buttons; sizing buttons from the tree; explanation panel.
5. **Hand review:** decision list with grades; replay controls (step through the seed).
6. **Calc trainer:** exercise picker, timed prompt, result with shortcut vs exact.
7. **Review session:** the FSRS queue, mixing kinds.
8. **Settings:** locale, layout preset, theme, backups, library source and prefetch packs, grading thresholds (read-only view).

### 15.2 Table rendering

As in spike §9.1: oval rail + felt, seats on an ellipse, avatars, stacks, positions, dealer button, chips, pot, 4-color deck, card backs. Animations: dealing, chips to pot, pot to winner (GPUI animation, 150–300 ms, can be disabled).

### 15.3 Layout model (D7)

```json
{ "schema": 1, "grid": { "cols": 48, "rows": 27 },
  "panels": [ { "id": "explanation", "x": 33, "y": 2, "w": 14, "h": 18, "visible": true },
              { "id": "actions",     "x": 14, "y": 22, "w": 20, "h": 4 } ] }
```

- Positions in grid cells over a resolution-independent grid; the table area scales to fit the remaining space.
- Built-in presets in `content/layouts/`; user copies in `layouts`. Save / load / reset in the first slice.
- Schema is versioned; each version bump ships a migration function (panels added later get default positions).
- **Edit mode (right after the first slice):** drag by header, free follow, dashed landing preview, snap to grid on release, Alt drops without snap, dot grid painted in one canvas pass and shown only in edit mode (behavior approved in spike §9.1).

### 15.4 Themes (D7)

- A theme is a JSON token file (`content/themes/default.json`): felt, rail, chip colors, accents, text, suit colors (4), card face/back, grade colors, spacing and radii.
- Default: petrol-teal felt, amber/gold accents, cream type, **no red anywhere**; hearts burnt orange, diamonds gold (as in the spike's `theme.rs`). Grade colors also avoid red (blunder = deep amber/violet, chosen in M6 with a contrast check).
- A test scans `apps/desktop/src` and fails on color literals (`rgb(`, `rgba(`, `hsla(`, hex) outside the theme module.
- WCAG AA contrast is checked for text tokens over their backgrounds in the same test.
- In-app theme editor comes right after the first slice, on the same token model.

### 15.5 i18n

- Fluent `.ftl` files per locale; every user-facing string goes through it. pt-BR is the default and the only shipped locale in the first slice; en-US exists for development and as the source.
- A test fails if a key exists in en-US but not in pt-BR.

---

## 16. Later features (interface-level)

| Feature | Wave | Hooks already in the first slice |
|---|---|---|
| Profiled bots | right after | `Bot` trait, best-response EVs in the build tool, profile TOML. |
| Layout editor, theme editor | right after | JSON layout/token models. |
| Range trainer, texture drills | 2 | `Range` grid, texture classes from §7.2. |
| Hand history import (PokerStars, GG) + live logger | 2 | `hands`/`decisions` tables; import maps to engine actions; graded where the line hits the library. |
| Glossary tooltips everywhere | 2 | glossary in Fluent (first slice uses it in lessons). |
| AI coach | 3 | Prompt built from the explanation payload (§8); answers must cite it. Key in Credential Manager. |
| Other formats (HU, 9-max, short/deep, rake) | 3 | `GameConfig` keys in the library; new build manifests. |
| MTT / SNG | 3 | `EquityModel::Icm`, `TournamentSpec`, push/fold generator. |
| Bankroll, variance, risk of ruin | 3 | `taful-core` math module. |
| Mental game, gamification, HUD literacy | 3 | `settings` + new tables under the same schema rules. |
| Web / Android | later | UI-free crates; `LibrarySource::Http`; sync API per D9; host `taful.filx.app` (D10). |

---

## 17. Testing and CI

- **taful-core:** exhaustive 7-card category counts (release mode; `#[ignore]` in debug), cross-check vs naive evaluator, pinned exact equities (AA vs KK 82.64%, etc.), engine property tests (chip conservation, legal-action invariants, side-pot correctness, replay determinism), grading property tests, calc solvers vs brute force.
- **taful-solver:** format round-trip (write → read → identical within quantization error), river solver exploitability bound on fixed spots, golden files for a tiny tree.
- **tools/library-build:** river solver vs `postflop-solver` agreement (not in default CI; run on the build machine).
- **taful-content:** `content-check` over the whole `content/` tree.
- **taful-store:** schema rule test, migration test from every past version, backup/restore round-trip.
- **desktop:** color-literal lint, i18n key parity, layout schema migrations, a smoke run on `windows-latest` with a screenshot artifact (as in the spike workflow).
- **Architecture checks:** crate dependency direction; no AGPL crate in shipped graphs.
- CI: `cargo fmt --check`, `clippy -D warnings`, tests on Windows (desktop) and Linux (non-UI crates).

---

## 18. Milestones (first slice)

| M | Delivers | Depends on |
|---|---|---|
| M0 | Workspace layout, CI (fmt, clippy, tests, architecture checks, Windows build), app shell with i18n + theme tokens. | — |
| M1 | `taful-core`: cards, evaluator, equity, `GameConfig`, engine with side pots, grading, calc generators/solvers. | M0 |
| M2 | `taful-store`, `taful-content`, calc trainer UI, lesson reader, skill tree, FSRS queue, backups. First usable loop (lessons + calc). First ~10 lessons. | M1 |
| M3 | Preflop solver + charts in library format; preflop spot mode and full-hand preflop. | M1 |
| M4 | Library format, build tool, first 245 postflop solves (49 flops × 5 configs), river solver + validation, local `LibrarySource`; postflop spot mode. Coverage then grows toward 1,755 flops in the background (D13). | M3, Q4 |
| M5 | Full-hand mode end to end, hand review/replay, drills from mistakes, leak detection. | M4 |
| M6 | Remaining lessons (to ~40), layout presets, theme polish, grade colors, performance pass. Slice acceptance (§1.1). | M5 |

The calc trainer and lessons (M2) give daily value before the library exists, so the loop is usable while the M4 build runs (~16 h for the first slice, weeks for full coverage).

---

## 19. Open questions

| # | Question | Proposed | Blocks |
|---|---|---|---|
| Q1 | Source of preflop charts with EVs | **Decided (D12):** own offline preflop solver, realization calibrated from the postflop library (§6) | — |
| Q2 | Boards outside the flop subset | **Decided (D13):** deal only solved flops; grow coverage to all 1,755 (§7.4) | — |
| Q3 | Trademark checks for "Taful" (INPI 9/41, USPTO/EUIPO) | Before any public release | release |
| Q4 | Machine for the library build (≥ 32GB RAM, many cores) | **Decided:** author's PC (Ryzen 9 9950X3D, 16 cores / 32 threads, 32GB), low priority at night; benchmarked in §7.3 | — |
| Q5 | Static host for the library | Deferred to the web client; local folder for the first slice (§7.6) | web |
