# Taful — Project Brief

> Input for the first `grill` session. Not a spec: it captures intent, early decisions and open questions. The spec comes out of `grill` → `to-spec`.

## 1. Concept

**Taful** (archaic Portuguese: a habitual gambler, someone who frequents gaming houses) is a personal poker coach. It teaches No-Limit Hold'em from zero to advanced through lessons, calculation training and decision simulation on a realistic poker table, and it measures progress over time.

- **User:** a single user (the author), learning from beginner level. Not a commercial product for now.
- **Primary platform:** Windows desktop. Web and Android come later, and iOS after that.
- **Out of scope:** real-money play, and any real-time assistance during online play (overlays, table reading). RTA is banned by poker sites. In-game calculations are mental techniques the user practices *inside* Taful.

## 2. Core idea: the closed loop

The product's backbone is a loop, not a set of separate features:

```
play / simulate → review → detect leak → targeted drill → spaced repetition → play again
```

Every feature feeds this loop or is fed by it. Mistakes made anywhere in the app become drill material.

## 3. Features by wave

### Wave 1 — MVP
- **Lesson track as a skill tree**, from 101 to advanced. Each lesson ends in a quiz or drill that unlocks the next one. Content is data (Markdown + TOML/JSON), not code.
- **Decision simulator** on a visual table (seats, chips, cards, pot, action buttons). Specific spots are presented and the user decides.
  - **Grading by EV loss in big blinds**, not right/wrong: `best / correct / inaccuracy / mistake / blunder`. Threshold values are TBD.
  - The explanation for each grade shows the reference strategy (action frequencies) and the reasoning behind it.
- **Timed mental math trainer:** outs, rule of 2 and 4, pot odds, MDF, alpha (required fold equity for a bluff), SPR, and equity estimates vs ranges. Every shortcut technique is shown next to the exact calculation.
- **Spaced repetition** of spots the user got wrong.
- **Drills from your own mistakes:** these reuse the same game state (positions, action history, sizing) with randomized boards.

### Wave 2
- **Play vs bots with profiles** (nit, calling station, maniac, TAG). This teaches exploitative play, not only GTO.
- **Range trainer:** paint a range on the 13×13 grid and compare it to the reference chart.
- **Board texture drills:** dry/wet, range advantage, nut advantage.
- **Hand review & replay:** import hand histories (PokerStars and GG formats) plus a quick logger for live games.
- **Optional live coach:** a toggle that nudges the user the moment they make a clear mistake in simulation.
- **Glossary tooltips** on every technical term, everywhere, including inside explanations.

### Wave 3
- **AI coach (Claude API)** explaining hands in natural language, **grounded in solver output**, never free-form opinion.
- **MTT module:** ICM, push/fold (Nash charts are computable, no licensing needed), short stacks.
- **Bankroll & sessions:** win rate, variance simulator, risk of ruin.
- **Mental game:** session journal, tilt tracker, pre-session warm-up.
- **Light gamification:** XP, streaks, per-skill rank.
- **HUD stats literacy:** what VPIP, PFR, 3-bet and similar stats say about an opponent (educational only).

## 4. Reference strategy (the "correct answer" source)

This is the main technical risk: grading needs a trustworthy reference.

- **Preflop:** static charts, either self-generated or from open sources. **Do not copy proprietary data** (GTO Wizard and similar).
- **Postflop:** the open-source `postflop-solver` by b-inary is written in Rust and powers Desktop Postflop and WASM Postflop.
  - **To verify:** license (believed to be AGPL, which is fine for personal use) and maintenance status (believed to be discontinued by the author).
- **v1 approach:** pre-solve a library of common spots (6-max, 100bb, SRP and 3-bet pots) and store the results on disk. Real-time solving comes later.

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

### Data: local-first, no backend in v1
- **SQLite** holds progress, history, the spaced-repetition queue, themes and layouts.
- Content and pre-solved spots ship with the app.
- The AI coach (wave 3) calls the Claude API directly. The key is stored in the Windows Credential Manager.
- **Sync-ready schema from day one:** ULID/UUID ids, `updated_at` on every row, soft deletes.
- **A backend is only needed for cross-device sync.** When that time comes, add a thin sync API. The hosting choice is TBD.

## 6. Look & feel

- **Mandatory poker-client look:** an oval table, felt, seats with avatars and stacks, chips, a dealer button, an animated pot and cards. It should be inspired by mainstream clients but not imitate any of them.
- **Default theme:** deep petrol-teal or slate-blue felt, amber/gold chips and accents, cream typography. **No red anywhere.** The 4-color deck uses gold or burnt orange for hearts and diamonds.
- **Everything is themeable through design tokens.** A theme is just a token file, and an in-app theme editor builds on that.
- **Customizable layout:**
  - Panel, HUD and action-button positions are stored as JSON.
  - An **edit mode with drag-and-drop and grid snapping** is preferred. Zed itself drags panes, so GPUI supports it.
  - Layout presets can be saved, loaded and reset.
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

## 8. Open questions (agenda for `grill`)

1. **Initial format:** cash 6-max 100bb only? When do MTT, other stack depths and PLO come in?
2. **Spots for v1:** exactly which ones to pre-solve, and how many?
3. **EV-loss thresholds** for each grade (in bb, and possibly relative to pot size).
4. **Simulator depth:** single decision points, or full hands street by street?
5. **Lesson content:** who writes it (AI-drafted then reviewed?), what format, and how many lessons for v1?
6. **Spaced-repetition algorithm:** SM-2 or FSRS?
7. **Layout customization in v1:** full drag-and-drop editor, or presets plus a theme editor first?
8. **Bots in v1 or wave 2?** And how are bot profiles modeled (parametric deviations from the reference strategy)?
9. **Sync:** when is it needed, and which backend/host?
10. **App name:** confirm that "Taful" is free (domain, Play Store, trademark registry).

## 9. Early spikes (before implementation)

1. **GPUI table render:** draw a table with seats, chips and cards, plus a drag-and-drop panel, on Windows.
2. **Solver:** solve one flop spot with `postflop-solver`, then measure time and memory and serialize the result.
3. **Hand evaluator + equity:** pick a crate or write our own, then benchmark it.

If any spike fails, revisit the stack before writing the spec.
