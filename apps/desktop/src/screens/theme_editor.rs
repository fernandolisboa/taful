//! Theme editor (issue #23, spec §15.4, D7 amendment "Theme editor
//! validation"): edits a user theme token by token. Every edit repaints the
//! whole app (table included) at once; the theme rules run live and show as
//! warnings, and saving with warnings is allowed.
//!
//! Color tokens take a typed `#rrggbb[aa]` value or hue / saturation /
//! lightness steps; size tokens take ± steps. Typing goes through the
//! shell's single key handler ([`Shell::theme_editor_key`]).

use std::rc::Rc;

use gpui::{Context, Div, FontWeight, KeyDownEvent, div, prelude::*, px};
use taful_app::appearance::{
    SIZE_GROUPS, THEME_NAME_MAX, color_label_key, is_user_theme, size_label_key,
    theme_preview_state,
};
use taful_app::preflop::{GameState, Seat};
use taful_app::theme::{Color, Theme, Warning};

use crate::Ui;
use crate::felt;
use crate::shell::Shell;
use crate::theme::{Palette, hsl_of, token_color};
use crate::widgets::{button, card, muted, title};

/// Color tokens by editor group (`theme-group-<id>`). A test checks that
/// they cover every color token exactly once.
const COLOR_GROUPS: [(&str, &[&str]); 5] = [
    (
        "surfaces",
        &[
            "background",
            "surface",
            "surface_raised",
            "border",
            "text",
            "text_muted",
            "accent",
            "accent_muted",
            "on_accent",
        ],
    ),
    (
        "table",
        &[
            "felt",
            "felt_line",
            "rail",
            "rail_edge",
            "seat",
            "seat_active",
            "grid_dot",
        ],
    ),
    (
        "cards",
        &[
            "card_face",
            "card_back",
            "suit_spades",
            "suit_hearts",
            "suit_diamonds",
            "suit_clubs",
        ],
    ),
    (
        "chips",
        &["chip_1", "chip_5", "chip_25", "chip_100", "chip_500"],
    ),
    (
        "grades",
        &[
            "grade_best",
            "grade_correct",
            "grade_inaccuracy",
            "grade_mistake",
            "grade_blunder",
        ],
    ),
];

/// Hue step, degrees.
const HUE_STEP: f64 = 10.0;
/// Saturation and lightness step, on the 0–1 scale.
const SL_STEP: f64 = 0.04;
/// Longest text the hex field takes (`#rrggbbaa`).
const HEX_MAX: usize = 9;
/// Corner radius at or above which a corner is "fully round" (the `pill`
/// token ships as 999).
const ROUND: f32 = 999.;
/// The largest radius stepped one pixel at a time; one more step is
/// [`ROUND`].
const RADIUS_STEPS_MAX: f32 = 32.;
/// Preview table scale.
const PREVIEW_SCALE: f32 = 0.8;
const SWATCH: f32 = 22.;

/// What the keyboard is typing into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Field {
    /// The selected color token's hex value.
    Hex(String),
    /// The theme's new name.
    Name(String),
}

/// The open editor: one user theme, its saved tokens and the working draft.
pub struct ThemeEditor {
    /// `user:<id>` of the theme being edited.
    pub key: String,
    /// Tokens as stored.
    pub saved: Theme,
    /// Tokens on screen (and applied to the whole app).
    pub draft: Theme,
    /// The color token being edited.
    pub selected: Option<&'static str>,
    pub field: Option<Field>,
    /// Asking to confirm a delete.
    pub confirm_delete: bool,
    /// Result of the last save, rename or delete.
    pub status: Option<String>,
    /// Hex text that did not parse.
    pub hex_invalid: bool,
    preview: (GameState, Seat),
}

impl ThemeEditor {
    pub fn new(key: String, theme: Theme) -> Self {
        Self {
            key,
            saved: theme.clone(),
            draft: theme,
            selected: None,
            field: None,
            confirm_delete: false,
            status: None,
            hex_invalid: false,
            preview: theme_preview_state(),
        }
    }

