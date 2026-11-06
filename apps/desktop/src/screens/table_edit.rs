//! Layout edit mode on the table screen (spec §15.3, D7, #22).
//!
//! Panels are dragged by a header strip and resized from their bottom-right
//! corner. A dragged panel follows the cursor freely; a dashed outline shows
//! where it will land, and it snaps to the grid only on release (hold Alt to
//! drop it where it is). The dot grid is painted in one canvas pass and only
//! while editing. Every drop goes through [`LayoutEditor`], which refuses a
//! drop that breaks a layout rule; the reason shows in the edit bar.

use gpui::{
    AnyElement, Bounds, Context, CursorStyle, DispatchPhase, Div, Hsla, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, canvas, div, fill, point,
    prelude::*, px, size,
};
use taful_app::layout::{Layout, PanelId, Rect};
use taful_app::layout_edit::{Landing, LayoutEditor, Refusal};

use super::{TableEvent, TableScreen};
use crate::Ui;

/// Height of the edit bar above the grid, in pixels.
pub(super) const EDIT_BAR: f32 = 64.;
/// Height of a panel's drag header, in pixels.
const HEADER: f32 = 26.;
/// Side of the resize handle, in pixels.
const HANDLE: f32 = 14.;
/// Smallest size a resize drags to, in cells (the editor refuses 0).
const MIN_CELLS: f32 = 2.;

/// An edit session.
pub(super) struct Editing {
    editor: LayoutEditor,
    drag: Option<Drag>,
    /// Why the last drop was refused.
    refused: Option<Refusal>,
    /// Saving failed (shown in the edit bar).
    error: Option<String>,
}

/// A panel being moved or resized.
#[derive(Clone, Copy)]
struct Drag {
    id: PanelId,
    resize: bool,
    /// Mouse position when the drag started, and now.
    start: Point<Pixels>,
    now: Point<Pixels>,
    /// Alt held (drop without snapping).
    alt: bool,
    /// The panel's position and size in cells when the drag started.
    from: (f32, f32, f32, f32),
}

impl TableScreen {
    /// Enters edit mode on the layout on screen.
    pub(super) fn start_edit(&mut self, cx: &mut Context<Self>) {
        self.editing = Some(Editing {
            editor: LayoutEditor::new(self.layout.clone()),
            drag: None,
            refused: None,
            error: None,
        });
        cx.notify();
    }

