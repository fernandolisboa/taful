# Taful — Decision Log

Decisions taken while grilling the open questions in [brief.md](brief.md) §8. Each entry records the decision, why, and what it implies for design. Later entries may supersede earlier ones; when that happens, the old entry is marked as superseded, not deleted.

---

## D1 — Game formats in scope

**Date:** 2026-11-04 · **Brief ref:** §8.1

### Decision
Taful covers the full No-Limit Hold'em landscape. Wave assignment is an implementation detail; none of the following is optional for the product:

- **Cash games:** heads-up, 6-max and full ring (9-max).
- **Stack depths:** short (≈20–40bb), standard (100bb) and deep (150–200bb+).
- **Tournaments:** MTT, including ICM, push/fold, short-stack play and bubble/final-table dynamics.
- **Sit & Go:** single-table and the ICM-heavy endgame that comes with it.
- **Tournament speed / structure:** slow, regular, turbo, hyper-turbo. Speed changes blind-level duration and therefore effective stack depth over time, which changes strategy.
- **Rake:** modeled and taught. Rake changes reference strategies (especially preflop and small pots) and is its own learning topic (rakeback, rake-adjusted win rate, why rake decides profitability at low stakes).

**Out of scope:** Omaha / PLO. Revisit only after the NLHE product is complete and in use.

### Why
The user wants a complete coach, not a narrow trainer. The original brief's "6-max 100bb only" framing was too small for that goal.

### Implications
- The domain model in `taful-core` is parametric from day one: a `GameConfig` (table size, effective stacks, blinds/antes, rake structure, format = cash | mtt | sng, blind structure / speed) is an input to every spot, drill, grade and simulation. No hard-coded 6-max or 100bb anywhere.
- The pre-solved spot library is keyed by `GameConfig` + preflop line, so adding a format is a data job, not a code change.
- Tournament formats need an ICM / equity-model layer between chip EV and $EV. EV-loss grading must therefore be expressed in the right unit per format (bb for cash, chip or $EV for tournaments). See D3.
- Sequencing (which format ships first) is left to planning. Working assumption: cash 6-max 100bb is the first vertical slice because it exercises the whole loop with the cheapest reference data, while the generic model above keeps every other format a data addition.
- Rake is a `GameConfig` parameter from the first slice, defaulting to zero, so it never requires a migration.

---

## D2 — First pre-solved spot library

**Date:** 2026-11-04 · **Brief ref:** §8.2

### Decision
The first reference library is a **vertical slice: cash 6-max, 100bb**.

- **Preflop:** full charts for every position and every common node (RFI, vs open, vs 3-bet, vs 4-bet, squeeze, blind vs blind).
- **Postflop:** the main single-raised-pot and 3-bet-pot configurations (e.g. BTN vs BB, CO vs BB, SB vs BB, BTN vs SB 3-bet, CO vs BTN 3-bet), each solved on a representative flop subset of about 49 flops.
- **Final spot count is set by the solver spike** (brief §9.2): time and memory per solve decide how many configurations and how deep a bet tree fit in the build budget.

Every other format from D1 is added later as more data in the same library.

### Why
It exercises the whole closed loop (simulate → grade → review → drill → repeat) with the cheapest reference data, before investing in formats that need extra machinery such as ICM.

### Implications
- Library entries are keyed by `GameConfig` + preflop line + board, so new formats are additive (D1).
- Boards outside the subset need a mapping strategy (nearest strategically-equivalent flop, or on-demand solve later). To be decided in the spec.
- Push/fold and ICM charts for tournaments are cheap to compute and can be generated independently of the postflop library when the MTT/SNG work starts.

---

## D3 — EV-loss grading thresholds

**Date:** 2026-11-04 · **Brief ref:** §8.3

### Decision
Each decision is graded on **two scales at once, absolute (bb) and relative (% of the pot before the decision)**, and receives the **worse** of the two grades.

| Grade | Absolute EV loss | Relative EV loss |
|---|---|---|
| best | action played by the reference strategy at ≥ 10% frequency, or EV within solver tolerance of the max | — |
| correct | < 0.1bb | < 2% of pot |
| inaccuracy | < 0.5bb | < 5% of pot |
| mistake | < 2bb | < 15% of pot |
| blunder | ≥ 2bb | ≥ 15% of pot |

These values are starting points. They live in configuration, not in code, and will be calibrated against real sessions.

### Why
Absolute bb alone treats a mistake in a 5bb pot the same as one in a 200bb pot, so small-pot errors almost never register. Percentage of pot alone hides expensive errors in big pots (3% of 300bb is 9bb). Taking the worse of both catches both failure modes.

### Implications
- `taful-core` exposes a pure function `grade(ev_loss, pot, thresholds) -> Grade`, property-tested for monotonicity (more EV loss never improves the grade).
- Tournaments (D1) run the same thresholds on **$EV from the ICM layer, converted back to bb-equivalents**, so one ruler works across formats.
- Solver tolerance (exploitability of the pre-solved spot) must be stored with each spot, so "best" is never stricter than the reference itself.
- Mixed strategies: choosing a low-frequency action that the solver still plays is not penalized beyond its actual EV loss.

---

## D4 — Simulator depth

**Date:** 2026-11-04 · **Brief ref:** §8.4

### Decision
The simulator ships with **both modes from the start**:

- **Spot mode:** a single decision point (game state, action history and sizing given). Fast to repeat; the default unit for drills, spaced repetition and "drills from your own mistakes".
- **Full-hand mode:** the hand is played street by street. Villain(s) act by sampling the reference strategy, and **every hero decision is graded** with the D3 thresholds. The review shows the grade of each decision plus the hand's total EV loss.

### Why
Spot mode trains individual decisions efficiently; full-hand mode trains planning a hand across streets and living with the consequences of a line. Both are needed for a complete coach (D1).

### Implications
- A pre-solved flop spot already contains the full turn and river tree, so full-hand mode reuses the same library (D2) without extra solves.
- Hero's bet sizes are **limited to the sizings in the solved tree**. The UI shows those as the available sizing buttons. Free sizing needs on-demand solving and is deferred.
- Preflop in full-hand mode uses the preflop charts; postflop is only available for lines that reach a pre-solved spot. Lines that leave the library end the hand early with a clear message, and are logged as candidates for new solves.
- Both modes share one engine: spot mode is full-hand mode started at a given node and stopped after one hero decision. No duplicated logic.
- Villain sampling must be seedable, so any hand can be replayed exactly (review, bug reports, tests).

---

## D5 — Lesson content: authorship and correctness

**Date:** 2026-11-04 · **Brief ref:** §8.5

### Decision
- **Authorship:** lessons are drafted by AI (Claude) and committed as data.
- **Format:** one Markdown file per lesson with TOML front matter (id, title, prerequisites in the skill tree, tags, glossary terms, the quiz/drill that unlocks the next node). Source language is en-US; pt-BR is shipped through the i18n pipeline.
- **Correctness:** every number a lesson states (outs, pot odds, MDF, alpha, equity, chart frequencies, EV figures) is **not hand-written**. It is a reference to a computation (`{{ calc: pot_odds(pot=10, bet=5) }}`-style placeholder or a fixture) that `taful-core` evaluates. CI renders every lesson and fails if a computation errors or a pinned expectation drifts.
- **Size of the first track:** about 40 lessons, from 101 up to intermediate cash 6-max 100bb play (matching D2). Further tracks (MTT, SNG, deep stacks, rake) follow the format rollout from D1.

### Why
The user is still learning, so a manual review would not catch technical errors. Binding every number to tested code makes the content verifiable; prose quality can be reviewed by reading. A human expert reviewer can be added later without changing the pipeline.

### Implications
- `taful-content` needs a small, safe expression language (whitelisted pure functions from `taful-core`, no arbitrary code) and a renderer used both by the app and by CI.
- Claims that cannot be computed (strategic heuristics, "why" explanations) must cite the reference strategy for that spot when one exists, so the text never contradicts the solver.
- Lessons are content-addressed / versioned so progress and spaced-repetition items survive lesson edits.

---

## D6 — Spaced-repetition algorithm

**Date:** 2026-11-04 · **Brief ref:** §8.6

### Decision
Use **FSRS** (Free Spaced Repetition Scheduler), via the official Rust implementation (`fsrs-rs`; license and API to verify during setup).

The D3 grade maps directly to the FSRS rating:

| D3 grade | FSRS rating |
|---|---|
| best | Easy |
| correct | Good |
| inaccuracy | Hard |
| mistake, blunder | Again |

### Why
FSRS schedules fewer reviews for the same retention than SM-2, can optimize its parameters from the user's own review history, and is the current default in Anki. Integration effort is about the same as SM-2, and choosing it now avoids converting history later.

### Implications
- A review item is a **spot template** (game state, positions, action history, sizing), not a fixed hand. Each review instantiates it with a randomized board/hand from the same strategic family, as the brief requires for "drills from your own mistakes".
- Full-hand mode (D4) feeds the queue per decision, not per hand: each graded mistake becomes its own item.
- Calc-trainer exercises and lesson quizzes use the same queue and the same rating mapping (timed drills can downgrade a slow correct answer to Hard).
- The full review log (item, timestamp, rating, elapsed) is stored in SQLite so FSRS parameters can be re-optimized locally, and so the log syncs later (sync-ready schema).

---

## D7 — Layout customization sequencing

**Date:** 2026-11-04 · **Brief ref:** §8.7

### Decision
1. **First slice:** layout is data from day one (JSON: panel, HUD and action-button positions and sizes). The app ships **built-in presets** with save / load / reset, and **themes as design-token files** (default petrol-teal/slate felt, amber/gold accents, cream type, no red, 4-color deck with gold/burnt orange).
2. **Right after:** the **drag-and-drop edit mode with grid snapping**, built on top of the same JSON model. The in-app theme editor follows the same pattern on top of token files.

### Why
Since layout and theme are pure data, the editor is a UI over an existing model and adds nothing that must be redone. Waiting until the set of panels stabilizes avoids rebuilding the editor as screens change.

### Implications
- The layout schema must be versioned with migrations, so presets and user layouts survive new panels being added.
- Layouts are resolution-independent (relative units + grid cells), so one preset works on different window sizes, and later on web.
- GPUI drag-and-drop is still validated early in spike §9.1, so the deferred editor carries no hidden technical risk.
- No widget hard-codes a color: everything reads tokens, enforced by a lint/test that rejects literal colors outside theme files.