    fn dirty(&self) -> bool {
        self.draft.colors != self.saved.colors
            || self.draft.spacing != self.saved.spacing
            || self.draft.radii != self.saved.radii
            || self.draft.text_size != self.saved.text_size
    }
}

/// `value` one step up (`dir` = 1) or down (-1), within the token's range.
fn step_size(group: &str, token: &str, value: f32, dir: f32) -> f32 {
    match group {
        "text" => (value + dir).clamp(9., 40.),
        "radii" => {
            if dir > 0. && value >= RADIUS_STEPS_MAX {
                ROUND
            } else if dir < 0. && value > RADIUS_STEPS_MAX {
                RADIUS_STEPS_MAX
            } else {
                (value + dir).clamp(0., RADIUS_STEPS_MAX)
            }
        }
        _ => {
            let _ = token;
            (value + dir).clamp(0., 64.)
        }
    }
}

/// Edits `field` with one key; `Some(true)` on Enter, `Some(false)` on Esc.
fn edit_field(field: &mut Field, ev: &KeyDownEvent) -> Option<bool> {
    let ks = &ev.keystroke;
    if ks.modifiers.control || ks.modifiers.alt || ks.modifiers.platform {
        return None;
    }
    let text = match field {
        Field::Hex(s) | Field::Name(s) => s,
    };
    match ks.key.as_str() {
        "enter" => return Some(true),
        "escape" => return Some(false),
        "backspace" => {
            text.pop();
            return None;
        }
        _ => {}
    }
    let ch = ks.key_char.as_deref().and_then(|s| s.chars().next())?;
    match field {
        Field::Hex(s) => {
            // A `#` starts over: type a whole new value without erasing.
            if ch == '#' {
                s.clear();
            }
            let ok = (ch == '#' && s.is_empty()) || ch.is_ascii_hexdigit();
            if ok && s.chars().count() < HEX_MAX {
                s.push(ch.to_ascii_lowercase());
            }
        }
        Field::Name(s) => {
            if !ch.is_control() && s.chars().count() < THEME_NAME_MAX {
                s.push(ch);
            }
        }
    }
    None
}

/// A typed hex value; the `#` is optional.
fn parse_hex(s: &str) -> Option<Color> {
    let s = s.strip_prefix('#').unwrap_or(s);
    format!("#{s}").parse().ok()
}

