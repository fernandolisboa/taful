//! Small building blocks shared by the screens: buttons, cards, the answer
//! box, and the lesson document renderer with glossary tooltips.

use std::ops::Range;

use gpui::{
    AlignSelf, AnyElement, App, Context, Div, ElementId, FontStyle, FontWeight, HighlightStyle,
    InteractiveText, SharedString, Stateful, StyledText, UnderlineStyle, Window, div, prelude::*,
    px,
};
use taful_app::model::{Block, Document, Span};

use crate::Ui;
use crate::theme::Palette;

/// A clickable button. `primary` uses the accent fill.
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    ui: &Ui,
    p: &Palette,
) -> Stateful<Div> {
    let t = &ui.theme;
    let mut base = div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .px(px(t.spacing.lg))
        .py(px(t.spacing.sm))
        .rounded(px(t.radii.md))
        .text_size(px(t.text_size.md))
        .cursor_pointer()
        .child(label.into());
    // Keep its own width inside column layouts.
    base.style().align_self = Some(AlignSelf::FlexStart);
    if primary {
        base.bg(p.accent)
            .text_color(p.on_accent)
            .font_weight(FontWeight::SEMIBOLD)
            .hover(|s| s.opacity(0.9))
    } else {
        base.bg(p.surface_raised)
            .text_color(p.text)
            .border_1()
            .border_color(p.border)
            .hover(|s| s.border_color(p.accent))
    }
}

/// A selectable chip (exercise picker, language).
pub fn chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    ui: &Ui,
    p: &Palette,
) -> Stateful<Div> {
    let t = &ui.theme;
    div()
        .id(id.into())
        .px(px(t.spacing.md))
        .py(px(t.spacing.xs))
        .rounded(px(t.radii.pill))
        .border_1()
        .text_size(px(t.text_size.sm))
        .cursor_pointer()
        .when(selected, |d| {
            d.bg(p.accent)
                .border_color(p.accent)
                .text_color(p.on_accent)
                .font_weight(FontWeight::SEMIBOLD)
        })
        .when(!selected, |d| {
            d.bg(p.surface_raised)
                .border_color(p.border)
                .text_color(p.text)
                .hover(|s| s.border_color(p.accent))
        })
        .child(label.into())
}

/// A bordered panel.
pub fn card(ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    div()
        .flex()
        .flex_col()
        .gap(px(t.spacing.md))
        .p(px(t.spacing.xl))
        .rounded(px(t.radii.lg))
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
}

/// A section title inside a card.
pub fn title(text: impl Into<SharedString>, ui: &Ui) -> Div {
    div()
        .text_size(px(ui.theme.text_size.lg))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}

/// Secondary text.
pub fn muted(text: impl Into<SharedString>, ui: &Ui, p: &Palette) -> Div {
    div()
        .text_size(px(ui.theme.text_size.md))
        .text_color(p.text_muted)
        .child(text.into())
}

/// The answer box: shows what was typed with a caret. Keys are handled by
/// the shell (one focus target for the whole window).
pub fn answer_box(value: &str, hint: impl Into<SharedString>, ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    div()
        .flex()
        .flex_col()
        .gap(px(t.spacing.xs))
        .child(
            div()
                .w(px(220.))
                .px(px(t.spacing.md))
                .py(px(t.spacing.sm))
                .rounded(px(t.radii.md))
                .bg(p.surface_raised)
                .border_1()
                .border_color(p.accent)
                .text_size(px(t.text_size.lg))
                .child(format!("{value}▏")),
        )
        .child(
            div()
                .text_size(px(t.text_size.sm))
                .text_color(p.text_muted)
                .child(hint.into()),
        )
}

/// Width of a glossary tooltip (a fixed width lets wrapped text measure).
const TIP_WIDTH: f32 = 360.;

/// Tooltip with a glossary definition.
pub struct TermTip {
    title: SharedString,
    body: SharedString,
    palette: Palette,
    pad: f32,
    radius: f32,
    size: (f32, f32),
}

impl Render for TermTip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let p = &self.palette;
        div()
            .w(px(TIP_WIDTH))
            .flex()
            .flex_col()
            .gap(px(self.pad / 2.))
            .p(px(self.pad))
            .rounded(px(self.radius))
            .bg(p.surface_raised)
            .border_1()
            .border_color(p.accent_muted)
            .text_color(p.text)
            .child(
                div()
                    .text_size(px(self.size.1))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.accent)
                    .child(self.title.clone()),
            )
            .child(div().text_size(px(self.size.0)).child(self.body.clone()))
    }
}

/// One paragraph of spans as styled text; glossary terms are underlined in
/// the accent color and show their definition on hover.
fn rich_text(id: ElementId, spans: &[Span], ui: &std::rc::Rc<Ui>, p: &Palette) -> AnyElement {
    let mut text = String::new();
    let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
    let mut terms: Vec<(Range<usize>, String)> = Vec::new();
    for s in spans {
        let start = text.len();
        text.push_str(&s.text);
        let range = start..text.len();
        if range.is_empty() {
            continue;
        }
        let mut h = HighlightStyle::default();
        let mut styled = false;
        if s.bold {
            h.font_weight = Some(FontWeight::BOLD);
            styled = true;
        }
        if s.italic {
            h.font_style = Some(FontStyle::Italic);
            styled = true;
        }
        if s.code {
            h.background_color = Some(p.surface_raised.into());
            styled = true;
        }
        if let Some(term) = &s.term {
            h.color = Some(p.accent.into());
            h.underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(p.accent_muted.into()),
                wavy: false,
            });
            styled = true;
            terms.push((range.clone(), term.clone()));
        }
        if styled {
            highlights.push((range, h));
        }
    }
    let styled = StyledText::new(text).with_highlights(highlights);
    if terms.is_empty() {
        return styled.into_any_element();
    }
    let ui = ui.clone();
    let palette = *p;
    InteractiveText::new(id, styled)
        .tooltip(move |ix, _window, cx: &mut App| {
            let (_, term) = terms.iter().find(|(r, _)| r.contains(&ix))?;
            let t = &ui.theme;
            let tip = TermTip {
                title: ui.i18n.tr(&format!("glossary-{term}")).into(),
                body: ui.i18n.tr(&format!("glossary-{term}-def")).into(),
                palette,
                pad: t.spacing.md,
                radius: t.radii.md,
                size: (t.text_size.md, t.text_size.lg),
            };
            Some(cx.new(|_| tip).into())
        })
        .into_any_element()
}

