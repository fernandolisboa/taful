# Taful — Project Brief

> Not a spec: it captures intent and the decisions taken so far. The open questions from the first `grill` session were decided on 2026-11-04 and are logged as D1–D10 in [decisions.md](decisions.md), together with later decisions (D11+). That log is the source of truth when the two disagree. The spec derived from this brief plus the decision log is [spec.md](spec.md).

## 1. Concept

**Taful** (archaic Portuguese: a habitual gambler, someone who frequents gaming houses) is a personal poker coach. It teaches No-Limit Hold'em from zero to advanced through lessons, calculation training and decision simulation on a realistic poker table, and it measures progress over time.

- **User:** a single user (the author), learning from beginner level. Not a commercial product for now.
- **Scope (D1):** the full No-Limit Hold'em landscape, none of it optional:
  - Cash games: heads-up, 6-max and full ring (9-max).
  - Stack depths: short (≈20–40bb), standard (100bb) and deep (150–200bb+).
  - Tournaments: MTT (ICM, push/fold, short stacks, bubble and final-table dynamics) and Sit & Go.
  - Tournament speeds: slow, regular, turbo and hyper-turbo.
  - Rake: modeled in reference strategies and taught as its own topic (rakeback, rake-adjusted win rate).
  - Omaha / PLO is out until the NLHE product is complete and in use.
- **Primary platform:** Windows desktop. Web and Android come later, and iOS after that.
- **Out of scope:** real-money play, and any real-time assistance during online play (overlays, table reading). RTA is banned by poker sites. In-game calculations are mental techniques the user practices *inside* Taful.

## 2. Core idea: the closed loop

The product's backbone is a loop, not a set of separate features:

```
play / simulate → review → detect leak → targeted drill → spaced repetition → play again
```

Every feature feeds this loop or is fed by it. Mistakes made anywhere in the app become drill material.

Waves below describe sequencing, not scope: every format in §1 is part of the product. The first vertical slice is **cash 6-max 100bb** because it exercises the whole loop with the cheapest reference data (D1, D2).

## 3. Features by wave

### Wave 1 — MVP (first slice: cash 6-max 100bb)
- **Lesson track as a skill tree**, from 101 to advanced. Each lesson ends in a quiz or drill that unlocks the next one. Content is data, not code (D5):
  - One Markdown file per lesson with TOML front matter (id, title, prerequisites, tags, glossary terms, unlocking quiz/drill). Source in en-US, shipped in pt-BR via i18n.
  - Drafted by AI (Claude). Every number a lesson states is a placeholder evaluated by `taful-core`, never hand-written; CI renders every lesson and fails on errors or drift.
  - First track: about 40 lessons, from 101 to intermediate cash 6-max 100bb. Other tracks follow the format rollout.
- **Decision simulator** on a visual table (seats, chips, cards, pot, action buttons), with two modes from the start (D4):
  - **Spot mode:** a single decision point. The default unit for drills and spaced repetition.
  - **Full-hand mode:** street by street, villains sample the reference strategy, and every hero decision is graded. Hero sizings are limited to those in the solved tree. Lines that leave the library end the hand early and are logged as candidates for new solves.
  - Both modes share one engine, and villain sampling is seedable so any hand replays exactly.
- **Grading by EV loss**, not right/wrong: `best / correct / inaccuracy / mistake / blunder` (D3). Each decision is graded on an absolute scale (bb) and a relative one (% of pot) and gets the worse of the two:

  | Grade | Absolute | Relative |
  |---|---|---|
  | best | played by the reference at ≥ 10% frequency, or within solver tolerance of max EV | — |
  | correct | < 0.1bb | < 2% of pot |
  | inaccuracy | < 0.5bb | < 5% of pot |
  | mistake | < 2bb | < 15% of pot |
  | blunder | ≥ 2bb | ≥ 15% of pot |

  Thresholds live in configuration and will be calibrated. Tournaments use $EV from the ICM layer converted back to bb-equivalents. The explanation for each grade shows the reference strategy (action frequencies) and the reasoning behind it.
- **Timed mental math trainer:** outs, rule of 2 and 4, pot odds, MDF, alpha (required fold equity for a bluff), SPR, and equity estimates vs ranges. Every shortcut technique is shown next to the exact calculation.
- **Spaced repetition with FSRS** (`fsrs-rs`) (D6). Grades map to ratings: best → Easy, correct → Good, inaccuracy → Hard, mistake/blunder → Again. Lessons, calc drills and spots share one queue; the full review log is stored so parameters can be re-optimized locally.
- **Drills from your own mistakes:** a review item is a spot template (positions, action history, sizing), re-instantiated with randomized boards. Full-hand mode feeds the queue per decision, not per hand.
- **Reference (GTO) bot** as the only opponent (D8).
- **Layout presets and themes** as data (D7): see §6.

### Right after the first slice
- **Play vs bots with profiles** (nit, calling station, maniac, TAG) (D8). This teaches exploitative play, not only GTO.
  - A profile is a data file of parametric deviations from the reference strategy (range widths, fold-to-c-bet, bluff/value ratio, sizings, aggression), not hand-written rules.
  - Decisions against a profiled bot are graded by EV against that bot's actual strategy (node-locked solves). The explanation shows the GTO line, the exploitative line and the EV gap.
