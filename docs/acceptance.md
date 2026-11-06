# First-slice acceptance (spec §1.1)

> **Status:** checked 2026-11-06 at the end of M6. Each item lists where it is built, how it is tested, and what is still open. "Windows" means the CI build and smoke run on `windows-latest`; the final hands-on pass is on the author's PC.

| # | Criterion | State |
|---|---|---|
| 1 | ~40-lesson skill tree, each lesson unlocks the next through a quiz or drill | **Done** |
| 2 | Every mental-math exercise type, timed, exact calculation next to the shortcut | **Done** |
| 3 | Preflop drills for every 6-max node; postflop drills for the 5 D2 configurations; graded by EV loss | **Done in code; postflop data pending** |
| 4 | Full hands vs the reference bot, every hero decision graded, hand review afterwards | **Done in code; postflop data pending** |
| 5 | Mistakes, failed quizzes and slow calc answers in one FSRS queue | **Done** |
| 6 | Layout presets, themes, automatic rotating backups | **Done** |

## 1. Skill tree

- 40 lessons in five tracks (`content/skill-tree.toml`): foundations 4, poker math 10, preflop 9, postflop 12, game plan 5. Both locales, same front matter and placeholders.
- Every lesson unlocks with a quiz (`content/quizzes/`); passing marks it done and unlocks the lessons whose prerequisites are all done (M2).
- Tests: `content-check` (placeholders, snapshots, locale parity, tree order and cycles, unlocks, glossary) runs in CI; `taful-app` lesson tests cover unlocking.

## 2. Mental math

- The calc trainer covers all seven `ExerciseKind`s (outs, rule of 2 and 4, pot odds, MDF, alpha, SPR, equity vs range) with a live timer, target times from `config/calc.toml`, and the exact calculation next to the shortcut (M2).

## 3. Spot drills

- Preflop: every spot kind of the chart (RFI, vs open, squeeze, vs 3-bet, vs 4-bet, vs all-in, blind vs blind); `available_kinds() == SpotKind::ALL` is pinned by a test (M3).
- Postflop: the five configurations of D2 (BTN vs BB, CO vs BB, SB vs BB single-raised; BTN vs SB and CO vs BTN 3-bet) on the solved flops, flop and turn decisions, river solved in the app (M4, M5).
- **Open:** the first-slice library (245 solves) is being built on the author's PC: 182 / 245 done as of 2026-11-06 morning, about 63 solves (≈ 4 h at night) left. The app reads whatever library folder is installed (settings → strategy library). This is tracked by the M4 thread, not by M6.

## 4. Full hands

- Preflop by the chart, flop and turn on the library, river solved in the app, every hero decision graded; the hand is stored with a replay and opened in the hand review (M5).
- M6: every hand step now runs off the UI thread (see spec §15.2, performance).
- **Open:** same library dependency as item 3 for hands that reach the flop.

## 5. One review queue

- Spot templates (graded decisions below Correct), quiz questions answered wrong, and calc answers rated Hard or worse create or update items in one FSRS queue; the review session mixes the kinds (M2, M5). Spot templates are dealt in the background since M6.

## 6. Layouts, themes, backups

- Layouts: three presets (`default`, `panel-left`, `wide-panel`) on the 48 × 27 grid; save a copy, delete a copy, back to default (M6, spec §15.3).
- Themes: `default`, `midnight`, `daylight`, each checked for AA contrast and no red, with grade colors (M6, spec §15.4).
- Backups: every 24 h of use and on exit, 7 daily + 4 weekly kept, export and import from settings (M2).

## What is left before calling the slice done

1. Finish the postflop library build (M4 thread) and install it in `%APPDATA%\Taful\library` (or pick the folder in settings).
2. A hands-on pass on Windows: one lesson + quiz, a calc round, preflop and postflop spots, a full hand to the river, the hand review, a review session, and switching every theme and layout.
