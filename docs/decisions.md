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