/// Renders a lesson document.
pub fn document(doc: &Document, ui: &std::rc::Rc<Ui>, p: &Palette) -> Div {
    let t = &ui.theme;
    let mut out = div().flex().flex_col().gap(px(t.spacing.md));
    for (i, b) in doc.blocks.iter().enumerate() {
        let el: AnyElement = match b {
            Block::Heading { level, spans } => div()
                .pt(px(if *level == 1 { 0. } else { t.spacing.sm }))
                .text_size(px(match level {
                    1 => t.text_size.xl,
                    2 => t.text_size.lg,
                    _ => t.text_size.md,
                }))
                .font_weight(FontWeight::BOLD)
                .text_color(if *level == 1 { p.accent } else { p.text })
                .child(rich_text(("h", i).into(), spans, ui, p))
                .into_any_element(),
            Block::Paragraph(spans) => div()
                .text_size(px(t.text_size.md))
                .line_height(px(t.text_size.md * 1.6))
                .child(rich_text(("p", i).into(), spans, ui, p))
                .into_any_element(),
            Block::List { ordered, items } => div()
                .flex()
                .flex_col()
                .gap(px(t.spacing.xs))
                .children(items.iter().enumerate().map(|(n, item)| {
                    let marker = if *ordered {
                        format!("{}.", n + 1)
                    } else {
                        "•".to_owned()
                    };
                    div()
                        .w_full()
                        .flex()
                        .gap(px(t.spacing.sm))
                        .text_size(px(t.text_size.md))
                        .line_height(px(t.text_size.md * 1.5))
                        .child(
                            div()
                                .flex_none()
                                .w(px(t.spacing.xl))
                                .text_color(p.accent)
                                .child(marker),
                        )
                        .child(div().flex_1().min_w_0().child(rich_text(
                            ("li", i * 1000 + n).into(),
                            item,
                            ui,
                            p,
                        )))
                }))
                .into_any_element(),
            Block::Quote(spans) => div()
                .pl(px(t.spacing.md))
                .border_l(px(3.))
                .border_color(p.accent_muted)
                .text_size(px(t.text_size.md))
                .text_color(p.text_muted)
                .child(rich_text(("q", i).into(), spans, ui, p))
                .into_any_element(),
            Block::Code(code) => div()
                .p(px(t.spacing.md))
                .rounded(px(t.radii.md))
                .bg(p.surface_raised)
                .text_size(px(t.text_size.sm))
                .child(code.clone())
                .into_any_element(),
            Block::Rule => div().h(px(1.)).bg(p.border).into_any_element(),
        };
        out = out.child(el);
    }
    out
}

/// A thin progress bar, `frac` in 0–1.
pub fn progress(frac: f32, ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    let frac = frac.clamp(0., 1.);
    div()
        .w_full()
        .h(px(6.))
        .rounded(px(t.radii.pill))
        .bg(p.surface_raised)
        .child(
            div()
                .h_full()
                .w(gpui::relative(frac))
                .rounded(px(t.radii.pill))
                .bg(p.accent),
        )
}

/// A small uppercase label above a card's content.
pub fn eyebrow(text: impl Into<SharedString>, ui: &Ui, p: &Palette) -> Div {
    div()
        .text_size(px(ui.theme.text_size.sm))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.accent)
        .child(text.into().to_uppercase())
}

/// A segmented control: one rounded track holding exclusive options.
pub fn segmented(ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    div()
        .flex()
        .flex_none()
        .p(px(3.))
        .gap(px(2.))
        .rounded(px(t.radii.md + 3.))
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
}

/// One option of a [`segmented`] control.
pub fn segment(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    ui: &Ui,
    p: &Palette,
) -> Stateful<Div> {
    let t = &ui.theme;
    div()
        .id(id.into())
        .px(px(t.spacing.lg))
        .py(px(t.spacing.xs + 2.))
        .rounded(px(t.radii.md))
        .text_size(px(t.text_size.md))
        .cursor_pointer()
        .when(selected, |d| {
            d.bg(p.accent)
                .text_color(p.on_accent)
                .font_weight(FontWeight::SEMIBOLD)
        })
        .when(!selected, |d| {
            d.text_color(p.text_muted)
                .hover(|s| s.bg(p.surface_raised).text_color(p.text))
        })
        .child(label.into())
}

/// An empty state: what is (not) here, what fills it, and what to do now.
pub fn empty_state(
    title_text: impl Into<SharedString>,
    body: impl Into<SharedString>,
    ui: &Ui,
    p: &Palette,
) -> Div {
    let t = &ui.theme;
    card(ui, p)
        .gap(px(t.spacing.sm))
        .py(px(t.spacing.xxl))
        .child(title(title_text, ui))
        .child(muted(body, ui, p))
}

/// A heading for a group of controls inside a card.
pub fn subhead(text: impl Into<SharedString>, ui: &Ui) -> Div {
    div()
        .text_size(px(ui.theme.text_size.md))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}