- **Drag-and-drop layout editor** and **in-app theme editor** on top of the same data (D7).

### Wave 2
- **Range trainer:** paint a range on the 13×13 grid and compare it to the reference chart.
- **Board texture drills:** dry/wet, range advantage, nut advantage.
- **Hand review & replay:** import hand histories (PokerStars and GG formats) plus a quick logger for live games.
- **Optional live coach:** a toggle that nudges the user the moment they make a clear mistake in simulation.
- **Glossary tooltips** on every technical term, everywhere, including inside explanations.

### Wave 3
- **AI coach (Claude API)** explaining hands in natural language, **grounded in solver output**, never free-form opinion.
- **Other formats as data:** heads-up, full ring, short and deep stacks, and rake-adjusted strategies, added to the same spot library keyed by `GameConfig` (D1).
- **Tournament modules (MTT and SNG):** ICM, push/fold (Nash charts are computable, no licensing needed), short stacks, bubble and final-table play, and tournament speeds/structures.
- **Bankroll & sessions:** win rate, variance simulator, risk of ruin.
- **Mental game:** session journal, tilt tracker, pre-session warm-up.
- **Light gamification:** XP, streaks, per-skill rank.
- **HUD stats literacy:** what VPIP, PFR, 3-bet and similar stats say about an opponent (educational only).

## 4. Reference strategy (the "correct answer" source)

This is the main technical risk: grading needs a trustworthy reference.

- **Preflop:** static charts, either self-generated or from open sources. **Do not copy proprietary data** (GTO Wizard and similar).
- **Postflop:** the open-source `postflop-solver` by b-inary is written in Rust and powers Desktop Postflop and WASM Postflop.
  - **Verified in spike §9.2:** licensed AGPL-3.0-or-later, development suspended since October 2023. It still works with a dependency pin. See [spikes.md](spikes.md).
- **v1 approach (D2):** pre-solve a library for cash 6-max 100bb offline, in Taful's own compact format, downloaded per flop on demand (D11). The only real-time solving is rivers, with Taful's own river solver (D11).
  - Preflop: full charts for every position and common node (RFI, vs open, vs 3-bet, vs 4-bet, squeeze, blind vs blind).
  - Postflop: the main single-raised and 3-bet pot configurations (e.g. BTN vs BB, CO vs BB, SB vs BB, BTN vs SB 3-bet, CO vs BTN 3-bet), each on a representative subset of about 49 flops.
  - The final spot count is set by the solver spike (§9.2), as is the budget for node-locked bot-profile solves (D8).
  - Entries are keyed by `GameConfig` + preflop line + board, so new formats are additive. Each spot stores its solver exploitability, so "best" is never stricter than the reference.
  - Boards outside the subset: decided in D13 (only solved flops are dealt; coverage grows to all 1,755).

## 5. Architecture

