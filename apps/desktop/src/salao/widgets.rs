//! Salão's widgets (the hooks `widgets.rs` dispatches): every screen the
//! mockup does not show (lesson reader, calc, reviews, settings, hand
//! review) is drawn with these, so it takes the style too.

use gpui::{AlignSelf, Div, ElementId, FontWeight, SharedString, Stateful, div, prelude::*, px};

use super::colors::colors;
use super::kit;
use crate::Ui;
use crate::theme::{Palette, alpha};

/// A gold button for the main action, a ghost one otherwise.
pub fn button(
    id: ElementId,
    label: SharedString,
    primary: bool,
    _ui: &Ui,
    p: &Palette,
) -> Stateful<Div> {
    let mut b = kit::btn(id, label, primary, false, p);
    // Keep its own width inside column layouts.
    b.style().align_self = Some(AlignSelf::FlexStart);
    b
}

/// A pill.
pub fn chip(
    id: ElementId,
    label: SharedString,
    selected: bool,
    _ui: &Ui,
    p: &Palette,
) -> Stateful<Div> {
    kit::pill(id, label, selected, p)
}

/// A lit panel.
pub fn card(ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    kit::panel(p)
        .flex()
        .flex_col()
        .gap(px(t.spacing.md))
        .px(px(26.))
        .py(px(20.))
}

/// A panel title, in the serif.
pub fn title(text: SharedString, _ui: &Ui) -> Div {
    kit::TITLE.text(23., kit::LEADING).child(text)
}

/// Secondary text.
pub fn muted(text: SharedString, ui: &Ui, p: &Palette) -> Div {
    kit::sans_text(ui.theme.text_size.md, FontWeight::NORMAL)
        .text_color(p.text_muted)
        .child(text)
}

/// The answer box: a sunk field with a gold edge.
pub fn answer_box(value: &str, hint: SharedString, ui: &Ui, p: &Palette) -> Div {
    let (c, t) = (colors(), &ui.theme);
    div()
        .flex()
        .flex_col()
        .gap(px(t.spacing.xs))
        .child(
            div()
                .w(px(220.))
                .px(px(t.spacing.md))
                .py(px(t.spacing.sm))
                .rounded(px(9.))
                .bg(c.well)
                .border_1()
                .border_color(alpha(p.accent, 0.55))
                .shadow(vec![kit::shadow(alpha(p.accent, 0.12), 0., 0., 14., 0.)])
                .child(kit::TEXT.text(22., kit::LEADING).child(format!("{value}▏"))),
        )
        .child(
            kit::sans_text(t.text_size.sm, FontWeight::NORMAL)
                .text_color(p.text_muted)
                .child(hint),
        )
}

/// A thin gold progress bar, `frac` in 0–1.
pub fn progress(frac: f32, _ui: &Ui, p: &Palette) -> Div {
    let c = colors();
    kit::bar(frac, 5., alpha(p.text, 0.09), p.accent, c.gold2).w_full()
}

/// A small tracked gold label.
pub fn eyebrow(text: SharedString, _ui: &Ui, p: &Palette) -> Div {
    kit::eyebrow(text, p.accent)
}

/// The track of a segmented control.
pub fn segmented(_ui: &Ui, p: &Palette) -> Div {
    kit::seg(p)
}

/// One option of a segmented control.
pub fn segment(
    id: ElementId,
    label: SharedString,
    selected: bool,
    _ui: &Ui,
    p: &Palette,
) -> Stateful<Div> {
    kit::seg_item(id, label, selected, p)
}

/// An empty state: what is (not) here, what fills it, and what to do now.
pub fn empty_state(title_text: SharedString, body: SharedString, ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    card(ui, p)
        .gap(px(t.spacing.sm))
        .py(px(t.spacing.xxl))
        .child(title(title_text, ui))
        .child(muted(body, ui, p))
}

/// A heading for a group of controls inside a panel.
pub fn subhead(text: SharedString, ui: &Ui) -> Div {
    kit::sans_text(ui.theme.text_size.md, FontWeight::SEMIBOLD).child(text)
}

/// An on/off switch with its label and an optional explanation below.
pub fn toggle(
    id: ElementId,
    label: SharedString,
    body: Option<SharedString>,
    on: bool,
    ui: &Ui,
    p: &Palette,
) -> Stateful<Div> {
    let (c, t) = (colors(), &ui.theme);
    const TRACK_W: f32 = 36.;
    const TRACK_H: f32 = 20.;
    const KNOB: f32 = 14.;
    let track = div()
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .w(px(TRACK_W))
        .h(px(TRACK_H))
        .px(px(2.))
        .rounded(px(TRACK_H / 2.))
        .border_1()
        .when(on, |d| {
            d.justify_end()
                .bg(p.accent)
                .border_color(p.accent)
                .children(kit::gold_fill(TRACK_H / 2. - 1., 0.6, p))
        })
        .when(!on, |d| d.bg(c.well).border_color(c.line2))
        .child(
            div()
                .relative()
                .size(px(KNOB))
                .rounded(px(KNOB / 2.))
                .bg(if on { p.on_accent } else { p.text_muted }),
        );
    div()
        .id(id)
        .flex()
        .items_start()
        .gap(px(t.spacing.md))
        .cursor_pointer()
        .child(div().pt(px(2.)).child(track))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(t.spacing.xs))
                .child(kit::sans_text(t.text_size.md, FontWeight::NORMAL).child(label))
                .children(body.map(|b| muted(b, ui, p))),
        )
}