    /// Leaves edit mode and restores the layout from before editing.
    pub(super) fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(e) = self.editing.take() {
            self.layout = e.editor.cancel();
        }
        cx.notify();
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(e) = &self.editing {
            cx.emit(TableEvent::SaveLayout(Box::new(e.editor.draft().clone())));
        }
    }

    /// The shell stored the edited layout (`Ok` is the saved copy, now
    /// active) or could not (the reason, localized).
    pub fn layout_saved(&mut self, r: Result<Layout, String>, cx: &mut Context<Self>) {
        match r {
            Ok(layout) => {
                self.layout = layout;
                self.editing = None;
            }
            Err(e) => {
                if let Some(ed) = &mut self.editing {
                    ed.error = Some(e);
                }
            }
        }
        cx.notify();
    }

    pub(super) fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// The layout to draw: the draft while editing.
    pub(super) fn shown_layout(&self) -> Layout {
        match &self.editing {
            Some(e) => e.editor.draft().clone(),
            None => self.layout.clone(),
        }
    }

    /// Where a panel is drawn: under the cursor while dragged.
    pub(super) fn panel_rect(&self, layout: &Layout, id: PanelId, cell: f32) -> Option<Rect> {
        let r = layout.rect(id, cell)?;
        match self.editing.as_ref().and_then(|e| e.drag) {
            Some(d) if d.id == id => {
                let (x, y, w, h) = self.drag_cells(&d, layout);
                Some(Rect {
                    x: x * cell,
                    y: y * cell,
                    w: w * cell,
                    h: h * cell,
                })
            }
            _ => Some(r),
        }
    }

    /// The dragged panel's rectangle in cells, kept inside the grid.
    fn drag_cells(&self, d: &Drag, layout: &Layout) -> (f32, f32, f32, f32) {
        let cell = self.cell.max(1.);
        let dx = f32::from(d.now.x - d.start.x) / cell;
        let dy = f32::from(d.now.y - d.start.y) / cell;
        let (cols, rows) = (f32::from(layout.grid.cols), f32::from(layout.grid.rows));
        let (x, y, w, h) = d.from;
        if d.resize {
            let w = (w + dx).clamp(MIN_CELLS.min(w), (cols - x).max(1.));
            let h = (h + dy).clamp(MIN_CELLS.min(h), (rows - y).max(1.));
            (x, y, w, h)
        } else {
            let x = (x + dx).clamp(0., (cols - w).max(0.));
            let y = (y + dy).clamp(0., (rows - h).max(0.));
            (x, y, w, h)
        }
    }

    /// Where the dragged panel would land if dropped now.
    fn landing(&self, e: &Editing, d: &Drag) -> Landing {
        let (x, y, w, h) = self.drag_cells(d, e.editor.draft());
        if d.resize {
            e.editor.landing_for_resize(d.id, w, h)
        } else {
            e.editor.landing_for_move(d.id, x, y, !d.alt)
        }
    }

    fn begin_drag(
        &mut self,
        id: PanelId,
        resize: bool,
        ev: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = &mut self.editing else { return };
        let Some(p) = e.editor.draft().panel(id) else {
            return;
        };
        e.drag = Some(Drag {
            id,
            resize,
            start: ev.position,
            now: ev.position,
            alt: ev.modifiers.alt,
            from: (p.x, p.y, f32::from(p.w), f32::from(p.h)),
        });
        e.refused = None;
        e.error = None;
        cx.stop_propagation();
        cx.notify();
    }

    fn on_drag_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        if let Some(d) = self.editing.as_mut().and_then(|e| e.drag.as_mut()) {
            d.now = ev.position;
            d.alt = ev.modifiers.alt;
            cx.notify();
        }
    }

    fn on_drop(&mut self, ev: &MouseUpEvent, cx: &mut Context<Self>) {
        let Some(mut d) = self.editing.as_mut().and_then(|e| e.drag.take()) else {
            return;
        };
        d.alt = ev.modifiers.alt;
        let landing = match &self.editing {
            Some(e) => self.landing(e, &d),
            None => return,
        };
        if let Some(e) = &mut self.editing {
            e.refused = e.editor.apply(&landing).err();
        }
        cx.notify();
    }

    /// The bar above the grid: hint or refusal, save and cancel.
    pub(super) fn edit_bar(&self, width: f32, cx: &mut Context<Self>) -> Div {
        let (p, ui) = (self.palette, self.ui.clone());
        let th = &ui.theme;
        let Some(e) = &self.editing else {
            return div();
        };
        let live = e.drag.map(|d| self.landing(e, &d)).and_then(|l| l.refused);
        let (text, warn) = match (&e.error, live.as_ref().or(e.refused.as_ref())) {
            (Some(err), _) => (err.clone(), true),
            (None, Some(r)) => (refusal_text(&ui, r), true),
            (None, None) => (ui.i18n.tr("layout-edit-hint"), false),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(th.spacing.md))
            .w(px(width))
            .h(px(EDIT_BAR))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(th.text_size.md))
                            .text_color(p.text)
                            .child(ui.i18n.tr("layout-edit-title")),
                    )
                    .child(
                        div()
                            .text_size(px(th.text_size.sm))
                            .text_color(if warn { p.grade_mistake } else { p.text_muted })
                            .child(text),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(th.spacing.sm))
                    .child(self.button(
                        "layout_edit_cancel",
                        ui.i18n.tr("layout-edit-cancel"),
                        false,
                        cx,
                        |this, cx| this.cancel_edit(cx),
                    ))
                    .child(self.button(
                        "layout_edit_save",
                        ui.i18n.tr("layout-edit-save"),
                        true,
                        cx,
                        |this, cx| this.save_edit(cx),
                    )),
            )
    }

    /// While editing: the dot grid under the panels and, during a drag,
    /// window-wide mouse listeners that move the dragged panel (the panels'
    /// covers take the hover, and the cursor may leave the grid).
    pub(super) fn edit_grid(
        &self,
        grid: Div,
        layout: &Layout,
        cell: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(e) = &self.editing else { return grid };
        let dots = dot_grid(
            layout.grid.cols,
            layout.grid.rows,
            cell,
            self.palette.border.into(),
        );
        let grid = grid.child(dots);
        if e.drag.is_none() {
            return grid;
        }
        let this = cx.entity().downgrade();
        let listeners = canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let moved = this.clone();
                window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        moved.update(cx, |s, cx| s.on_drag_move(ev, cx)).ok();
                    }
                });
                let released = this.clone();
                window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble && ev.button == MouseButton::Left {
                        released.update(cx, |s, cx| s.on_drop(ev, cx)).ok();
                    }
                });
            },
        )
        .absolute()
        .size_0();
        grid.child(listeners)
    }

    /// The dashed outline of where the dragged panel will land, drawn over
    /// the panels; the refusal color when it may not land there.
    pub(super) fn landing_outline(&self, cell: f32) -> Option<Div> {
        let e = self.editing.as_ref()?;
        let d = e.drag?;
        let (p, th) = (self.palette, &self.ui.theme);
        let l = self.landing(e, &d);
        let color = if l.allowed() {
            p.accent
        } else {
            p.grade_mistake
        };
        Some(
            div()
                .absolute()
                .left(px(l.x * cell))
                .top(px(l.y * cell))
                .w(px(f32::from(l.w) * cell))
                .h(px(f32::from(l.h) * cell))
                .rounded(px(th.radii.md))
                .border_2()
                .border_dashed()
                .border_color(color),
        )
    }

    /// The panel being dragged, drawn above the others.
    pub(super) fn dragged_panel(&self) -> Option<PanelId> {
        self.editing.as_ref()?.drag.map(|d| d.id)
    }

    /// While editing, a panel's content is covered (no clicks reach it) and
    /// gets a drag header and a resize handle.
    pub(super) fn edit_chrome(
        &self,
        id: PanelId,
        panel: Div,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(e) = &self.editing else {
            return panel.into_any_element();
        };
        let (p, ui) = (self.palette, self.ui.clone());
        let th = &ui.theme;
        let dragged = e.drag.is_some_and(|d| d.id == id);
        let i = PanelId::ALL.iter().position(|q| *q == id).unwrap_or(0);
        panel
            .child(
                div()
                    .id(("layout_edit_cover", i))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .rounded(px(th.radii.md))
                    .border_2()
                    .border_color(if dragged { p.accent } else { p.border })
                    .occlude(),
            )
            .child(
                div()
                    .id(("layout_edit_header", i))
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(px(HEADER))
                    .flex()
                    .items_center()
                    .px(px(th.spacing.sm))
                    .rounded_t(px(th.radii.md))
                    .bg(if dragged { p.accent } else { p.surface_raised })
                    .text_size(px(th.text_size.sm))
                    .text_color(if dragged { p.on_accent } else { p.text })
                    .cursor(if dragged {
                        CursorStyle::ClosedHand
                    } else {
                        CursorStyle::OpenHand
                    })
                    .child(ui.i18n.tr(&format!("layout-panel-{}", id.id())))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            this.begin_drag(id, false, ev, cx)
                        }),
                    ),
            )
            .child(
                div()
                    .id(("layout_edit_resize", i))
                    .absolute()
                    .right_0()
                    .bottom_0()
                    .w(px(HANDLE))
                    .h(px(HANDLE))
                    .rounded(px(th.radii.sm))
                    .bg(p.accent)
                    .cursor(CursorStyle::ResizeUpLeftDownRight)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            this.begin_drag(id, true, ev, cx)
                        }),
                    ),
            )
            .into_any_element()
    }
}

/// One dot per grid corner, painted in a single canvas pass.
fn dot_grid(cols: u16, rows: u16, cell: f32, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds: Bounds<Pixels>, _, window, _| {
            const DOT: f32 = 2.;
            for i in 0..=cols {
                for j in 0..=rows {
                    let c = point(
                        bounds.origin.x + px(f32::from(i) * cell - DOT / 2.),
                        bounds.origin.y + px(f32::from(j) * cell - DOT / 2.),
                    );
                    window.paint_quad(fill(Bounds::new(c, size(px(DOT), px(DOT))), color));
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// A refused drop, in words.
fn refusal_text(ui: &Ui, r: &Refusal) -> String {
    let i18n = &ui.i18n;
    match r {
        Refusal::Overlaps(other) => i18n.tr_args(
            "layout-edit-refused-overlap",
            &[(
                "panel",
                i18n.tr(&format!("layout-panel-{}", other.id())).into(),
            )],
        ),
        Refusal::LeavesGrid => i18n.tr("layout-edit-refused-grid"),
        Refusal::Empty => i18n.tr("layout-edit-refused-empty"),
        Refusal::NoSuchPanel | Refusal::Invalid(_) => i18n.tr("layout-edit-refused-invalid"),
    }
}
