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
| `library/` | Built library files in the static layout of spec §7.6 (`v1/<config key>/…`). In git: the preflop chart (embedded by `taful-app`) and `flops.txt`. The postflop files are too big for git and live in the app's library folder. |
| `content/` | Themes, `skill-tree.toml`, lessons (`lessons/<locale>/*.md`), quizzes (`quizzes/*.toml`) and their render snapshots (`.snapshots/`). Embedded into `taful-app` by its `build.rs`. |
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

CI builds `taful-setup-<version>.exe` with [Inno Setup 6](https://jrsoftware.org/isinfo.php) from `installer/taful.iss` and uploads it as the `taful-setup-<version>` artifact of the Windows job (every main build and every PR that is not docs-only). The version is the workspace version in `Cargo.toml`.

- **Unsigned, no auto-update.** The postflop library is not bundled: pick its folder in Settings.
- **Per user**, no admin prompt: the app goes to `%LOCALAPPDATA%\Programs\Taful`, the shortcut to your Start menu. Per user is the simpler choice because the data folder is per user anyway (`%APPDATA%\Taful`), and it needs no elevation for installs or upgrades.
- **Data is never touched.** Install, upgrade and uninstall only write `%LOCALAPPDATA%\Programs\Taful` and the shortcut; `%APPDATA%\Taful` (database, backups, default library folder) stays as it is. `installer/check.ps1` checks this in CI: it installs, installs again over it, uninstalls, and compares a hash of every file in the data folder after each step.

**Installing:**

1. Open the latest green CI run on `main` → *Artifacts* → download `taful-setup-<version>` and unzip it.
2. Run `taful-setup-<version>.exe`. Windows SmartScreen says "Windows protected your PC" because the file is unsigned: click **More info**, then **Run anyway**.
3. Launch Taful from the Start menu.

**Upgrading:** run the newer setup over the old one (Taful closes if it is open). **Uninstalling:** Settings → Apps → Installed apps → Taful → Uninstall. To remove your data too, delete `%APPDATA%\Taful` by hand.

Build it locally (Windows): `cargo build --release -p taful-desktop`, then `ISCC /DAppVersion=0.0.1 installer\taful.iss`; the setup lands in `target\installer\`.

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

## Full hands, reviews and leaks (M5)

- `taful-app::hand` (full-hand driver), `walk` (library tree + in-app river), `template` (spot templates, leak buckets), `training` (`Trainers`, drills, storage of hands/decisions, leaks, hand review). The desktop table screen emits `TableEvent`s and the shell stores them.
- Schema v2 (`0002_hand_review.sql`): `hands.replay` (JSON), `hands.hero_net`, and the decision context columns used by the review queue and leak buckets.
- Debug builds solve rivers coarsely (60 iterations) so tests stay fast; use a release build to judge river strategies.
- `TAFUL_TABLE_SEED` fixes the table screen's first seed (screenshots).

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

## Adding a string

1. Add the key to `i18n/en-US/<file>.ftl` and `i18n/pt-BR/<file>.ftl`.
2. A new `.ftl` file also goes in `taful_app::i18n::SOURCES`.
3. Use it from the UI with `ui.i18n.tr("key")` or `tr_args`.
