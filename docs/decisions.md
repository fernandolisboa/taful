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

### Solver spike results (amended 2026-11-04)
Measured in spike §9.2 (details in [spikes.md](spikes.md)) on a 4-vCPU, 15GB machine, with exploitability ≤ 0.5% of the pot.

| Spot type | Tree | Per flop: time | Per flop: memory (16-bit) | Shippable size (truncated after turn) |
|---|---|---|---|---|
| SRP (SPR ≈ 18) | medium (2 flop sizes; 2 sizes + all-in on turn and river) | ~35–60 min | 8–14GB | ~15–30MB |
| 3-bet pot (SPR ≈ 4) | large (3 flop sizes; 3 sizes + all-in on turn and river; 2 raise sizes) | ~5 min | ~2GB | ~5–20MB |

A large tree for SRPs needs 63–211GB per flop and is out of reach. Turn and river subgames solve on demand in ~5s and < 0.1s.

**Proposed library for the first slice** (the scope of D2 is unchanged, so this sets only the numbers):
- **5 configurations × 49 flops = 245 solves:** 3 SRP configurations (BTN vs BB, CO vs BB, SB vs BB) on the medium tree, and 2 3-bet pot configurations (BTN vs SB, CO vs BTN) on the large tree.
- **Build budget:** ~85–150 hours for the SRPs plus ~8 hours for the 3-bet pots on a 4-core machine. A 16-core, 32GB+ machine should cut this to roughly 1–2 days (scaling not measured). A 32GB machine is the minimum for dry-board SRPs. The build is an offline, resumable batch job (one file per flop), run outside the app.
- **Shipped size:** ~4–5GB with turn-truncated trees, or ~25MB flop-only with on-demand turn solving (~5s each). Rivers are always solved on demand in the app (< 0.1s). Decided in D11: ship turn-truncated trees, and solve rivers with Taful's own river solver.
- The library is stored in Taful's own format, exported by the build tool, not in the solver's binary format.

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

---

## D8 — Bots with profiles

**Date:** 2026-11-04 · **Brief ref:** §8.8

### Decision
- **First slice:** the only opponent is the **reference (GTO) bot** that already powers full-hand mode (D4).
- **Right after the first slice:** profiled bots (nit, calling station, maniac, TAG, and more later), modeled as **parametric deviations from the reference strategy**, not as hand-written rules.
- **Grading against a profiled bot uses EV against that bot's actual strategy**, obtained by solving with the bot's strategy node-locked. The explanation shows both the GTO line and the exploitative line, and the EV gap between them.

### Why
Exploitative play is only teachable if the grade measures EV against the real opponent. A rule-based bot has no EV reference to grade against. Deviations from the reference keep bots consistent with the rest of the engine and make them tunable.

### Implications
- Profile = a data file of deviation parameters, for example: preflop range width multipliers per node (VPIP/PFR/3-bet), fold-to-c-bet and fold-to-raise adjustments, bluff/value ratio shifts, sizing preferences, and per-street aggression. Applied as a transform over the reference strategy, then renormalized.
- Each profile needs its own node-locked solves for the spot library (D2), multiplying solve cost. The solver spike (§9.2) decides how many profile × spot combinations are affordable, and whether the most common ones are pre-solved and the rest solved on demand.
- Profiles are seedable and deterministic like the reference bot (D4), so hands replay exactly.
- The same profile parameters later describe real opponents from imported hand histories (HUD stats literacy), so the model is reused, not duplicated.