### Stack
- **Rust + GPUI** (Zed's UI framework) for desktop, the same stack as Bardo. Use Context7 for up-to-date GPUI docs.
- **To verify:** GPUI's web and mobile status. As far as known it is desktop-only. That is why the core must be UI-free.

### Workspace layout

```
taful/
├─ crates/
│  ├─ taful-core/      # rules, hand evaluator, equity, EV-based scoring,
│  │                   # spaced repetition, progress model — zero UI deps
│  ├─ taful-solver/    # solver wrapper + pre-solved spot reader
│  └─ taful-content/   # lesson/drill loader (Markdown + TOML/JSON)
├─ apps/
│  ├─ desktop/         # GPUI
│  ├─ web/             # later: core via WASM + web UI
│  └─ android/         # later: core via uniffi + native UI
├─ content/            # lessons and drills as data
└─ docs/
```

Rule: poker logic lives only in `taful-core`. Web and Android must become "UI-only" work.

### Parametric game model (D1)
Every spot, drill, grade and simulation takes a `GameConfig` as input: table size, effective stacks, blinds/antes, rake structure, format (cash | mtt | sng) and blind structure/speed. Nothing hard-codes 6-max or 100bb. Rake is a parameter from the first slice, defaulting to zero. Tournament formats add an ICM layer between chip EV and $EV.

### Data: local-first, no backend in v1
- **SQLite** holds progress, history, the spaced-repetition queue, themes and layouts.
- Content and pre-solved spots ship with the app.
- The AI coach (wave 3) calls the Claude API directly. The key is stored in the Windows Credential Manager.
- **Sync-ready schema from day one:** ULID ids, `updated_at` and `deleted_at` on every row, enforced by a schema test from the first migration (D9).
- **Backups:** automatic rotating snapshots of the SQLite database (via SQLite's backup API, never a raw copy of a live file) to a user-chosen folder, plus manual export/import. Secrets are never included.
- **No backend until a second client exists** (web or Android) (D9). Then add a thin sync API; its hosting is decided at that point. Only user data syncs (progress, review logs, layouts/themes, hand history, settings), never content or pre-solved spots. Syncing the SQLite file through a cloud-drive folder is rejected.

### Public host (D10)
- Any public surface (web app, sync API) uses **`taful.filx.app`**. `taful.com` is taken, `taful.com.br` is dropped, and `taful.app` (unregistered as of 2026-11-05) is the preferred apex if one is needed. No domain is bought before web or mobile work starts, and that work waits until the desktop app is ready.
- To keep a later move cheap: the desktop app depends on no domain, the base URL is always configuration, the old host keeps serving 301/308 redirects, Android App Links list every host during a transition, and the web client treats browser storage as a cache of synced data (or ships export/import) because IndexedDB/localStorage are per-origin.

## 6. Look & feel

- **Mandatory poker-client look:** an oval table, felt, seats with avatars and stacks, chips, a dealer button, an animated pot and cards. It should be inspired by mainstream clients but not imitate any of them.
- **Default theme:** deep petrol-teal or slate-blue felt, amber/gold chips and accents, cream typography. **No red anywhere.** The 4-color deck uses gold or burnt orange for hearts and diamonds.
- **Everything is themeable through design tokens.** A theme is just a token file, and an in-app theme editor builds on that.
- **Customizable layout (D7):**
  - Panel, HUD and action-button positions and sizes are stored as versioned JSON, in resolution-independent units (relative units + grid cells).
  - First slice: built-in presets with save / load / reset, and themes as token files.
  - Right after: an **edit mode with drag-and-drop and grid snapping** over the same model, and an in-app theme editor. GPUI drag-and-drop was validated in spike §9.1.
  - No widget hard-codes a color; a lint/test rejects literal colors outside theme files.
- **i18n:** the codebase and docs are in English, and the UI ships in pt-BR first.

## 7. Quality bar

- Pragmatic SOLID / YAGNI / KISS / DRY. No duplicated logic.
- Minimal comments, only for genuinely hard, performance-critical or security-critical code.
- **Heavy automated testing**, especially in `taful-core`:
  - Hand evaluator correctness against known tables.
  - Equity calculations within tolerance.
  - Scoring thresholds.
  - Property-based tests where they fit.
- Multi-reviewer review: logic/correctness, security and architecture (high priority), style/conventions, and context/history.

## 8. Decisions

The original open questions were decided on 2026-11-04. Details, reasoning and implications are in [decisions.md](decisions.md).

| # | Question | Decision |
|---|---|---|
| D1 | Formats in scope | Full NLHE: HU/6-max/full ring, short to deep stacks, cash/MTT/SNG, all speeds, rake. PLO out. First slice: cash 6-max 100bb. |
| D2 | First spot library | Cash 6-max 100bb: full preflop charts, 3 SRP + 2 3-bet pot configs × ~49 flops (245 solves), sized by the solver spike. |
| D3 | Grading thresholds | Worse of absolute (bb) and relative (% of pot) EV loss; values in config. |
| D4 | Simulator depth | Spot mode and full-hand mode from the start, one engine. |
| D5 | Lesson content | AI-drafted Markdown + TOML; every number computed by `taful-core` and checked in CI; ~40 lessons first. |
| D6 | Spaced repetition | FSRS, with grades mapped to ratings. |
| D7 | Layout customization | Presets and token themes first; drag-and-drop editor right after. |
| D8 | Bots | GTO bot first; profiled bots as parametric deviations right after, graded by best-response EV against the profile, computed at build time. |
| D9 | Sync | No backend until a second client exists; rotating local backups meanwhile. |
| D10 | App name and host | Keep "Taful" (no conflicting live mark, checked 2026-11-05); INPI filing (classes 9, 41) before the first desktop release; host on `taful.filx.app`; no domain bought before web/mobile work. |
| D11 | Solver licensing | `postflop-solver` (AGPL) only in the offline build tool; own river solver in the app; library downloaded on demand in a compact format. |
| D12 | Preflop charts | Own offline preflop solver, realization calibrated from the postflop library. |
| D13 | Boards outside the subset | Deal only solved flops; grow coverage to all 1,755. |

### Still open
- ~~Formal trademark checks for "Taful"~~: checked 2026-11-05, no conflicting live mark found (D10). INPI filing (classes 9, 41) remains a step before the first desktop release.
- ~~Spot count and bot-profile solve budget~~: decided in D2 and D8.
- ~~Mapping strategy for boards outside the pre-solved flop subset~~: decided in D13 (deal only solved flops).
- ~~License and API of `fsrs-rs`~~: BSD-3-Clause, verified 2026-11-04 (see [spec.md](spec.md) §4.1).

## 9. Early spikes (before implementation)

1. **GPUI table render:** draw a table with seats, chips and cards, plus a drag-and-drop panel, on Windows.
2. **Solver:** solve one flop spot with `postflop-solver`, then measure time and memory and serialize the result.
3. **Hand evaluator + equity:** pick a crate or write our own, then benchmark it.

If any spike fails, revisit the stack before writing the spec.

All three spikes passed on 2026-11-04. Results are in [spikes.md](spikes.md) and [spikes/9.1-gpui-table.md](spikes/9.1-gpui-table.md).
