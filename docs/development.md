# Development

## Layout

See [spec §4](spec.md#4-workspace). In short:

| Path | What |
|---|---|
| `crates/taful-core` | Domain model. No IO, no UI, builds for `wasm32-unknown-unknown`. |
| `crates/taful-solver` | Library format, river solver, best response. WASM-safe, no AGPL. |
| `crates/taful-content` | Lesson/quiz loader and content checks. |
| `crates/taful-store` | SQLite schema, migrations, backups. |
| `crates/taful-app` | Application services, i18n (Fluent), theme tokens. The only crate the UI uses. |
| `apps/desktop` | GPUI app, binary `taful`. Windows only for now. |
| `tools/arch-check` | Dependency-direction and license checks over `cargo metadata`. |
| `tools/content-check` | Runs the content checks over `content/`. |
| `tools/library-build` | Offline library build: preflop solver (M3), postflop batch (M4). The only crate allowed to link `postflop-solver` (pinned git rev, no default features). |
| `tools/profile-stats` | Offline: HUD stats of the bot profiles over the library and auto-tuning of `content/bots/*.toml` toward `targets.toml` (#31). Core + solver only. |
| `library/` | Built library files in the static layout of spec §7.6 (`v1/<config key>/…`). In git: the preflop charts (embedded by `taful-app`) and `flops.txt`. The postflop files are too big for git and live in the app's library folder. |
| `content/` | Bot profiles (`bots/*.toml`), themes, `skill-tree.toml`, lessons (`lessons/<locale>/*.md`), quizzes (`quizzes/*.toml`) and their render snapshots (`.snapshots/`). Embedded into `taful-app` by its `build.rs`. |
| `config/` | Tunable numbers loaded at runtime: `grading.toml` (D3 thresholds), `calc.toml` (trainer tolerances and target times). Tests pin them to the code defaults. |
| `i18n/<locale>/*.ftl` | Fluent strings. en-US is the source, pt-BR ships. |

## Commands

```sh
cargo fmt --all
cargo run -p taful-arch-check                        # architecture rules
cargo clippy --workspace --exclude taful-desktop --all-targets -- -D warnings
cargo test --workspace --exclude taful-desktop       # UI-free crates and tools
cargo test --release -p taful-core -- --include-ignored  # + exhaustive evaluator/equity tests
cargo run -p taful-content-check -- content
cargo run -p taful-content-check -- --bless content   # after an intended change to a rendered value

# Windows only (gpui 0.2.2 does not build on Linux/macOS, see docs/spikes/9.1-gpui-table.md):
cargo test -p taful-desktop                          # color-literal lint, Fluent ids
cargo run --release -p taful-desktop                 # TAFUL_LOCALE=en-US for English
```

CI (`.github/workflows/ci.yml`) runs the Linux block on `ubuntu-latest` (plus a `wasm32` build of `taful-core` and `taful-solver`) and the Windows block on `windows-latest`, which also launches the app, builds the installer, runs it through install/upgrade/uninstall, and uploads `taful.exe`, the installer and a screenshot as artifacts.

## Windows installer (D14)

CI builds `taful-setup-<version>.exe` with [Inno Setup 6](https://jrsoftware.org/isinfo.php) from `installer/taful.iss` and uploads it as the `taful-setup-<version>` artifact of the Windows job (every main build and every PR that is not docs-only). The version is the workspace version in `Cargo.toml` plus a pre-release suffix (see [Releases and auto-update](#releases-and-auto-update-d18)); the app shows it in the nav footer.

- **Unsigned.** The postflop library is not bundled: pick its folder in Settings.
- **Per user**, no admin prompt: the app goes to `%LOCALAPPDATA%\Programs\Taful`, the shortcut to your Start menu. Per user is the simpler choice because the data folder is per user anyway (`%APPDATA%\Taful`), and it needs no elevation for installs or upgrades.
- **Data is never touched.** Install, upgrade and uninstall only write `%LOCALAPPDATA%\Programs\Taful` and the shortcut; `%APPDATA%\Taful` (database, backups, default library folder) stays as it is. `installer/check.ps1` checks this in CI: it installs, installs again over it, uninstalls, and compares a hash of every file in the data folder after each step.

**Installing:**

1. Download `taful-setup-<version>.exe` from the newest [release](https://github.com/fernandolisboa/taful/releases) (or a CI run's *Artifacts*).
2. Run it. Windows SmartScreen says "Windows protected your PC" because the file is unsigned: click **More info**, then **Run anyway**.
3. Launch Taful from the Start menu.

**Upgrading:** the app updates itself (below). By hand: run the newer setup over the old one (Taful closes if it is open). **Uninstalling:** Settings → Apps → Installed apps → Taful → Uninstall. To remove your data too, delete `%APPDATA%\Taful` by hand.

Build it locally (Windows): `cargo build --release -p taful-desktop`, then `ISCC /DAppVersion=0.1.0 installer\taful.iss`; the setup lands in `target\installer\`.

## App icon (D20)

The mark and the app icon are drawn as SVG in `apps/desktop/assets/icon/`. `taful.ico` is rendered from `icon-small.svg` (16, 20, 24 and 32 px) and `icon.svg` (40, 48, 64, 128 and 256 px); render it again whenever one of them changes. Entries below 256 px are 32-bit bitmaps and the 256 px one is a PNG, the layout every Windows tool reads.

`apps/desktop/build.rs` compiles `apps/desktop/taful.rc` into `taful.exe` (Windows builds only; it needs `rc.exe` from the Windows SDK, which the MSVC toolchain already requires). The installer takes the same file as its own icon. The notification-area icon is not a file: `taful_app::logo` paints the small cut from a copy of the outline in `mark-small.svg`, and a test fails when the two differ.

## Releases and auto-update (D18)

The `release` job of CI publishes GitHub Releases the app updates from:

| Trigger | Version | Release |
|---|---|---|
| push to `main` (CI green) | `<workspace>-<run>.alpha` | pre-release; only the 10 newest alphas are kept |
| *Run workflow* on `main`, `release: beta` | `<workspace>-<run>.beta` | pre-release |
| *Run workflow* on `main`, `release: stable` | `<workspace>` | latest release; bump the workspace version afterwards |
| pull request | `<workspace>-0.pr<n>.<run>` | none (artifact only) |

Each release has the installer and `<installer>.sha256`. The app (Settings › Updates) checks at startup and every 6 hours, downloads to `%LOCALAPPDATA%\Taful\updates`, verifies the hash, and runs the installer with `/WAITPID=<pid>` (the installer waits for the app to exit) and `/RELAUNCH=1` for *Restart and update*. The tray icon's menu has *Check for updates*.

Dev overrides: `TAFUL_VERSION` (build time) sets the app's version; `TAFUL_UPDATE_URL` points the check at another release list (e.g. a local `python -m http.server` serving a JSON file shaped like the GitHub API); `TAFUL_SETTINGS_TAB=updates` opens that settings tab.

## Rules enforced by tests

- **Dependency direction** (`tools/arch-check`): `core ← solver ← app ← desktop`, `core ← content ← app`, `store ← app`. A new workspace crate must be added to `RULES` in `tools/arch-check/src/main.rs`.
- **No copyleft in shipped crates** (D11): no shipped crate may reach `postflop-solver` or a GPL/AGPL-only package.
- **No color literals** in `apps/desktop/src` outside `theme.rs`; use a token from `content/themes/*.json`.
- **Theme rules** (every file in `content/themes/`): `Theme::warnings` must be empty: WCAG AA contrast for text and grade tokens over their backgrounds (4.5:1, or 3:1 for large card ranks), no red token, and grade colors at least 60 apart in RGB. A widget that draws text on a new background adds the pair to `Theme::text_pairs`. User themes from the in-app editor run the same checks live as warnings only (D7 amendment) and are stored in the `themes` table. The editor's color helpers live in `apps/desktop/src/theme.rs` (`token_color`, `hsl_of`) because the color-literal lint flags HSL/RGBA calls anywhere else.
- **i18n**: every `.ftl` parses, en-US and pt-BR have the same keys, every file is embedded, and every Fluent id the desktop UI names exists.

## taful-core notes

- **Card and combo encoding** (`cards.rs`): `Card(rank * 4 + suit)`, suits `c d h s`; combo index is the triangular index of the two card ids. Shared with the library format, so changing it means a library `format_version` bump.
- **WASM:** `rs_poker` pulls `rand 0.10` → `getrandom 0.4`, which refuses to build for `wasm32-unknown-unknown` without a backend. `taful-core` enables getrandom's `wasm_js` feature for that target only. Nothing in Taful asks for OS entropy; every random draw is a seeded `ChaCha8Rng`.
- **`GameConfig::key()`** is pinned by a test. If it changes, every library key changes: bump `MODEL_VERSION` and the library format together.
- **Engine** (`engine.rs`): the driver deals the board (`deal_next` from the seeded deck, or `deal_board` for library flops, D13). Rules decisions not spelled out in the spec are listed in the module docs (short blinds, big-blind ante as dead money, cumulative short all-ins reopening).

## Preflop charts (M3)

The app embeds `library/v1/<config key>/preflop.tafl`. To rebuild it after changing the manifest or the solver:

```sh
cargo run --release -p taful-library-build -- preflop tools/library-build/manifests/preflop-6max-100bb.toml library
cargo run --release -p taful-library-build -- summary library/v1/*/preflop.tafl 3   # sanity check
```

About 5 minutes on 4 vCPU (the 169×169 equity table is cached in `library/cache/`, which is git-ignored). A test fails if the committed chart was built from a different manifest. Model and format details: spec §6 and §7.5.

The same run writes one best-response chart per bot profile listed in the manifest's `[profiles]` (`preflop-<id>.tafl`, spec §9). Edit a profile in `content/bots/` → rerun the command above: the reference chart comes out byte-identical (profiles are not in its hash) and only the profile charts change; a test fails if a profile changed without a rebuild. A new profile also needs its `bot-profile-<id>` and `bot-profile-<id>-about` messages and an entry in `BUILTIN_PROFILE_CHARTS` (`crates/taful-app/src/preflop.rs`).

It also writes `preflop-full.tafl` (the chart without pruning) and `preflop-equity.bin` (the equity table the tree was built on), which the app embeds to compute mixed tables' best responses on demand (#29, spec §9). Commit them with the chart. `cargo run --release -p taful-library-build -- preflop-mixed-bench tools/library-build/manifests/preflop-6max-100bb.toml <scratch dir>` re-solves the manifest and times those best responses.

The desktop table screen opens directly with `TAFUL_SCREEN=table` (any `nav-<id>` works). `TAFUL_THEME=<id>` and `TAFUL_LAYOUT=<preset>` override the saved theme and layout (screenshots).

## Postflop library (M4)

Built on the author's PC (spec §7.3, Q4), never in CI. Files land in `<out>/v1/<config key>/<spot>/<flop>.tafl` plus `<out>/manifest.json` (size + BLAKE3 per file). The app reads the folder in `TAFUL_LIBRARY_DIR`, else `<data dir>/library`.

```sh
# Memory estimate of every pending solve (cheap, no solving):
cargo run --release -p taful-library-build -- postflop tools/library-build/manifests/postflop-6max-100bb.toml <out> --estimate
# Daytime, light: 3-bet pots only, 4 threads, nothing above 6 GB, at most 10 solves:
cargo run --release -p taful-library-build -- postflop tools/library-build/manifests/postflop-6max-100bb.toml <out> \
  --only btn-sb-3bp,co-btn-3bp --threads 4 --max-mem-gb 6 --limit 10
# Night: everything (one solve at a time, all threads):
cargo run --release -p taful-library-build -- postflop tools/library-build/manifests/postflop-6max-100bb.toml <out>
```

- **Resumable:** finished files are skipped; a file appears only when complete. Stopping the process loses at most the current solve.
- **Pause:** create `<out>/PAUSE` and the run waits before its next solve; delete it to continue.
- **Priority:** the run lowers itself to below-normal priority (PowerShell on Windows, `renice` elsewhere).
- **Log:** one JSON line per solve in `<out>/build-log.jsonl` (iterations, exploitability, time, estimated memory, nodes, file size).
- **Flop order:** `library/flops.txt` (all 1,755 canonical flops; the first 49 are the stratified first slice). Regenerate with `taful-library-build flops library/flops.txt` only if no solved flop moves; a test pins it.
- **River solver validation** (`tools/library-build/src/postflop/validate.rs`): Taful's river solver against postflop-solver on fixed spots. Fast, so it runs with the normal tests.
- **App test fixture:** `crates/taful-app/tests/fixtures/postflop` is a tiny library (coarse tree, few iterations) from `manifests/fixture.toml`; rebuild with the command at the top of that manifest.
- **Bot profiles (#27):** a `[profiles]` table (`dir` relative to the manifest, `ids`) makes every solve also store each profile's flop/turn strategy and hero's best-response EVs (format v3, spec §7.5, §9). Profiles are part of the postflop hash, so editing one means rebuilding. The shipped manifest lists the four profiles with `solve.profile_seed` on (#31, D15), so its hash differs from the reference-only 245: build into a **fresh output folder**, because finished files are skipped by existence and old reference-only files would otherwise stay. Seeded solves take ~7.5 min each against ~4 without, and dry SRP flops peak at ~26 GB (night only). Files grow about 5× with the four profiles. `crates/taful-app/tests/fixtures/postflop-profiles` is the fixture with profile data, from `manifests/fixture-profiles.toml`.

## Bot profile calibration (#31)

`tools/profile-stats` measures what a HUD would show for each bot and tunes the profile knobs toward `tools/profile-stats/targets.toml` (usual HUD reads per player type, 6-max 100bb), so nobody has to calibrate by playing.

```sh
cargo run --release -p taful-profile-stats -- preflop                     # VPIP, PFR, 3-bet, fold to 3-bet, open %
cargo run --release -p taful-profile-stats -- postflop --library <out>    # flop c-bet, fold to c-bet, aggression, per spot
cargo run --release -p taful-profile-stats -- check --library <out>       # recomputed vs stored profile strategies
cargo run --release -p taful-profile-stats -- tune --library <out>        # search; add --write to rewrite content/bots
```

- **Preflop** is exact: the bot sits in each seat at a table of five reference players and every line of the published `preflop.tafl` (the chart the app's bots play) is walked once (card removal between players ignored). Takes well under a second.
- **Flop** reads only the flop of each library file and recomputes the bot's strategy from the stored reference strategy and EVs, so knobs can be tried without re-solving. Its flop range is its own preflop range (D15). Files weigh by orbit size, spots by how often the bot plays the spot's line from that seat. The reference stores no rows for the seeded combos a loose bot adds (D15), so those combos play the strategy the build stored for that profile (`fixed%`: they don't follow a knob change) or, without one, are left out (`nodata%`). After rebuilding with new knobs, `postflop --stored` measures exactly what the build stored.
- **Tuning** is coordinate-wise bisection: `threebet_mult` → 3-bet, `vpip_mult` → VPIP, `pfr_mult` → PFR, then `fold_to_cbet` → fold to c-bet and `aggression` → flop aggression factor. Fold to 3-bet and flop c-bet follow from those and are only reported. `fold_to_raise`, `bluff_value_shift` and sizing have no HUD target and stay as they are.
- Writing new knobs changes both builds: rebuild the preflop best-response charts (`taful-library-build preflop`, a test checks `preflop-<id>.tafl` against the profiles) and the postflop library into a fresh folder.

## Full hands, reviews and leaks (M5)

- `taful-app::hand` (full-hand driver), `walk` (library tree + in-app river), `template` (spot templates, leak buckets), `training` (`Trainers`, drills, storage of hands/decisions, leaks, hand review). The desktop table screen emits `TableEvent`s and the shell stores them.
- Schema v2 (`0002_hand_review.sql`): `hands.replay` (JSON), `hands.hero_net`, and the decision context columns used by the review queue and leak buckets.
- Debug builds solve rivers coarsely (60 iterations) so tests stay fast; use a release build to judge river strategies.
- `TAFUL_TABLE_SEED` fixes the table screen's first seed (screenshots); `TAFUL_TABLE_KIND=<spot kind id>` and `TAFUL_TABLE_OPPONENT=<profile id>` preselect spot mode's kind and opponent.

## Data and content (M2)

- **Data dir:** `TAFUL_DATA_DIR`, else `%APPDATA%\Taful`, else `$XDG_DATA_HOME/taful`, else `~/.local/share/taful`. One `taful.sqlite` plus `backups/` (the folder can be changed in settings).
- **Schema changes:** add `crates/taful-store/src/migrations/NNNN_*.sql` and list it in `schema::MIGRATIONS`. Never edit a shipped migration. `schema_rules` checks the D9 columns and `migrates_from_every_past_version` runs every prefix.
- **A new lesson:** write it in both locales with the same front matter, add it to `content/skill-tree.toml`, give it a quiz (or a drill unlock once spot templates exist), add any glossary terms to both `glossary.ftl`, then run `content-check --bless` and review the snapshot diff.
- **A new theme:** add `content/themes/<id>.json` (schema 2, every token), list it in `taful_app::theme::BUILTIN_THEMES`, and add `theme-<id>` to both `settings.ftl`. Tests check AA contrast for every text pair (including the grade colors), no red, and distinct grade colors. A new token bumps `theme::SCHEMA_VERSION` and adds a step to `Theme::from_json` that fills it for older files.
- **A new layout preset:** add `content/layouts/<id>.json` (48 × 27 grid, all four panels), list it in `taful_app::layout::BUILTIN_LAYOUTS`, and add `layout-<id>` to both `settings.ftl`. A new panel id goes in `layout::PanelId` and gets a place in every preset; older saved layouts pick it up from the default preset.
- **Slow work in the UI:** anything that can solve a river or read a library file runs on `cx.background_executor()`; see `TableScreen::run` and `Shell::review_step`. A type that crosses that boundary is added to the `thread_safety` test in `taful-app`.
- **A new placeholder function:** add it to `FUNCTIONS` and `eval` in `crates/taful-content/src/expr.rs`; it must be a pure call into `taful-core`.

## Running the desktop on Linux (unsupported, for screenshots)

gpui 0.2.2 fails on Linux because `xattr` 0.2.3 uses `ENOATTR`. A local copy of the crate with `ENOATTR` → `ENODATA` in `src/sys/mod.rs`, wired in through `--config 'patch.crates-io.xattr.path="…"'`, builds it. Never commit that patch or the `Cargo.lock` it produces. Running under Xvfb needs `libxkbcommon-x11-dev`, `mesa-vulkan-drivers` and `libvulkan1`, and the first frame needs a mouse move (`xdotool mousemove`).

## Visual styles (D19)

The desktop app draws itself through the active visual style (`ui.style`): Salão (the default, D21), Cockpit, Baralho or Classic.

- One module per style: `apps/desktop/src/<id>/mod.rs`. `classic` is the reference set of hooks, listed at the top of `apps/desktop/src/style.rs`. Another style starts as `pub use crate::classic::*;` and replaces a hook by defining an item with the same name. Shared code calls `by_style!(ui.style, hook(args))` and never names a style.
- Hooks: constants (`READY`, `ASSETS`, `FONTS`, `FONT_FAMILY`, `WINDOW`, `FELT_SCALE`, `FELT_WIDTH`), state (`ShellState`, `TableState`), views (`frame`, `key`, `home`, `lessons`, `table`, `story`), the felt (`felt_table`, `felt_card`, `explanation`, `felt_panel`) and the widgets the top of `widgets.rs` dispatches (`button`, `chip`, `card`, `title`, `muted`, `answer_box`, `progress`, `eyebrow`, `segmented`, `segment`, `empty_state`, `subhead`, `toggle`). The lesson text renderer (`document`) is shared by every style.
- Colors: `content/styles/<id>.json` holds `{"theme": <theme tokens>, "extra": {"name": "#rrggbb[aa]"}}`. In that style `ui.theme` is the file's theme and `ui.extra.get("name")` gives an extra color. `theme::alpha(color, a)` makes a translucent variant. No color literal outside `src/theme.rs`, as before. CI builds the desktop app in release mode, where a misspelled extra name draws nothing instead of panicking: give the style a test that checks every name it uses with `ui.extra.contains(name)`.
- Files: fonts and art live in `apps/desktop/assets/<id>/` and are listed in the style's `ASSETS` (served to `img()` and `svg()`, paths start with `<id>/`) and `FONTS` (added to the text system at startup).
- Strings only one style shows go in `i18n/<locale>/<id>.ftl`.
- `READY = true` offers the style in Settings › Appearance.
- `TAFUL_STYLE=<id>` overrides the saved style and `TAFUL_WINDOW=1600x900` sets the window size (screenshots at a mockup's size).

Writing a style's module, mind how the hooks resolve:

- An item defined in the style's `mod.rs` replaces Classic's hook of the same name. A hook kept in a submodule is re-exported by name (`pub use self::views::home;`): a second glob (`pub use self::views::*;`) is ambiguous with Classic's.
- Do not import the shared dispatchers by their bare names at the root of the style's module (`use crate::widgets::{button, card};`, `use crate::felt::card;`): the import hides the hook of the same name. Call them by path (`crate::widgets::button(..)`) or import them inside a submodule.
- A hook must not call the shared function of the same name: `crate::widgets::card` dispatches back to the hook and never returns. Build on Classic's with `crate::classic::card`.
- `key` runs before the answer box and the theme editor: a style that binds plain characters checks `shell.screen` first.
- `Shell::content()` and `Shell::header()` are Classic's content pane and title (the reading column assumes Classic's `NAV_WIDTH`). A style with another navigation draws its own in `frame` from `Shell::screen_body()`.
- Element ids and extra color names use underscores: a `lower-case-with-dashes` string literal must be a Fluent id (lint).
- A style's `table` arranges the screen itself and takes its pieces from `TableScreen`: `question_parts`/`hand_parts` (the felt, the action buttons and the side panel, drawn through the felt hooks), `mode_segments`, `kind_chips`, `postflop_chips` and `opponent_chips` (the controls, as widgets of the style), `story_columns` (the hand's story as data, for the `story` hook) and `repeat_spot` (the answered spot again; the second answer is graded and not stored). `render_classic` is the grid of panels: the layout edit mode and layouts other than the built-in default need it.
- `FELT_SCALE` sizes a felt that is not in a layout cell (reviews, hand review): `felt::natural_scale(ui)`.

### Small windows (D21)

The window is at least `MIN_WINDOW` (940 × 600, `main.rs`), and every style has to hold every screen from there up. At the size a style was drawn for, nothing may move: compare screenshots with `main` before and after a change.

- GPUI has no scale transform and wraps text only in a box with a definite width. A style adapts by computing sizes from the window (`window.viewport_size()`), not by letting flex shrink things: give wrapped text a width, give a row that may not fit `flex_wrap()` or a second arrangement, and `truncate()` what may be cut.
- A style's frame sets `Shell::room_w` (the width the screen's body gets) before it asks for the body. Shared screens size their cards with it (`Shell::card_width()`), and a felt that stands alone is drawn with `felt::room_scale(ui, room_w)`, which needs the style's `FELT_WIDTH` (the table's width at scale 1; zero for a style that fits its table to the window itself, like Baralho).
- A block whose height depends on wrapping (the table's controls) measures itself with `screens::table::measure_height` and is laid out from that on the next frame.
- Breakpoints are constants next to the layout they switch, named for what changes (`STATS_STACK_W`, `TWO_COLUMNS_W`, `WIDE_PANE`), and compare against the room the screen has, not the window.
- Check a change at 940 × 600, 1100 × 700, 1280 × 720 and 1600 × 900 (`TAFUL_WINDOW` and `TAFUL_SCREEN`, above).

### Cockpit

- `apps/desktop/src/cockpit/`: `frame.rs` (icon rail, status bar), `home.rs` (dashboard), `lessons.rs`, and the table in four files: `table.rs` (pickers, action bar, keys, the scene the panels draw from), `felt.rs` (the stage, at the design's coordinates and scaled as a whole), `line.rs` (hand line) and `eval.rs` (evaluation and range). `kit.rs` holds the shared pieces and `pal.rs` every color.
- Its numbers come from two `taful-app` services: `dashboard` (home: decisions by day, leaks, preflop deviation, review forecast) and `insight` (table: hero's range at a decision, the reference mix per hand class, equity against the last aggressor's range, pot odds). `insight.rs` documents each definition.
- The table shows nothing of the strategy before the answer: the range matrix is only in or out of range, and the frequencies, EVs and equity appear once the decision is graded.
- After an answer the felt shows hero's action played; `‹` on the hand line steps back to the table the decision was made on, then through the replay.
- Keys on the table: `F` fold, `C` check or call, `R` the bet or raise (`1`, `2`, `3`… when there are several sizes), `A` all-in, `Space` next deal or skip the replay, `Backspace` replay again.
- The bundled fonts have no suit symbols: draw a suit with `kit::suit_icon`.
- The panels are fixed. "Editar layout" is shown as designed and does nothing in Cockpit yet (the layout editor arranges Classic's panels).
- Under a 1280 px window (`frame::WIDE_PANE`) the screens restack: the home goes to two numbers a row and full-width panels, the lessons wrap their columns, and the table stacks the table, the hand line and the evaluation and scrolls down. The pickers above the table wrap, and the panels start under whatever height they take.
- The status bar has no button to replay a screen's tour (the mockup has none): tours are reset in Settings.
- The color lint matches substrings: `anchored()` reads as `red()`, so `table.rs` imports it under another name.

## Adding a string

1. Add the key to `i18n/en-US/<file>.ftl` and `i18n/pt-BR/<file>.ftl`.
2. A new `.ftl` file also goes in `taful_app::i18n::SOURCES`.
3. Use it from the UI with `ui.i18n.tr("key")` or `tr_args`.
