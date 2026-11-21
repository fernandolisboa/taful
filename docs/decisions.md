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
- Boards outside the subset need a mapping strategy (nearest strategically-equivalent flop, or on-demand solve later). To be decided in the spec. **Superseded by D13** (deal only solved flops; no mapping).
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

### Theme editor validation (amended 2026-11-06)
- The in-app theme editor runs the same checks as the built-in themes (WCAG AA for text and grade colors, no red, grade colors distinct) **live, as warnings only**: the user can save a theme that fails them.
- Built-in themes keep failing CI on any violation; the relaxation applies only to user themes.

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

### Choosing profiled opponents (amended 2026-11-06)
- **Spot mode:** the user picks the opponent's profile (reference or any profile) in the controls panel.
- **Full-hand mode:** each bot seat can carry its own profile (for example two nits, one maniac, two TAGs); a single profile for every seat is the simple case.
- Graded mistakes against a profile land in the FSRS queue and in leak detection **keyed by profile**, so the user sees whether they adjust to each opponent type.
- Postflop is heads-up in every D2 configuration, so only the remaining opponent's profile matters there and its best responses come precomputed from the build. **Preflop at a mixed table** needs a best response against that seat combination, computed on demand by the own preflop solver (D12). Its cost is unmeasured; the mixed table ships as a separate step after the single-profile one, starting with that measurement. *Measured in #29:* about 0.16 s per table (median, release, 4 vCPU), computed once per table inside hero's action; spec §9 has the numbers and the table key decisions are filed under.

### Build sequencing (amended 2026-11-06)
- **Per-profile best responses can only be computed inside the solve.** Library files are turn-truncated and pruned (spec §7.5), and a best response against an exploitative profile needs the full tree down to the river, including the lines the reference strategy rarely takes. A separate pass over stored `.tafl` files cannot produce them.
- **Rule:** any per-flop output that needs the full solved tree (profile best responses today, anything similar later) is computed by the build tool during the solve. Profiles are calibrated on the first-slice library before the coverage run, because changing a profile afterwards means re-solving.
- **Sequencing:** the first-slice library (245 solves, D2/D13) is finished without profiles, so the slice is playable against the reference bot. **Coverage growth toward all 1,755 flops (8,775 solves) is blocked** until the profiles exist and the build tool computes their best responses inline. That run then re-solves the 245 first-slice files as part of the full set, once.
- **Why:** the 245 files cost ~1.5 days of build time and make the app usable weeks before the profiles exist; growing coverage first would have to be redone at full-coverage scale (~430–700 h).

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
- (Done 2026-11-05, see Availability check.) Before publishing, complete the INPI search in Nice classes 9 (software) and 41 (education/entertainment), and USPTO/EUIPO if publishing outside Brazil.
- Keep the product name out of crate names, identifiers and storage paths where practical (e.g. one constant plus i18n strings), so a rename stays cheap if a conflict appears.
- Note for marketing copy: "taful" means a habitual gambler, which may need care with store policies on gambling-related apps. Taful has no real-money play, which should be stated clearly in store listings.

### Availability check (amended 2026-11-05)
Closes spec Q3 for name conflicts and supersedes the trademark items in Decision and Implications above. Read-only lookups were run on 2026-11-05 from the author's PC (registry RDAP/WHOIS, trademark databases, store searches). Nothing was registered or bought.

**Trademarks**

| Source | Result |
|---|---|
| USPTO (tmsearch) | 0 hits for word mark "taful", live or dead. |
| TMview (EU, UK, BR and other offices) | Only two marks contain TAFUL as a whole word, both Brazilian (below). Other hits are different words (METAFUL, VITAFUL, DATAFUL, TAFULAN…). None at EUIPO. |
| WIPO Global Brand DB | Exact "taful": 1 result, the expired BR mark below. |
| INPI 814715036 "TAFUL" | Taful Magazine Ltda ME, filed 1989, old class 25:10. **Extinct** (expired 2000-12-04). |
| INPI 927608448 "PADRÃO TAFUL" | Individual (MEI), filed 2022, Nice class 25 (clothing). **In force.** Different class and market; no apparent conflict with classes 9, 28 or 41. |