### Solver spike results (amended 2026-11-04)
- A profile as defined above is a transform applied to the bot's whole strategy, so the bot is **locked at every node it acts**. Hero's EV against it is then a **best response**: one pass over the tree, with no solver iterations. The solver already runs this kind of pass (the exploitability check) every 10 iterations during each library solve, so its cost is a small fraction of a solve. This is reasoned from the solver's design and was not timed separately in the spike.
- Best responses need the full tree in memory (8–14GB for an SRP), so flop and turn best-response EVs per profile are computed **offline during the library build**, while each solved tree is still loaded, and stored next to the reference strategy. River best responses run on demand in the app, like river solves (< 0.1s).
- **Budget:** each profile adds a few minutes of build time per flop and less storage than the reference strategy (EVs for hero's hands only), instead of a full extra solve. All profiles × all D2 spots are affordable, so there is no need to split pre-solved and on-demand profiles.
- **Avoid partial locks** (bot locked only at some nodes and re-optimizing elsewhere). Each would be a full re-solve (~35–60 min per SRP flop), which would multiply the D2 build time by the number of profiles.

---

## D9 — Cross-device sync

**Date:** 2026-11-04 · **Brief ref:** §8.9

### Decision
- **No backend until a second client exists** (web or Android). Until then Taful is fully local.
- **Meanwhile:** automatic, rotating backups/exports of the SQLite database (consistent snapshot via SQLite's backup API, never a raw file copy of a live DB) to a user-chosen folder, plus manual export/import.
- **Hosting is decided when sync is built**, not now.

### Why
With one user on one device, sync delivers nothing and adds monthly cost and operations. The sync-ready schema from the brief (ULID ids, `updated_at` on every row, soft deletes) keeps adding a backend later an additive change.

### Implications
- The sync-ready schema rules are mandatory from the first migration and enforced by a schema test (every table has a ULID primary key, `updated_at` and `deleted_at`).
- Content and pre-solved spots are not user data and never sync; only progress, review logs (D6), layouts/themes (D7), hand history and settings do.
- Syncing the SQLite file through a cloud-drive folder is explicitly rejected (corruption risk with concurrent writers).
- Secrets (Claude API key) stay in the OS credential store and are never included in backups or sync.

---

## D10 — App name

**Date:** 2026-11-04 · **Brief ref:** §8.10

### Decision
Keep **Taful** as the name. A preliminary availability search was run on 2026-11-04 (results below). A formal check is still required before any public release.

### Preliminary findings (2026-11-04)
- **Web search:** no app, poker product or gaming brand named "Taful" was found. Existing uses are unrelated: a Portuguese university student music group (Tuna Académica de Farmácia, Lisbon), an archived C "crackme" repo on GitHub (`LorenzKofler/Taful`), a small personal GitHub account `taful`, and a Facebook page.
- **App stores:** no "Taful" listing found on Google Play or the App Store in search results.
- **Domains:** `taful.com` is taken (confirmed by the user). Other TLDs were not verified here (RDAP/WHOIS blocked from this environment).
- **Trademarks (INPI, USPTO, EUIPO):** not verified. These databases require interactive searches that could not be run here.

### Why
Low collision risk in the poker/education space based on what could be checked, and the name carries the intended meaning (archaic Portuguese for a habitual gambler).

### Implications
- Before publishing, complete the INPI search in Nice classes 9 (software) and 41 (education/entertainment), and USPTO/EUIPO if publishing outside Brazil.
- Keep the product name out of crate names, identifiers and storage paths where practical (e.g. one constant plus i18n strings), so a rename stays cheap if a conflict appears.
- Note for marketing copy: "taful" means a habitual gambler, which may need care with store policies on gambling-related apps. Taful has no real-money play, which should be stated clearly in store listings.

### Domain (amended 2026-11-04)
- **Use `taful.filx.app`** (a subdomain of a domain the user already owns) whenever Taful needs a public host. No new domain is bought now.
- `taful.com.br` is rejected (not available through the user's registrar, Vercel). `taful.app` may be bought later; not decided.
- **Moving to an apex domain later is cheap if these rules hold from the start:**
  - The desktop app depends on no domain at all (local-first, D9).
  - The base URL is configuration, never hard-coded, for the web app, the sync API, OAuth redirect URIs and any links in content.
  - The old host keeps serving permanent (301/308) redirects after a move.
  - **The one real cost is the web app's browser storage:** IndexedDB/localStorage are per-origin, so data stored on `taful.filx.app` is invisible on `taful.app`. The web client must treat browser storage as a cache of synced data, or ship an export/import path, before any move.
  - Android App Links need `assetlinks.json` on every host the app claims; list both during a transition.

---

## D11 — Solver licensing and in-app solving

**Date:** 2026-11-04 · **Ref:** spike §9.2 ([spikes.md](spikes.md))

### Decision
- `postflop-solver` (AGPL-3.0) is used **only in the offline library build tool**, never linked into the app.
- The app solves rivers on demand with **Taful's own river-only solver** in `taful-solver`, under the project's own license.
- The library ships **turn-truncated trees** (~4–5GB for the D2 library) in Taful's own format, so the app needs no turn solver.

### Why
The user prefers to keep the option of distributing Taful someday without having to publish it under AGPL. A river-only solver is small: one street, no chance nodes, and hand strengths are precomputable with the `rs_poker` evaluator. The spike measured river subgames at < 1MB and < 0.1s.

### Implications
- The offline build tool (which links `postflop-solver`) is a separate binary outside the shipped app. It may be AGPL; its output (strategy and EV data) is not covered by the license.
- The own river solver is validated against `postflop-solver` inside the offline tool: for a fixed set of river spots, exploitability and per-hand EVs must agree within tolerance. This test lives with the build tool, not in the app.
- `taful-solver` exposes a trait for subgame solving, so swapping the implementation later is a local change.

### Library delivery (amended 2026-11-04)
The user expects a web version, and download size matters. The ~4–5GB library is therefore **not bundled with the app**:
- **On-demand download:** one file per configuration + flop, served from a CDN. Web clients fetch a file when a hand first reaches that spot and cache it. Desktop does the same, and can also offer optional prefetch packs (e.g. "BTN vs BB SRP").
- **Compact format:** Taful's library format quantizes strategy frequencies (e.g. 8-bit) and EVs, and may drop nodes that are reached with negligible frequency. The estimated reduction is 2–4x vs the solver's 16-bit trees, to be measured when the format is built.
- **An own turn solver was considered and rejected for now.** Turns took ~5s natively on 4 cores, and would take an estimated 10–30s in single-threaded WASM, which is a wait on every turn in full-hand mode. The subgame-solver trait keeps it possible later.
- The client needs a library host (an endpoint under the configurable base URL, D10) and an offline path: spots already cached keep working offline, and uncached spots end the hand early with a message, like lines outside the library (D4).

---

## D12 — Source of preflop reference charts

**Date:** 2026-11-04 · **Ref:** [spec.md](spec.md) §6, Q1

### Decision
Preflop charts (frequencies and per-combo action EVs) are produced by **Taful's own offline preflop solver** in `tools/library-build`:
- DCFR over the full preflop tree of the `GameConfig` with a fixed sizing set.
- Postflop is abstracted at terminal nodes as `pot × equity × realization`, with realization factors per position, pot type and SPR bucket.
- Realization factors are **calibrated from the postflop library** (each combo's root EV vs pot × raw equity, averaged over the flop subset), then the charts are re-solved. The build tool reports the chart diff per iteration.
- Nothing is copied from proprietary sources.

### Why
D3 grades by EV loss, so preflop needs per-combo EVs, not just frequencies; frequency-only charts would break the single grading ruler. An own solver also keeps the data free of licensing questions and makes every `GameConfig` (D1) a build-manifest change.

### Implications
- Build order: preflop v0 → postflop library (ranges from v0) → calibrate → preflop v1. Preflop charts block M3; the postflop library uses them as input ranges.
- Preflop EVs are model-based approximations; their accuracy is bounded by the realization model. The exploitability of each node within the abstracted game is stored, and "best" uses it as tolerance (D3).
- Push/fold and ICM charts for tournaments stay a separate, exact computation.

---

## D13 — Boards outside the solved flop subset

**Date:** 2026-11-04 · **Ref:** [spec.md](spec.md) §7.2, §7.4, Q2

### Decision
- **Postflop deals only flops the library has solved.** When a hand reaches a solved configuration, the flop is drawn from the solved canonical flops, each with a random suit isomorph, weighted by the number of real flops it represents and conditioned on the hole cards already dealt. Grading stays exact.
- **Coverage grows to all 1,755 canonical flops** (100% of the 22,100 real flops). The first build solves ~49 flops per configuration to unblock M4; later builds add flops in order of real-flop frequency until the library is complete.

### Why
Mapping a random flop to the "nearest" solved flop changes hero's real hand strength, so grades would be wrong exactly where they matter. Ending the hand on unsolved flops would stop ~97% of hands at the flop. 49 canonical flops (~620 real flops) are too little variety for a trainer used daily, which is why full coverage is the goal rather than a fixed subset.

### Implications
- The flop subset is a build-manifest parameter; adding flops is a build job, never a code change.
- Build budget for full coverage (5 configurations × 1,755 flops) is several hundred CPU-hours. Proposed machine: the author's PC (Ryzen 9 9950X3D, 16 cores, 32GB), running at low priority at night/weekends; the build must be resumable and pausable. Benchmark pending to replace the estimates.
- 32GB RAM fits one SRP solve at a time (up to ~14GB in 16-bit). Shipped size grows proportionally (to be measured with the compact format, §7.5).
