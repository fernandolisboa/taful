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
| `tools/library-build` | Offline library build (M3/M4). The only crate allowed to link `postflop-solver`. |
| `content/` | Themes now; lessons, quizzes, layouts later. |
| `i18n/<locale>/*.ftl` | Fluent strings. en-US is the source, pt-BR ships. |

## Commands

```sh
cargo fmt --all
cargo run -p taful-arch-check                        # architecture rules
cargo clippy --workspace --exclude taful-desktop --all-targets -- -D warnings
cargo test --workspace --exclude taful-desktop       # UI-free crates and tools
cargo run -p taful-content-check -- content

# Windows only (gpui 0.2.2 does not build on Linux/macOS, see docs/spikes/9.1-gpui-table.md):
cargo test -p taful-desktop                          # color-literal lint, Fluent ids
cargo run --release -p taful-desktop                 # TAFUL_LOCALE=en-US for English
```

CI (`.github/workflows/ci.yml`) runs the Linux block on `ubuntu-latest` (plus a `wasm32` build of `taful-core` and `taful-solver`) and the Windows block on `windows-latest`, which also launches the app and uploads `taful.exe` and a screenshot as artifacts.

## Rules enforced by tests

- **Dependency direction** (`tools/arch-check`): `core ← solver ← app ← desktop`, `core ← content ← app`, `store ← app`. A new workspace crate must be added to `RULES` in `tools/arch-check/src/main.rs`.
- **No copyleft in shipped crates** (D11): no shipped crate may reach `postflop-solver` or a GPL/AGPL-only package.
- **No color literals** in `apps/desktop/src` outside `theme.rs`; use a token from `content/themes/*.json`.
- **Theme rules** (every file in `content/themes/`): WCAG AA contrast for text tokens over their backgrounds (4.5:1, or 3:1 for large card ranks), and no red token. A widget that draws text on a new background adds the pair to `Theme::text_pairs`.
- **i18n**: every `.ftl` parses, en-US and pt-BR have the same keys, every file is embedded, and every Fluent id the desktop UI names exists.

## Adding a string

1. Add the key to `i18n/en-US/<file>.ftl` and `i18n/pt-BR/<file>.ftl`.
2. A new `.ftl` file also goes in `taful_app::i18n::SOURCES`.
3. Use it from the UI with `ui.i18n.tr("key")` or `tr_args`.