No live "TAFUL" mark was found in Nice classes 9, 28 or 41 in the BR, US, EU or WIPO databases searched.

**Domains**

| Domain | Status |
|---|---|
| taful.com | Registered since 2005 (GoDaddy), expires 2027-02-04, transfer-locked. "Coming soon" placeholder, no sale offer. |
| taful.net | Registered since 2005 (GMO/Onamae, Japan), expires 2034. Nothing hosted, no sale offer. |
| taful.app, .dev, .io, .ai, .gg, .games, .com.br | Not registered (RDAP 404 / NXDOMAIN; registro.br reports `taful.com.br` available). Premium pricing not checked. |

**Stores:** no app named "Taful" on Google Play (BR) or the Microsoft Store (US). The App Store was not rechecked (none found on 2026-11-04).

**Outcome (decided 2026-11-05):**
- **Taful is the final name.** Spec Q3 is closed for name conflicts.
- **INPI filing is deferred until the desktop app is ready**, and is a required step before the first public (desktop) release: file in Nice classes 9 and 41 (28 was checked for conflicts only, not filed). Brazil is first-to-file, so file before the name is public to reduce the risk of someone else filing first. USPTO/EUIPO only if the app is published outside Brazil.
- **No domain is bought now.** No web or mobile work happens before the desktop app is ready, and the desktop app needs no domain (D9). `taful.app` stays the preferred apex if one is needed later (`taful.filx.app` meanwhile, see below); `.com`/`.net` are taken and not listed for sale.
- Re-run this check right before filing, since availability can change.

