//! The shared widgets as Baralho draws them, for the screens on a cream
//! sheet (and the tour and update cards over any screen): raised buttons,
//! outlined pills and panels, display type for titles.

use gpui::{AlignSelf, Div, ElementId, SharedString, Stateful, div, prelude::*, px, relative};

use super::kit::{self, Fill};
use super::{bx, typo, u};
use crate::Ui;
use crate::theme::Palette;

/// A button: amber when `primary`, cream otherwise.
pub fn button(
    id: ElementId,
    label: SharedString,
    primary: bool,
    ui: &Ui,
    _: &Palette,
) -> Stateful<Div> {
    let fill = if primary { Fill::Amber } else { Fill::Cream };
    // Here the drop takes room of its own (the kit's buttons hang it below
    // their box, which a column of a panel does not allow for).
    let mut b = kit::btn(id, label, fill, true, &bx(ui)).mb(px(0.));
    // Keep its own width inside column layouts.
    b.style().align_self = Some(AlignSelf::FlexStart);
    b
}

/// A selectable pill (exercise picker, language).
pub fn chip(
    id: ElementId,
    label: SharedString,
    selected: bool,
    ui: &Ui,
    _: &Palette,
) -> Stateful<Div> {
    let bx = bx(ui);
    let lit = bx.cream2;
    typo::text(13.5, 700)
        .on(div())
        .id(id)
        .flex_none()
        .px(u(12.))
        .py(u(4.))
        .rounded_full()
        .border(u(2.))
        .border_color(bx.deep)
        .text_color(bx.ink)
        .cursor_pointer()
        .map(|d| {
            if selected {
                d.bg(bx.amber)
            } else {
                d.bg(bx.cream_hi).hover(move |s| s.bg(lit))
            }
        })
        .child(label)
}

/// An outlined panel inside a sheet.
pub fn card(ui: &Ui, _: &Palette) -> Div {
    let bx = bx(ui);
    div()
        .flex()
        .flex_col()
        .gap(u(12.))
        .p(u(22.))
        .rounded(u(14.))
        .bg(bx.cream_hi)
        .border(u(2.))
        .border_color(bx.deep)
        .text_color(bx.ink)
}

/// A section title inside a panel.
pub fn title(text: SharedString, _: &Ui) -> Div {
    typo::disp(22., 800, 25).lh(1.15).on(div()).child(text)
}

/// Secondary text.
pub fn muted(text: SharedString, ui: &Ui, _: &Palette) -> Div {
    typo::text(14.5, 500)
        .on(div())
        .text_color(bx(ui).mut_ink)
        .child(text)
}

/// The answer box: what was typed, with a caret. Keys are handled by the
/// shell.
pub fn answer_box(value: &str, hint: SharedString, ui: &Ui, _: &Palette) -> Div {
    let bx = bx(ui);
    div()
        .flex()
        .flex_col()
        .gap(u(6.))
        .child(
            typo::disp(24., 800, 25)
                .lh(1.3)
                .on(div())
                .w(u(230.))
                .px(u(14.))
                .py(u(7.))
                .rounded(u(12.))
                .bg(bx.cream_hi)
                .border(u(2.5))
                .border_color(bx.deep)
                .text_color(bx.ink)
                .child(format!("{value}▏")),
        )
        .child(
            typo::text(13., 500)
                .on(div())
                .text_color(bx.mut_ink)
                .child(hint),
        )
}

/// A progress bar, `frac` in 0–1: the amber fill of the mockup's bars.
pub fn progress(frac: f32, ui: &Ui, _: &Palette) -> Div {
    let bx = bx(ui);
    let frac = frac.clamp(0., 1.);
    div()
        .w_full()
        .h(u(14.))
        .rounded(u(7.))
        .bg(bx.deep)
        .p(u(2.))
        .child(
            div()
                .size_full()
                .rounded(u(5.))
                .bg(bx.cream2)
                .overflow_hidden()
                .when(frac > 0., |d| {
                    d.child(
                        div()
                            .h_full()
                            .w(relative(frac))
                            .bg(bx.amber)
                            .border_r(u(2.))
                            .border_color(bx.deep),
                    )
                }),
        )
}

/// A small tracked label above a panel's content.
pub fn eyebrow(text: SharedString, _: &Ui, p: &Palette) -> Div {
    typo::caps(11.5, 800, 120)
        .on(div())
        .text_color(p.accent)
        .child(text.to_uppercase())
}

/// A segmented control's track.
pub fn segmented(ui: &Ui, _: &Palette) -> Div {
    let bx = bx(ui);
    div()
        .flex()
        .flex_none()
        .p(u(4.))
        .rounded_full()
        .bg(bx.cream2)
        .border(u(2.))
        .border_color(bx.deep)
}

/// One option of a [`segmented`] control.
pub fn segment(
    id: ElementId,
    label: SharedString,
    selected: bool,
    ui: &Ui,
    _: &Palette,
) -> Stateful<Div> {
    let bx = bx(ui);
    let ink = bx.ink;
    typo::text(14., 700)
        .on(div())
        .id(id)
        .px(u(16.))
        .py(u(5.))
        .rounded_full()
        .whitespace_nowrap()
        .cursor_pointer()
        .map(|d| {
            if selected {
                // The ring around the lit segment takes no room.
                d.bg(bx.amber)
                    .text_color(bx.ink)
                    .border(u(2.))
                    .border_color(bx.deep)
                    .m(u(-2.))
            } else {
                d.text_color(bx.ink2).hover(move |s| s.text_color(ink))
            }
        })
        .child(label)
}

/// An empty state: what is (not) here, what fills it, and what to do now.
pub fn empty_state(title_text: SharedString, body: SharedString, ui: &Ui, p: &Palette) -> Div {
    card(ui, p)
        .gap(u(8.))
        .py(u(30.))
        .child(title(title_text, ui))
        .child(muted(body, ui, p))
}

/// A heading for a group of controls inside a panel.
pub fn subhead(text: SharedString, _: &Ui) -> Div {
    typo::text(15., 700).on(div()).child(text)
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
    let bx = bx(ui);
    let track = div()
        .flex_none()
        .flex()
        .items_center()
        .w(u(42.))
        .h(u(24.))
        .px(u(3.))
        .rounded_full()
        .border(u(2.))
        .border_color(bx.deep)
        .map(|d| {
            if on {
                d.justify_end().bg(bx.amber)
            } else {
                d.bg(bx.cream2)
            }
        })
        .child(
            div()
                .size(u(14.))
                .rounded_full()
                .bg(if on { bx.ink } else { bx.mut_ink }),
        );
    div()
        .id(id)
        .flex()
        .items_start()
        .gap(u(12.))
        .cursor_pointer()
        .child(track)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(u(4.))
                .child(typo::text(15., 600).on(div()).child(label))
                .children(body.map(|b| muted(b, ui, p))),
        )
}