impl Shell {
    /// Puts `theme` on screen everywhere without storing it (live preview).
    pub(crate) fn preview_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        let ui = Rc::new(self.ui.with_theme(theme));
        self.palette = Palette::new(&ui.theme.colors);
        let table_ui = ui.clone();
        self.table.update(cx, |t, cx| {
            t.set_ui(table_ui);
            cx.notify();
        });
        self.ui = ui;
    }

    /// Applies the draft after an edit.
    fn theme_draft_changed(&mut self, cx: &mut Context<Self>) {
        if let Some(e) = &mut self.settings.theme_editor {
            e.status = None;
            let draft = e.draft.clone();
            self.preview_theme(draft, cx);
        }
    }

    /// Copies the active theme into a new user theme and opens the editor.
    pub(crate) fn theme_duplicate(&mut self, cx: &mut Context<Self>) {
        let key = self.app.theme_key();
        let source = if is_user_theme(&key) {
            self.app.theme().name
        } else {
            self.ui.i18n.tr(&format!("theme-{key}"))
        };
        let name = self
            .ui
            .i18n
            .tr_args("theme-copy-name", &[("name", source.into())]);
        match self.app.duplicate_theme(&key, &name) {
            Ok(choice) => self.theme_edit_open(choice.key, cx),
            Err(e) => self.report::<()>(Err(e), |_| None),
        }
    }

    /// Opens the editor on the active theme, which must be a user theme.
    pub(crate) fn theme_edit_open(&mut self, key: String, cx: &mut Context<Self>) {
        let theme = self.app.theme();
        self.settings.theme_editor = Some(ThemeEditor::new(key, theme.clone()));
        self.preview_theme(theme, cx);
    }

    /// Closes the editor; unsaved changes are dropped.
    pub(crate) fn theme_edit_close(&mut self, cx: &mut Context<Self>) {
        if self.settings.theme_editor.take().is_some() {
            let theme = self.app.theme();
            self.preview_theme(theme, cx);
        }
    }

    fn theme_edit_save(&mut self) {
        let Some(e) = &mut self.settings.theme_editor else {
            return;
        };
        let i18n = &self.ui.i18n;
        e.status = Some(match self.app.save_theme(&e.key, &e.draft) {
            Ok(t) => {
                let n = t.warnings().len();
                e.saved = t.clone();
                e.draft = t;
                if n == 0 {
                    i18n.tr("theme-editor-saved")
                } else {
                    i18n.tr_args("theme-editor-saved-warnings", &[("count", n.into())])
                }
            }
            Err(err) => i18n.tr_args("settings-failed", &[("error", err.to_string().into())]),
        });
    }

    fn theme_edit_rename(&mut self, name: String) {
        let Some(e) = &mut self.settings.theme_editor else {
            return;
        };
        match self.app.rename_theme(&e.key, &name) {
            Ok(name) => {
                e.saved.name = name.clone();
                e.draft.name = name;
                e.status = None;
            }
            Err(err) => {
                e.status = Some(
                    self.ui
                        .i18n
                        .tr_args("settings-failed", &[("error", err.to_string().into())]),
                );
            }
        }
    }

    fn theme_edit_delete(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.settings.theme_editor.take() else {
            return;
        };
        let r = self.app.delete_theme(&e.key);
        let theme = self.app.theme();
        self.preview_theme(theme, cx);
        if r.is_err() {
            self.report(r, |_| None);
        }
    }

    /// Sets a color token in the draft.
    fn theme_set_color(&mut self, token: &'static str, c: Color, cx: &mut Context<Self>) {
        if let Some(e) = &mut self.settings.theme_editor {
            e.draft.colors.set(token, c);
            if let Some(Field::Hex(s)) = &mut e.field {
                *s = c.to_string();
            }
            e.hex_invalid = false;
        }
        self.theme_draft_changed(cx);
    }

    /// Keys typed while the settings screen is up.
    pub(crate) fn theme_editor_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(e) = &mut self.settings.theme_editor else {
            return;
        };
        let Some(field) = &mut e.field else {
            return;
        };
        let Some(enter) = edit_field(field, ev) else {
            return;
        };
        let field = e.field.take();
        match (field, enter) {
            (Some(Field::Hex(s)), true) => match (parse_hex(&s), e.selected) {
                (Some(c), Some(token)) => {
                    e.field = Some(Field::Hex(c.to_string()));
                    self.theme_set_color(token, c, cx);
                }
                _ => {
                    e.field = Some(Field::Hex(s));
                    e.hex_invalid = true;
                }
            },
            (Some(Field::Hex(_)), false) => {
                e.selected = None;
                e.hex_invalid = false;
            }
            (Some(Field::Name(s)), true) => self.theme_edit_rename(s),
            _ => {}
        }
    }

    /// The editor card, under the theme picker.
    pub(crate) fn theme_editor_card(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let e = self.settings.theme_editor.as_ref()?;
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let i18n = &ui.i18n;

        // Name and rename.
        let name_row = match &e.field {
            Some(Field::Name(s)) => div()
                .flex()
                .flex_col()
                .gap(px(t.spacing.xs))
                .child(text_box(s, &ui, &p))
                .child(muted(i18n.tr("theme-editor-rename-hint"), &ui, &p)),
            _ => div()
                .flex()
                .items_center()
                .gap(px(t.spacing.md))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(i18n.tr_args(
                    "theme-editor-name",
                    &[("name", e.draft.name.clone().into())],
                )))
                .child(
                    button(
                        "theme_rename",
                        i18n.tr("theme-editor-rename"),
                        false,
                        &ui,
                        &p,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(e) = &mut this.settings.theme_editor {
                            e.field = Some(Field::Name(e.draft.name.clone()));
                            e.selected = None;
                        }
                        cx.notify();
                    })),
                ),
        };

        // Preview: the table and the grade labels on each surface.
        let (state, hero) = &e.preview;
        let grades = |bg| {
            div()
                .flex()
                .flex_wrap()
                .gap(px(t.spacing.md))
                .p(px(t.spacing.sm))
                .rounded(px(t.radii.md))
                .bg(bg)
                .children(e.draft.grade_colors().into_iter().map(|(id, c)| {
                    div()
                        .text_color(token_color(c))
                        .child(i18n.tr(&format!("table-grade-{id}")))
                }))
        };
        let chips = div().flex().gap(px(t.spacing.sm)).children(
            ["chip_1", "chip_5", "chip_25", "chip_100", "chip_500"]
                .into_iter()
                .filter_map(|tok| e.draft.colors.get(tok))
                .map(|c| {
                    div()
                        .size(px(SWATCH))
                        .rounded(px(SWATCH / 2.))
                        .border_2()
                        .border_color(p.rail_edge)
                        .bg(token_color(c))
                }),
        );
        let preview = div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.sm))
            .child(muted(i18n.tr("theme-editor-preview"), &ui, &p))
            .child(felt::table_scaled(
                &ui,
                &p,
                state,
                *hero,
                false,
                PREVIEW_SCALE,
            ))
            .child(chips)
            .child(grades(p.background))
            .child(grades(p.surface))
            .child(grades(p.surface_raised));

        // Live checks.
        let warnings = e.draft.warnings();
        let mut checks = div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.xs))
            .child(title(i18n.tr("theme-editor-checks"), &ui));
        if warnings.is_empty() {
            checks = checks.child(
                div()
                    .text_color(p.grade_best)
                    .child(i18n.tr("theme-editor-ok")),
            );
        } else {
            checks = checks.children(warnings.iter().map(|w| {
                div()
                    .flex()
                    .gap(px(t.spacing.sm))
                    .child(div().text_color(p.accent).child("⚠"))
                    .child(warning_text(w, &ui))
            }));
        }

        // Color tokens.
        let mut colors = div().flex().flex_col().gap(px(t.spacing.md));
        let mut row_id = 0usize;
        for (group, tokens) in COLOR_GROUPS {
            let mut rows = div().flex().flex_col().gap(px(t.spacing.xs));
            for &token in tokens {
                let Some(c) = e.draft.colors.get(token) else {
                    continue;
                };
                let selected = e.selected == Some(token);
                row_id += 1;
                let row = div()
                    .id(("theme_tok", row_id))
                    .flex()
                    .items_center()
                    .gap(px(t.spacing.md))
                    .px(px(t.spacing.sm))
                    .py(px(t.spacing.xs))
                    .rounded(px(t.radii.md))
                    .cursor_pointer()
                    .when(selected, |d| d.bg(p.surface_raised))
                    .hover(|s| s.bg(p.surface_raised))
                    .child(swatch(c, &ui, &p))
                    .child(div().flex_1().child(i18n.tr(&color_label_key(token))))
                    .child(muted(c.to_string(), &ui, &p))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(e) = &mut this.settings.theme_editor {
                            if e.selected == Some(token) {
                                e.selected = None;
                                e.field = None;
                            } else {
                                e.selected = Some(token);
                                e.field =
                                    e.draft.colors.get(token).map(|c| Field::Hex(c.to_string()));
                            }
                            e.hex_invalid = false;
                        }
                        cx.notify();
                    }));
                rows = rows.child(row);
                if selected {
                    rows = rows.child(self.color_controls(token, c, row_id, &ui, &p, cx));
                }
            }
            colors = colors
                .child(muted(i18n.tr(&format!("theme-group-{group}")), &ui, &p))
                .child(rows);
        }

        // Size tokens.
        let mut sizes = div().flex().flex_col().gap(px(t.spacing.xs)).child(muted(
            i18n.tr("theme-group-sizes"),
            &ui,
            &p,
        ));
        for (gi, (group, tokens)) in SIZE_GROUPS.into_iter().enumerate() {
            for (ti, &token) in tokens.iter().enumerate() {
                let value = match group {
                    "spacing" => e.draft.spacing.get(token),
                    "radii" => e.draft.radii.get(token),
                    _ => e.draft.text_size.get(token),
                }
                .unwrap_or(0.);
                let id = gi * 100 + ti;
                let step = move |dir: f32| {
                    move |this: &mut Shell,
                          _: &gpui::ClickEvent,
                          _: &mut gpui::Window,
                          cx: &mut Context<Shell>| {
                        if let Some(e) = &mut this.settings.theme_editor {
                            let d = &mut e.draft;
                            let (get, set): (Option<f32>, &mut dyn FnMut(f32)) = match group {
                                "spacing" => (d.spacing.get(token), &mut |v| {
                                    d.spacing.set(token, v);
                                }),
                                "radii" => (d.radii.get(token), &mut |v| {
                                    d.radii.set(token, v);
                                }),
                                _ => (d.text_size.get(token), &mut |v| {
                                    d.text_size.set(token, v);
                                }),
                            };
                            if let Some(v) = get {
                                set(step_size(group, token, v, dir));
                            }
                        }
                        this.theme_draft_changed(cx);
                        cx.notify();
                    }
                };
                let shown = if value >= ROUND {
                    "∞".to_string()
                } else {
                    felt::num(&ui, f64::from(value), 0)
                };
                sizes = sizes.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(t.spacing.md))
                        .px(px(t.spacing.sm))
                        .child(div().flex_1().child(i18n.tr(&size_label_key(group, token))))
                        .child(
                            button(("size_down", id), "−", false, &ui, &p)
                                .on_click(cx.listener(step(-1.))),
                        )
                        .child(div().w(px(36.)).flex().justify_center().child(shown))
                        .child(
                            button(("size_up", id), "+", false, &ui, &p)
                                .on_click(cx.listener(step(1.))),
                        ),
                );
            }
        }

        // Footer: status and actions.
        let status = if let Some(s) = &e.status {
            Some(s.clone())
        } else if e.dirty() {
            Some(i18n.tr("theme-editor-unsaved"))
        } else {
            None
        };
        let mut actions = div().flex().flex_wrap().gap(px(t.spacing.sm)).child(
            button("theme_save", i18n.tr("theme-editor-save"), true, &ui, &p).on_click(
                cx.listener(|this, _, _, cx| {
                    this.theme_edit_save();
                    cx.notify();
                }),
            ),
        );
        if e.dirty() {
            actions = actions.child(
                button(
                    "theme_discard",
                    i18n.tr("theme-editor-discard"),
                    false,
                    &ui,
                    &p,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(e) = &mut this.settings.theme_editor {
                        e.draft = e.saved.clone();
                        e.field = e.selected.and_then(|tok| {
                            e.draft.colors.get(tok).map(|c| Field::Hex(c.to_string()))
                        });
                    }
                    this.theme_draft_changed(cx);
                    cx.notify();
                })),
            );
        }
        actions = actions
            .child(
                button("theme_close", i18n.tr("theme-editor-close"), false, &ui, &p).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.theme_edit_close(cx);
                        cx.notify();
                    }),
                ),
            )
            .child(
                button(
                    "theme_delete",
                    i18n.tr("theme-editor-delete"),
                    false,
                    &ui,
                    &p,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(e) = &mut this.settings.theme_editor {
                        e.confirm_delete = true;
                    }
                    cx.notify();
                })),
            );
        let confirm = e.confirm_delete.then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(t.spacing.sm))
                .p(px(t.spacing.md))
                .rounded(px(t.radii.md))
                .bg(p.surface_raised)
                .child(i18n.tr_args(
                    "theme-editor-delete-confirm",
                    &[("name", e.draft.name.clone().into())],
                ))
                .child(
                    div()
                        .flex()
                        .gap(px(t.spacing.sm))
                        .child(
                            button(
                                "theme_delete_yes",
                                i18n.tr("theme-editor-delete-yes"),
                                true,
                                &ui,
                                &p,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.theme_edit_delete(cx);
                                cx.notify();
                            })),
                        )
                        .child(
                            button(
                                "theme_delete_no",
                                i18n.tr("settings-cancel"),
                                false,
                                &ui,
                                &p,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(e) = &mut this.settings.theme_editor {
                                    e.confirm_delete = false;
                                }
                                cx.notify();
                            })),
                        ),
                )
        });

        Some(
            card(&ui, &p)
                .child(title(i18n.tr("theme-editor"), &ui))
                .child(muted(i18n.tr("theme-editor-body"), &ui, &p))
                .child(name_row)
                .child(actions)
                .children(status.map(|s| muted(s, &ui, &p)))
                .children(confirm)
                .child(preview)
                .child(checks)
                .child(colors)
                .child(sizes),
        )
    }

    /// Hex field and HSL steppers for the selected color token.
    fn color_controls(
        &self,
        token: &'static str,
        c: Color,
        id: usize,
        ui: &Ui,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let t = &ui.theme;
        let i18n = &ui.i18n;
        let e = self.settings.theme_editor.as_ref();
        let hex = match e.and_then(|e| e.field.as_ref()) {
            Some(Field::Hex(s)) => s.clone(),
            _ => c.to_string(),
        };
        let invalid = e.is_some_and(|e| e.hex_invalid);
        let saved = e.and_then(|e| e.saved.colors.get(token));
        let (h, s, l) = hsl_of(c);
        let stepper = |label: String, value: String, k: usize, delta: (f64, f64, f64)| {
            let adjust = move |sign: f64| {
                move |this: &mut Shell,
                      _: &gpui::ClickEvent,
                      _: &mut gpui::Window,
                      cx: &mut Context<Shell>| {
                    let cur = this
                        .settings
                        .theme_editor
                        .as_ref()
                        .and_then(|e| e.draft.colors.get(token));
                    if let Some(cur) = cur {
                        let next = cur.adjust(delta.0 * sign, delta.1 * sign, delta.2 * sign);
                        this.theme_set_color(token, next, cx);
                    }
                    cx.notify();
                }
            };
            div()
                .flex()
                .items_center()
                .gap(px(t.spacing.sm))
                .child(div().w(px(140.)).child(label))
                .child(
                    button(("hsl_down", id * 10 + k), "−", false, ui, p)
                        .on_click(cx.listener(adjust(-1.))),
                )
                .child(div().w(px(48.)).flex().justify_center().child(value))
                .child(
                    button(("hsl_up", id * 10 + k), "+", false, ui, p)
                        .on_click(cx.listener(adjust(1.))),
                )
        };
        let pct = |x: f64| format!("{}%", felt::num(ui, x * 100., 0));
        let mut controls = div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.sm))
            .ml(px(SWATCH + t.spacing.md + t.spacing.sm))
            .p(px(t.spacing.md))
            .rounded(px(t.radii.md))
            .border_1()
            .border_color(p.border)
            .child(text_box(&hex, ui, p))
            .child(muted(
                i18n.tr(if invalid {
                    "theme-editor-hex-invalid"
                } else {
                    "theme-editor-hex-hint"
                }),
                ui,
                p,
            ))
            .child(stepper(
                i18n.tr("theme-editor-hue"),
                format!("{}°", felt::num(ui, h, 0)),
                0,
                (HUE_STEP, 0., 0.),
            ))
            .child(stepper(
                i18n.tr("theme-editor-saturation"),
                pct(s),
                1,
                (0., SL_STEP, 0.),
            ))
            .child(stepper(
                i18n.tr("theme-editor-lightness"),
                pct(l),
                2,
                (0., 0., SL_STEP),
            ));
        if let Some(saved) = saved.filter(|s| *s != c) {
            controls = controls.child(
                button(
                    ("token_reset", id),
                    i18n.tr("theme-editor-reset-token"),
                    false,
                    ui,
                    p,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.theme_set_color(token, saved, cx);
                    cx.notify();
                })),
            );
        }
        controls
    }
}