### Domain (amended 2026-11-04)
- **Use `taful.filx.app`** (a subdomain of a domain the user already owns) whenever Taful needs a public host. No new domain is bought now.
- `taful.com.br` is rejected (the user's registrar, Vercel, cannot register it, although registro.br lists it as available on 2026-11-05). `taful.app` is the preferred apex if one is ever needed; nothing is bought before web or mobile work starts (see Availability check).
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
- **Coverage grows to all 1,755 canonical flops** (100% of the 22,100 real flops). The first build solves ~49 flops per configuration to unblock M4; later builds add flops in order of real-flop weight (ties broken by stratified sampling over texture classes) until the library is complete.

### Why
Mapping a random flop to the "nearest" solved flop changes hero's real hand strength, so grades would be wrong exactly where they matter. Ending the hand on unsolved flops would stop ~97% of hands at the flop. 49 canonical flops (~620 real flops) are too little variety for a trainer used daily, which is why full coverage is the goal rather than a fixed subset.

### Implications
- The flop subset is a build-manifest parameter; adding flops is a build job, never a code change.
- Build machine: the author's PC (Ryzen 9 9950X3D, 16 cores / 32 threads, 32GB), at low priority at night and on weekends; the build must be resumable and pausable. Benchmarked 2026-11-04 (spec §7.3): SRP medium ~4–7 min per flop, 3-bet pot large ~1.5 min. The first slice takes ~16 h; full coverage takes ~430–700 h, about 3–4 weeks 24/7 or 6–10 weeks of nights and weekends.
- 32GB RAM fits one SRP solve at a time (up to ~14GB in 16-bit). Shipped size grows proportionally (to be measured with the compact format, spec §7.5).

---

## D14 — Desktop installer for personal use

**Date:** 2026-11-06 · **Spec ref:** §19 Q3, Q5

### Decision
- Once the first slice is minimally usable (first-slice library finished and installed), CI produces a **simple Windows installer** so the author can install Taful himself.
- **Unsigned**. SmartScreen warns on first run ("More info → Run anyway"); that is accepted for personal use.
- *Amended 2026-11-20 (D18):* auto-update is no longer part of the full release: the author wants it for his own use, so it ships now from GitHub Releases.
- The postflop library is **not** bundled: it stays in a folder the user picks in settings (`LocalDir`, spec §7.6).
- A **full release** (code signing, library downloaded from a host (Q5), INPI filing (D10)) happens only if the author decides to sell the app. It is tracked as a placeholder, not planned work.

### Why
The only user today is the author, on his own PC. Signing certificates, hosting and trademark filing cost money that is only worth spending if the app is sold.

### Implications
- The installer format must not require a signature to install (rules out MSIX); an MSI or a setup executable built in CI fits.
- Q5 (library host) and the INPI filing stay deferred until a sell decision.

---

## D15 — Profiles play their own preflop range postflop

**Date:** 2026-11-07 · **Spec ref:** §9 simplifications, #48

### Decision
- The postflop build computes each listed profile's preflop range along the line (the profile's transform of the chart, the same function the app's profiled bots sample). Each profile's best-response and reach passes start the bot from that range instead of the reference's, and the library stores it per profile section (format v4). The app starts a profiled bot from it.
- **Seeding** (`solve.profile_seed`, opt-in): combos a profile brings to the flop that the reference never does join the solve at a tiny weight. In CFR a hand's regrets do not depend on its own reach, so seeded combos get a real strategy while the reference barely moves. The reference's own rows and header ranges never include seeded combos. Without a seed, the profile's range is restricted to the reference's combos, and a bot holding another combo ends the hand with `bot-off-range`.
- **Rejected:** a separate solve per profile from its own root range (#48 option 2). It is the most faithful, but costs ~5x the build (2,150–3,500 h for the 8,775 solves).

### Cost (measured 2026-11-07, cloud, 4 vCPU)
- Solver memory and time scale with the number of hands, and loose profiles roughly double the ranges. Memory estimates over the 49 first-slice flops, all four profiles seeded, mean and max per flop: btn-bb-srp 9.5/14.6 → 17.3/25.6 GB (1.82x); co-bb-srp 7.7/11.8 → 14.0/21.1 GB (1.82x); sb-bb-srp 10.6/16.0 → 16.4/24.2 GB (1.54x); btn-sb-3bp 3.5/5.5 → 9.0/13.8 GB (2.56x); co-btn-3bp 4.8/7.5 → 10.5/16.2 GB (2.18x).
- One real solve (`btn-sb-3bp` AcKdQh, large tree): reference only 1,158 s, 340 iterations, peak 4.7 GB, 3.2 MB. Seeded with four profiles 2,631 s, 300 iterations (2.6x per iteration), best responses 125 s, peak 13.2 GB, 23.5 MB (14.7 MB with unseeded profiles, #27).
- Rough full-coverage estimate with seeding: ~1.9x the reference build, so ~800–1,350 h instead of 430–700 h, with dry SRP flops peaking at ~26 GB on the 32 GB build PC.

### Implications
- Whether the full-coverage run (#31) enables `profile_seed` is decided with that cost in view. Without it, the code still runs every profile's best response from its own range within the reference's combos.
- Manifests that list profiles hash a profile-sections tag, so a build resumed across this change never mixes v3 and v4 files.


---

## D16 — Postflop solve accuracy target

**Date:** 2026-11-12 · **Ref:** #31, PR #64

### Decision
- The postflop library build stops each solve at **exploitability ≤ 1.0% of the pot** (was 0.5%).
- Files already built at 0.5% (76 of the 245 first-slice solves in the profile-seeded re-solve) are kept, not rebuilt. They are more accurate, and each file stores its own exploitability.

### Why
- Measured on 6 seeded solves with the per-check trace (PR #64): reaching 1.0% took 44% less solve time than 0.5% in total (per solve 32–50%); 0.75% saved 29%. Solving is ~93% of build time, so the build runs almost twice as fast.
- The finest grade bands (D3) split at 2% of the pot, so a 1.0% reference stays below them. Only decisions right at a band edge can move one grade. "Best" already uses each spot's stored exploitability as its tolerance (D3).
- Faster hardware was not the alternative: a thread-scaling test (5.5x at 32 threads vs 4) showed the solver is not memory-bandwidth bound, so a second DIMM would not speed it up.

### Implications
- `target_exploitability_pct` is part of the manifest hash, so files built before and after this change carry different hashes in their headers. Output paths and existence-based skipping do not depend on the hash, so the build resumes in the same folder.
- Exploitability is not monotone across checks; the build stops at the first check at or below the target, as before.


---

## D17 — Bot profiles calibrated to HUD targets

**Date:** 2026-11-16 · **Ref:** #31, PRs #66, #67

### Decision
- The four bot profiles are tuned automatically, not by playing: `tools/profile-stats` measures what a HUD would show for each bot and bisects the knobs toward `tools/profile-stats/targets.toml`. Fernando approved the target table (2026-11-15). It uses the usual HUD reads for each player type in 6-max cash at 100bb, not anyone's feel.

| Profile | VPIP | PFR | 3-bet | Fold to flop c-bet | Flop AF |
|---|---|---|---|---|---|
| nit | 13 ±2 | 10 ±2 | 3 ±1 | 60 ±5 | 2.0 ±0.5 |
| tag | 22 ±2 | 18 ±2 | 7.5 ±1.5 | 45 ±5 | 3.0 ±0.5 |
| calling-station | 45 ±5 | 10 ±3 | 2 ±1 | 22 ±7 | 0.8 ±0.3 |
| maniac | 50 ±5 | 40 ±5 | 18 ±3 | 28 ±6 | 5.0 ±1.0 |

- Tuned knobs: `threebet_mult`, `vpip_mult`, `pfr_mult`, `fold_to_cbet` and `aggression`. `fold_to_raise`, `bluff_value_shift` and sizing have no HUD target and stay as they were. Fold to 3-bet and flop c-bet follow from the tuned knobs and are only reported.
- `MAX_MULT` went from 5 to 10 (PR #66). The tree has no limps, so a station gets into pots by calling raises, and it needs `vpip_mult` ≈ 7.5 to reach VPIP 45 with PFR 10.

### Result (2026-11-16, on the 245 first-slice files built with the old knobs)
- Preflop is exact (published chart, bot in each seat vs five reference players): nit 13.1/10.0/3.2, tag 21.9/18.0/7.5, station 45.3/10.0/2.0, maniac 49.5/40.0/18.1 (VPIP/PFR/3-bet).
- Flop numbers are estimates. The reference stores no rows for the seeded combos a loose bot adds (D15), so those combos keep the strategy the old build stored: about a third of station and maniac flop decisions. Estimates: fold to c-bet / AF nit 60.0/2.00, tag 44.6/3.00, station 22.1/0.80, maniac 26.9/5.00.
- Before: station PFR was 20.4 (pfr_mult 1.3 made it a loose opener) and maniac flop AF was 8.8.

### Implications
- The 245 are re-solved with the new profiles into a fresh folder. Afterwards `profile-stats postflop --stored` measures the flop numbers exactly. A stat outside its band means one more tune and re-solve before the library grows.
- The profiles are frozen once that check passes (#31 acceptance).
- A profile can make lines its table never plays (the station never folds to a raise). Their best-response EVs are all zero, so spot drills and review items against that profile skip those nodes. A full hand only gets there if a bot samples a near-zero action; it is graded as before rather than ended.

---

## D18 — Auto-update from GitHub Releases

**Date:** 2026-11-20 · **Ref:** #68 (split out of #34), D14

### Decision
- The desktop app updates itself from **GitHub Releases** of `fernandolisboa/taful` (public, so no token). The library host (Q5) stays deferred; it is not needed for the app.
- **CI publishes the releases.** Every push to `main` that passes CI publishes a pre-release `v<version>-<run>.alpha`. A manual CI run on `main` with `release: beta` publishes `v<version>-<run>.beta`, and `release: stable` publishes `v<version>` (the workspace version, once; bump it afterwards). The run number leads the pre-release, so pre-releases of one version sort by build whatever their kind (an alpha built after a beta is newer). PR builds are `<version>-0.pr<n>.<run>`: they sort below every published pre-release of that version, so an installed PR build still updates. Only the 10 newest alphas are kept.
- Each release carries `taful-setup-<version>.exe` and `taful-setup-<version>.exe.sha256`. The app checks the hash before it runs the installer. The installer stays unsigned (D14), so the hash guards against a broken download, and HTTPS to GitHub is the trust anchor.
- **Channels:** Stable (final versions), Beta (beta and rc plus stable), Alpha (everything). Until the user picks one, the app follows the channel of the build it runs, so an installed alpha keeps getting alphas.
- **Settings** (Settings › Updates): check automatically (on, at startup and every 6 hours), download automatically (on), install on close (off), notifications Notify (a card in the corner) or Silent (no card; manual checks still report in the tab). A version can be skipped. Never a downgrade, even when the channel narrows.
- **Installing:** the app runs the downloaded setup with `/SILENT` (`/VERYSILENT` on close) `/SUPPRESSMSGBOXES /NORESTART /WAITPID=<app pid>` and quits. Setup waits up to 2 minutes for the app to exit (else it cancels and the app offers the update again), installs, and with `/RELAUNCH=1` (Restart and update) starts the new version. Installing on close does not reopen the app.
- **Tray icon:** a notification-area icon with Open, Check for updates (it becomes "Restart and update to <version>" once one is ready) and Quit. Closing the window still quits the app; the icon lives as long as the app.
- **Settings tabs:** settings are grouped in tabs (General, Appearance, Study, Data, Updates) instead of one long page.

### Why
The author installs every CI build by hand. GitHub Releases is free, already where the builds come from, and needs no new host. Releasing from CI keeps one source of truth for versions; alphas from every merge give the author each change without a manual step.

### Implications
- The workspace version became 0.1.0. A stable release needs a version bump after it, or later alphas sort below it (the release job warns).
- The first auto-updating build has to be installed by hand once (from the newest alpha release).
- Code signing is still out of scope (D14). An unsigned installer downloaded by the app carries no Mark of the Web, so SmartScreen does not prompt on updates.


---

## D19 — Visual styles

**Date:** 2026-11-20 · **Ref:** D7

### Decision
- The desktop app has selectable **visual styles**: **Classic** (the look it shipped with), **Salão**, **Cockpit** and **Baralho**. A style is a whole look: navigation, components, fonts and art. It is picked in Settings › Appearance › Visual style (setting `ui.style`) and applies at once.
- Salão, Cockpit and Baralho are built exactly as the mockups the author approved on 2026-11-20, wired to real data (the mockups' numbers are illustrative).
- Every style but Classic has **fixed colors**, in `content/styles/<id>.json`: a full set of theme tokens plus the extra colors that style needs. Themes and the theme editor (D7) apply to Classic only; the user's theme choice is kept while another style is active.
- The house rules hold in every style: no red, the four-color deck, the grade colors. A test checks every style file for red and for four different suit colors and five different grade colors. A style's colors are an approved design, so the contrast and grade-distance warnings of user themes (D7) are not applied to them.
- A style that is not finished is not offered in settings.

### Why
The author found the app flat, liked all three directions that were proposed, and wants to switch between them rather than pick one.

### Implications
- One module per style in `apps/desktop/src/<id>/`, all with the same set of hooks (`style.rs` lists them). Classic is the reference; another style starts as a re-export of Classic and replaces what it draws differently, so shared code never names a style.
- D7 still holds: no color literal outside `theme.rs`. A style's colors are data, like a theme's.
- A screen that has no mockup takes the style through the shared widgets. A new shared widget needs a look in every style.
- Fonts and art are embedded in the binary from `apps/desktop/assets/<id>/`, with their licenses next to them.


---

## D20 — The logo is a bow tie

**Date:** 2026-11-20 · **Ref:** D10, D18, D19

### Decision
- The Taful mark is a **bow tie whose knot is a diamond**: amber wings (`#f0b040`), cream knot (`#f6efdc`). The app icon is the mark on a dark petrol tile.
- It replaces the spade used in the first mockups of the visual styles (D19), and the felt disc the notification-area icon was drawn as (D18).
- The brand does not use a spade.

### Why
The author found the spade in a chip too close to the PokerStars logo. Of three proposals (bow tie, two cards showing a T, a 3×3 range grid), he picked the bow tie: "taful" means a dandy and also a gambler, and it is the dealer's tie.

### Implications
- Sources in `apps/desktop/assets/icon/`: `mark.svg` (the mark, for 32 px and up), `mark-small.svg` (a simpler cut for 16 to 24 px), `icon.svg` and `icon-small.svg` (the same two cuts on the tile) and `taful.ico` (16 to 256 px, built from the two icon files).
- `taful.exe` carries `taful.ico` as icon resource 1, which is the one GPUI gives its windows. Title bar, taskbar, Alt+Tab, Explorer, the Start menu shortcut, the uninstall entry and the installer all show it.
- The notification-area icon is the small cut with no tile, painted by `taful_app::logo`. It no longer follows the theme: amber and cream on a dark taskbar, one dark ink on a light one (read from Windows when the icon is made).
- The mark's colors are brand colors, the same in every theme and style, so they are constants in `taful_app::logo` and not theme tokens. D7's rule for the desktop UI is unchanged.
- A visual style draws the mark in its navigation in these two colors, with no chip or badge around it.


---

## D21 — Small windows, and Salão as the default style

**Date:** 2026-11-20 · **Ref:** D19

### Decision
- The window is never smaller than **940 × 600**. 940 is what half of a Full HD screen leaves a snapped window.
- Every visual style lays every screen out from that size up. In a window smaller than its mockup a style keeps its look and adapts: it drops secondary text, steps type and art down, narrows or restacks panels. From the size a style was drawn for (1600 × 900 for Salão, Cockpit and Baralho) the screens are the mockups, unchanged.
- **Salão** is the style of a new install. Classic stays on offer, last in the list.
- Whoever never picked a style gets Salão on the next start, with one exception: someone who picked a theme stays on Classic, the only style that shows it.

### Why
The styles were built at the mockups' size (D19). In a narrow window Salão's home wrapped its headline word by word, labels ran into buttons, and Cockpit scrolled sideways. The author asked for a minimum size and for the screens to hide text or shrink type below the designed size, and for Salão to be what people see first ("Classic only if someone really wants it").

### Implications
- `MIN_WINDOW` in `apps/desktop/src/main.rs` is the one place the minimum is set; Cockpit derives its narrowest layout from it.
- How each style adapts:
  - Classic: the reading column and the cards follow the pane, the table's grid makes room for the controls when they wrap, and the navigation drops its tagline and footer note in a short window.
  - Salão: a narrower sidebar and margins under 1200 px; the hero's art, type and extras step down with the column; the three numbers stack their captions; lessons go to one column; the table keeps the mockup's arrangement with a narrower side panel and a compact evaluation.
  - Cockpit: under 1280 px the home, the lessons and the table restack their panels on the whole width and scroll down (never sideways); the status bar drops the last grades. From 1280 px up the table screen keeps the mockup's panels, and two things changed for windows that are not the mockup's: in a short one the table shrinks before the screen scrolls, and the pickers wrap to a second row instead of running under the panel beside them.
  - Baralho: the whole canvas scales with the window, down to half its size (at the minimum its text is small) and never past the mockup's size: a larger window shows the same screens centered across it, under the navigation at the top, with more felt around them (amended 2026-11-21: it used to grow up to 3×, which made everything large on a big screen).
- The reviews and the hand review, drawn the same in every style, read the room they have from `Shell::room_w`, which the style's frame sets: their cards follow it, and their table keeps the explanation beside it for as long as the table stays readable, then lets it wrap below.
- `Style::default()` is Salão. A stored `ui.style` always wins, so a user who picked Classic keeps it.

---

## D22 — The app draws its own title bar

**Date:** 2026-11-20 · **Ref:** D19, D21

### Decision
- The window has no system title bar. A 32 px strip in the active style's background color takes its place, with the window's minimize, maximize/restore and close buttons on the right. The rest of the strip drags the window.
- On Windows the strip and the buttons are window control areas, so the system treats them as its own caption: double-click to maximize, Aero Snap, the snap layouts flyout on the maximize button, and resizing from the top edge of a restored window behave as before.
- The strip is the system bar's height, so every style keeps the room it had: the minimum window and a style's preferred size grow by the strip's height, and the styles lay out in the window below it.

### Why
The author asked to hide the blue Windows title bar: it clashed with the dark styles. Floating the buttons over the content instead would have covered parts of the mockups (Salão's "Editar layout" at the top right) and left only scraps of the top edge to drag the window by.

### Implications
- `apps/desktop/src/caption.rs` draws the strip; `caption::viewport(window)` is the room the styles draw in. A style asks it, not `window.viewport_size()`.
- The window title is still set: it names the window in the taskbar and Alt+Tab.
- `TAFUL_WINDOW=1600x900` sizes the room below the strip, so screenshots of a style at its mockup size are unchanged below the strip.

---

## D23 — Lessons: one catalog, several ways to browse it

**Date:** 2026-11-21 · **Ref:** D5, brief §3 (lesson track)

### Context
The lessons screen showed 40 lessons in five topic tracks chained one after another, the first of which already assumes the reader knows how a hand is played. The author asked for a path that starts from someone who never played and climbs to pro, with levels.

### Decision
- Lessons form **one catalog**. Every lesson carries a **level** and a **theme**; the screen has a **"Ver por"** switch (in the spirit of a "view by" picker, drawn for Taful) that regroups the same lessons:
  - **Jornada** (default): from zero to pro, grouped by level: Primeiros passos, Fundamentos, Iniciante, Intermediário, Avançado, Pró.
  - **Temas**: math, preflop, postflop, reading the game, mind and bankroll (the current tracks, refined).
  - Further views may be added (for example by moment of the hand, or lessons tied to the user's own leaks); the format must not assume a fixed list.
- A lesson's progress and lock state are the same in every view; the view only changes grouping and order.
- The current 40 lessons are placed in Fundamentos through Intermediário. Primeiros passos, Avançado and Pró are new content.

### Why
The author: "acho que é outra coisa que podemos deixar o usuário escolher". A beginner reads a journey by level best; someone who already plays wants to jump to a theme. One catalog with views serves both without duplicating lessons.

### Implications
- `content/skill-tree.toml` gives way to per-lesson `level` and `theme` (in the front matter or in one catalog file); content checks verify every lesson has both and that prerequisites never point to a higher level.
- Unlocking can no longer be "the next lesson in the track", since order depends on the view. How locks work is decided separately.

### As built (#85)
- One file, `content/catalog.toml`: the levels and themes in display order and every lesson with its level and theme, listed in Jornada order. Level and theme names are Fluent `level-<id>` (with a one-liner `level-<id>-about`, reused by D25) and `theme-<id>`.
- Themes got a sixth group, **O básico** (rules and table basics: hand rankings, positions, blinds and stacks), since those lessons fit none of the five and Primeiros passos will add more of them.
- The view choice is remembered (`lessons.view` setting). The next lesson is always the first open one in the Jornada, whatever the view.

---

## D24 — The user picks how lessons unlock

**Date:** 2026-11-21 · **Ref:** D23

### Decision
- How lessons unlock is a **user setting** with three modes:
  - **Prova** (default): every lesson of the user's current level is open. A short level test opens the next level, and it can be taken early to skip a level the user already knows.
  - **Guiado**: one lesson at a time; passing a lesson's quiz opens the next one in the Jornada order.
  - **Livre**: nothing is locked. The app marks the recommended next lesson and warns when a lesson's prerequisites are not done, but opens it anyway.
- The setting lives behind a **gear on the lessons screen**, which opens a small popover with the three modes, a one-line label each and an (i) with a short explanation on hover. The same setting is also in Settings.
- Changing the mode never loses progress: done lessons stay done, passed level tests stay passed.
- Lock state is computed from the mode, the lesson's level and its prerequisites, so it is the same in every "Ver por" view (D23).

### Why
The author liked the level test with a skip option but did not want to limit anyone: "o usuário consegue dizer se prefere prova, guiado, ou sugerido".

### Implications
- A level test is new content per level (a quiz drawn from that level's lessons); it is the only thing that marks a level as passed in Prova mode.
- In Livre mode the level tests still exist, as a way to see where one stands.

### As built (amended 2026-11-21, #86)
- Settings keys: `lessons.unlock_mode` (`prova` | `guiado` | `livre`), `lessons.tests_passed` (the levels whose test was passed) and `lessons.start_level` (D25). Done lessons stay in `lesson_progress`; nothing about the mode is stored there, which is why switching never loses progress.
- **Level tests** are content: `content/tests/<level>.toml` with `level`, `version`, `pass` (default 0.8), `draw` (questions per attempt) and `pool` (`<quiz id>/<question id>` from that level's lessons). Each attempt draws one question per lesson in a random order, then a second round, until `draw`. `content-check` requires a test for every level with lessons and checks the pool. A passed test is stored in settings and the attempt in `quiz_attempts` as `level-test/<level>`; tests never mark lessons done and never feed the review queue.
- **Prova:** levels up to the starting level are open, each passed test opens the next level, and a level with no lessons yet lets the next one through. A closed level names the test right before it ("Abre com a prova de Iniciante"). When every open lesson is done the lessons screen and Home suggest the test.
- **Guiado:** within a reached level only the first lesson not done is open. A level is reached when Prova would open it or when every lesson before it is done (or its test was passed).
- **Livre:** everything open; the reader lists the prerequisites not done yet.
- The next lesson is the first open one in Jornada order from the user's floor (the starting level, or a lower level they already read; past every passed test), else the first open one anywhere.
- The gear popover sits under the gear, right-aligned, closes on any click outside it, and is drawn by the shell so it floats over every style. Settings › Estudo has the same three rows.

---

## D25 — First run: the user says their level, nothing is forced

**Date:** 2026-11-21 · **Ref:** D23, D24

### Decision
- On the first run the app asks **"Qual é o seu nível no poker?"** and lists the six levels of the Jornada, each with a one-line description in plain words (for example "Nunca joguei", "Sei as regras", "Jogo por diversão", "Jogo e estudo"). A "Pular" link starts at Primeiros passos.
- The answer only sets where the Jornada starts: every level up to the chosen one is **unlocked**. Nothing is marked done or passed: lessons in skipped levels stay unread, with no score, and the user can open them any time.
- **No test is ever forced.** Level tests are an option the user can take to unlock or to see where they stand, never a gate at the start. In Prova mode (D24) the level picked on the first run is open without a test.
- The level can be changed later from the gear on the lessons screen.

### Why
The author found "Você já joga?" crude and wanted the question to ask for a level; he wants skipping to unlock, not to complete; and he finds forcing a placement test intrusive ("que fossem 3 minutos! 10, pior ainda").

### Implications
- Progress keeps two separate facts: unlocked levels and done lessons. Home counts and the level badge read only done lessons.

### As built (amended 2026-11-21, #86)
- `lessons.start_level` holds the level picked on the first run (#88 builds the picker). Until then it is derived from existing progress, so a user from before D24 keeps every lesson they had open: the highest level where a lesson is done or every prerequisite is done. The derived value is stored the first time new progress is recorded (a quiz or a level test), so later progress does not move it.

---

## D26 — Primeiros passos: short text, quiz and a guided hand

**Date:** 2026-11-21 · **Ref:** D23, D25

### Decision
- Every lesson of **Primeiros passos** has three parts, in this order:
  1. **Short text** with pictures, in plain words, a few minutes long (what a hand is, blinds, the four betting rounds, fold/check/call/bet/raise, showdown, hand rankings, chips and stacks, seats).
  2. **Quiz**, as in the current lessons.
  3. **Guided hand** on the real table: a scripted hand (fixed seats, cards and opponent actions) that pauses at each step with one sentence about what is happening, and asks the user to click the right action. A wrong click gets a short explanation and a retry, never a penalty.
- The lesson is done when the quiz is passed and the guided hand is finished.
- The guided hand is a lesson part any level can use later; Primeiros passos is the first to need it.

### Why
The author picked all three: "texto curto + quiz + mão de treino guiada na mesa". Someone who never saw a table learns the order of play by doing it, and the text and quiz give them the words first.

### Implications
- New content format: a scripted-hand file (seats, stacks, hole cards, board, opponent actions, and the text shown at each step), checked in CI like the lessons (the script must be a legal hand; every expected user action must be legal at that point).
- The table screen needs a guided mode: actions other than the expected one are explained instead of played, and the coach line sits where the evaluation panel goes.

---

## D27 — Home shows one next step

**Date:** 2026-11-21 · **Ref:** D23–D26

### Decision
- The Home's hero shows **one thing to do now**, picked in this order:
  1. Reviews that are due.
  2. The lesson in progress (or the next one in the Jornada).
  3. Practice at the table on the theme of the last lesson done.
- The other cards stay on the Home as shortcuts; only the hero changes. Nothing is mandatory and nothing is tracked as a daily plan.

### Why
The author did not understand the app's flow (lessons, drills, table). One button that answers "what do I do now?" ties lessons, reviews and the table together without turning study into a chore.

### Implications
- `taful_app` computes the next step (a small enum with its target) for every style's Home; each style draws it in its own hero.
