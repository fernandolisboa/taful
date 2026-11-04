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