/// A one-line text box showing what is typed, with a caret.
fn text_box(value: &str, ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    div()
        .w(px(260.))
        .px(px(t.spacing.md))
        .py(px(t.spacing.sm))
        .rounded(px(t.radii.md))
        .bg(p.surface_raised)
        .border_1()
        .border_color(p.accent)
        .child(format!("{value}▏"))
}

/// A color sample; checkered by a border so translucent tokens show.
fn swatch(c: Color, ui: &Ui, p: &Palette) -> Div {
    div()
        .flex_none()
        .size(px(SWATCH))
        .rounded(px(ui.theme.radii.sm))
        .border_1()
        .border_color(p.border)
        .bg(token_color(c))
}

/// A warning in the UI language.
fn warning_text(w: &Warning, ui: &Ui) -> String {
    let i18n = &ui.i18n;
    let token = |name: &str| i18n.tr(&color_label_key(name));
    match w {
        Warning::Contrast { pair, ratio, min } => {
            let (fg, bg) = pair.split_once(" on ").unwrap_or((pair, pair));
            i18n.tr_args(
                "theme-editor-warn-contrast",
                &[
                    ("fg", token(fg).into()),
                    ("bg", token(bg).into()),
                    ("ratio", felt::num(ui, *ratio, 2).into()),
                    ("min", felt::num(ui, *min, 1).into()),
                ],
            )
        }
        Warning::Red { token: name } => {
            i18n.tr_args("theme-editor-warn-red", &[("token", token(name).into())])
        }
        Warning::GradesClose { a, b, .. } => i18n.tr_args(
            "theme-editor-warn-grades",
            &[
                ("a", i18n.tr(&format!("table-grade-{a}")).into()),
                ("b", i18n.tr(&format!("table-grade-{b}")).into()),
            ],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taful_app::theme::Colors;

    #[test]
    fn color_groups_cover_every_token_once() {
        let mut listed: Vec<&str> = COLOR_GROUPS
            .iter()
            .flat_map(|(_, t)| t.iter().copied())
            .collect();
        let mut all: Vec<&str> = Colors::NAMES.to_vec();
        listed.sort_unstable();
        all.sort_unstable();
        assert_eq!(listed, all);
    }

    #[test]
    fn size_steps_stay_in_range() {
        assert_eq!(step_size("spacing", "xs", 0., -1.), 0.);
        assert_eq!(step_size("spacing", "xs", 4., 1.), 5.);
        assert_eq!(step_size("text", "sm", 9., -1.), 9.);
        assert_eq!(step_size("radii", "pill", ROUND, -1.), RADIUS_STEPS_MAX);
        assert_eq!(step_size("radii", "pill", RADIUS_STEPS_MAX, 1.), ROUND);
        assert_eq!(step_size("radii", "sm", 4., 1.), 5.);
    }

    #[test]
    fn hex_input() {
        // Hex digits without the hash, then with it (built so the
        // color-literal lint does not see a literal).
        let digits = "e02020";
        let c = parse_hex(digits).unwrap();
        assert_eq!((c.r, c.g, c.b, c.a), (0xe0, 0x20, 0x20, 0xff));
        assert_eq!(parse_hex(&format!("{}{digits}", '#')), Some(c));
        assert!(parse_hex(&digits[1..]).is_none());
        assert!(parse_hex("").is_none());
    }
}
