//! Layout edit mode (spec §15.3, D7, #22).
//!
//! The table screen's edit mode drags panels by their header and resizes them
//! from a corner. Every move or resize goes through a [`LayoutEditor`], which
//! applies the same rules a layout file is checked against on load (known
//! unique ids, inside the grid, non-empty, no overlap among visible panels)
//! and refuses a drop that breaks one, with the reason.
//!
//! - A move snaps the panel's corner to the nearest whole cell, or, with
//!   `snap: false` (Alt held), keeps it where it was let go, rounded to
//!   [`crate::layout::FREE_STEP`].
//! - A resize always snaps to whole cells.
//! - Nothing is stored while editing. [`App::save_edited_layout`] stores the
//!   draft as a user copy (built-in presets stay read-only) after the same
//!   parse-and-migrate step a saved copy goes through on load;
//!   [`LayoutEditor::cancel`] hands back the layout from before editing.

use crate::layout::{Layout, PanelId, Problem, quantize};

/// Why a move or resize was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Zero width or height.
    Empty,
    /// Part of the panel would be outside the grid.
    LeavesGrid,
    /// The panel would cover this visible panel.
    Overlaps(PanelId),
    /// The panel is not in the layout, or is hidden.
    NoSuchPanel,
    /// Any other rule (not reachable by moving or resizing a valid layout).
    Invalid(Problem),
}

/// Where a panel would land, and whether it may.
#[derive(Debug, Clone, PartialEq)]
pub struct Landing {
    pub id: PanelId,
    /// Position and size in cells.
    pub x: f32,
    pub y: f32,
    pub w: u16,
    pub h: u16,
    /// `None` when the drop is allowed.
    pub refused: Option<Refusal>,
}

impl Landing {
    pub fn allowed(&self) -> bool {
        self.refused.is_none()
    }
}

/// An edit session over a layout.
#[derive(Debug, Clone)]
pub struct LayoutEditor {
    before: Layout,
    draft: Layout,
}

impl LayoutEditor {
    /// Starts editing `layout` (a valid layout, as [`App::active_layout`]
    /// returns).
    pub fn new(layout: Layout) -> LayoutEditor {
        LayoutEditor {
            before: layout.clone(),
            draft: layout,
        }
    }

    /// The layout as edited so far (always valid).
    pub fn draft(&self) -> &Layout {
        &self.draft
    }

    /// The layout from before editing.
    pub fn before(&self) -> &Layout {
        &self.before
    }

    /// Whether any move or resize was accepted.
    pub fn changed(&self) -> bool {
        self.draft != self.before
    }

    /// Ends the session without saving: the layout from before editing.
    pub fn cancel(self) -> Layout {
        self.before
    }

    /// Where `id` would land if its top-left corner were dropped at
    /// (`x`, `y`) cells: the nearest whole cell when `snap`, else the nearest
    /// [`crate::layout::FREE_STEP`].
    pub fn landing_for_move(&self, id: PanelId, x: f32, y: f32, snap: bool) -> Landing {
        let (x, y) = if snap {
            (x.round(), y.round())
        } else {
            (quantize(x), quantize(y))
        };
        match self.draft.panel(id) {
            Some(p) => self.check(id, x, y, p.w, p.h),
            None => self.missing(id),
        }
    }

    /// Where `id` would land if resized to `w` × `h` cells (rounded to whole
    /// cells), keeping its top-left corner. A size that reaches the grid's
    /// edge but rounds past it (a panel at a fraction of a cell) is cut to
    /// the whole cells that fit.
    pub fn landing_for_resize(&self, id: PanelId, w: f32, h: f32) -> Landing {
        let cells = |v: f32, at: f32, room: u16| {
            if !v.is_finite() {
                return 0;
            }
            let room = f32::from(room) - at;
            let mut n = v.round().clamp(0., f32::from(u16::MAX));
            if v <= room && n > room {
                n = room.floor().max(0.);
            }
            n as u16
        };
        let grid = self.draft.grid;
        match self.draft.panel(id) {
            Some(p) => self.check(
                id,
                p.x,
                p.y,
                cells(w, p.x, grid.cols),
                cells(h, p.y, grid.rows),
            ),
            None => self.missing(id),
        }
    }

    /// Accepts a landing from [`Self::landing_for_move`] or
    /// [`Self::landing_for_resize`] if it is allowed.
    pub fn apply(&mut self, landing: &Landing) -> Result<(), Refusal> {
        // Checked again: the draft may have changed since the landing.
        let again = self.check(landing.id, landing.x, landing.y, landing.w, landing.h);
        if let Some(r) = again.refused {
            return Err(r);
        }
        let p = self
            .draft
            .panels
            .iter_mut()
            .find(|p| p.id == landing.id.id())
            .ok_or(Refusal::NoSuchPanel)?;
        (p.x, p.y, p.w, p.h) = (again.x, again.y, again.w, again.h);
        Ok(())
    }

    /// Moves a panel (see [`Self::landing_for_move`]).
    pub fn move_panel(&mut self, id: PanelId, x: f32, y: f32, snap: bool) -> Result<(), Refusal> {
        let l = self.landing_for_move(id, x, y, snap);
        self.apply(&l)
    }

    /// Resizes a panel (see [`Self::landing_for_resize`]).
    pub fn resize_panel(&mut self, id: PanelId, w: f32, h: f32) -> Result<(), Refusal> {
        let l = self.landing_for_resize(id, w, h);
        self.apply(&l)
    }

    fn missing(&self, id: PanelId) -> Landing {
        Landing {
            id,
            x: 0.,
            y: 0.,
            w: 0,
            h: 0,
            refused: Some(Refusal::NoSuchPanel),
        }
    }

    /// The draft with `id` placed at the given cells, checked against the
    /// load-time rules. The draft is valid, so any rule the candidate breaks
    /// comes from this panel.
    fn check(&self, id: PanelId, x: f32, y: f32, w: u16, h: u16) -> Landing {
        let (x, y) = (quantize(x), quantize(y));
        let mut candidate = self.draft.clone();
        let refused = match candidate.panels.iter_mut().find(|p| p.id == id.id()) {
            None => Some(Refusal::NoSuchPanel),
            Some(p) if !p.visible => Some(Refusal::NoSuchPanel),
            Some(p) => {
                (p.x, p.y, p.w, p.h) = (x, y, w, h);
                refusal(id, candidate.violations())
            }
        };
        Landing {
            id,
            x,
            y,
            w,
            h,
            refused,
        }
    }
}

/// The most useful reason among the rules a candidate breaks.
fn refusal(id: PanelId, problems: Vec<Problem>) -> Option<Refusal> {
    let mine = |p: &str| p == id.id();
    let mut best: Option<(u8, Refusal)> = None;
    for problem in problems {
        let r = match &problem {
            Problem::Empty(p) if mine(p) => (0, Refusal::Empty),
            Problem::LeavesGrid(p) if mine(p) => (1, Refusal::LeavesGrid),
            Problem::Overlap(a, b) if mine(a) || mine(b) => {
                let other = if mine(a) { b } else { a };
                match PanelId::parse(other) {
                    Some(o) => (2, Refusal::Overlaps(o)),
                    None => (3, Refusal::Invalid(problem.clone())),
                }
            }
            _ => (3, Refusal::Invalid(problem.clone())),
        };
        if best.as_ref().is_none_or(|(rank, _)| r.0 < *rank) {
            best = Some(r);
        }
    }
    best.map(|(_, r)| r)
}

#[cfg(test)]
mod tests {
    use crate::App;
    use crate::appearance::builtin_layout_key;
    use crate::layout::{BUILTIN_LAYOUTS, Layout, PanelId};
    use crate::services::tests::app_at;

    use super::*;

    /// Default preset: controls 0,0 48×5; table 0,5 31×17; actions 0,22
    /// 31×5; explanation 31,5 17×22.
    fn editor() -> (App, LayoutEditor) {
        let (app, _) = app_at(1);
        let e = app.edit_layout();
        (app, e)
    }

    #[test]
    fn moves_snap_on_release_and_alt_drops_free() {
        let (_, mut e) = editor();
        // Shrink the table to make room, then move it around.
        e.resize_panel(PanelId::Table, 20., 17.).unwrap();
        let l = e.landing_for_move(PanelId::Table, 6.4, 4.6, true);
        assert_eq!((l.x, l.y), (6., 5.));
        assert!(l.allowed());
        e.apply(&l).unwrap();
        let p = e.draft().panel(PanelId::Table).unwrap();
        assert_eq!((p.x, p.y, p.w, p.h), (6., 5., 20, 17));

        // Alt: kept where let go, to an eighth of a cell.
        e.move_panel(PanelId::Table, 3.3, 5.06, false).unwrap();
        let p = e.draft().panel(PanelId::Table).unwrap();
        assert_eq!((p.x, p.y), (3.25, 5.0));
        assert!(e.changed());
    }

    #[test]
    fn invalid_drops_are_refused_with_a_reason() {
        let (_, mut e) = editor();
        let before = e.draft().clone();
        // Onto the explanation panel.
        assert_eq!(
            e.move_panel(PanelId::Actions, 10., 22., true),
            Err(Refusal::Overlaps(PanelId::Explanation))
        );
        // A free drop that clips the table by an eighth of a cell.
        assert_eq!(
            e.move_panel(PanelId::Actions, 0., 21.875, false),
            Err(Refusal::Overlaps(PanelId::Table))
        );
        // Out of the grid.
        assert_eq!(
            e.move_panel(PanelId::Explanation, 32., 5., true),
            Err(Refusal::LeavesGrid)
        );
        assert_eq!(
            e.move_panel(PanelId::Controls, -1., 0., true),
            Err(Refusal::LeavesGrid)
        );
        // Resizes: wider than the room left, empty, overlapping.
        assert_eq!(
            e.resize_panel(PanelId::Explanation, 18., 22.),
            Err(Refusal::LeavesGrid)
        );
        assert_eq!(
            e.resize_panel(PanelId::Controls, 0.3, 5.),
            Err(Refusal::Empty)
        );
        assert_eq!(
            e.resize_panel(PanelId::Table, 31., 18.),
            Err(Refusal::Overlaps(PanelId::Actions))
        );
        // The landing preview says the same before the drop.
        let l = e.landing_for_move(PanelId::Table, 10., 5., true);
        assert_eq!(l.refused, Some(Refusal::Overlaps(PanelId::Explanation)));
        assert_eq!(e.draft(), &before, "refused edits change nothing");
        assert!(!e.changed());
    }

    #[test]
    fn resizing_to_the_edge_from_a_fraction_of_a_cell() {
        let (_, mut e) = editor();
        e.resize_panel(PanelId::Explanation, 16., 22.).unwrap();
        e.move_panel(PanelId::Explanation, 31.5, 5., false).unwrap();
        // Dragged to the right edge: 16.5 cells of room round to 17, which
        // would leave the grid, so 16 fit.
        let l = e.landing_for_resize(PanelId::Explanation, 16.5, 22.);
        assert_eq!((l.w, l.refused), (16, None));
        // Past the edge is still refused.
        assert_eq!(
            e.resize_panel(PanelId::Explanation, 17., 22.),
            Err(Refusal::LeavesGrid)
        );
        // A hand-built landing is kept on the position step.
        let mut l = e.landing_for_move(PanelId::Explanation, 31.5, 5., false);
        l.x = 31.56;
        e.apply(&l).unwrap();
        assert_eq!(e.draft().panel(PanelId::Explanation).unwrap().x, 31.5);
    }

    #[test]
    fn hidden_panels_are_not_edited() {
        let mut l = Layout::builtin_default();
        l.panels[3].visible = false;
        let mut e = LayoutEditor::new(l);
        assert_eq!(
            e.move_panel(PanelId::Explanation, 0., 0., true),
            Err(Refusal::NoSuchPanel)
        );
        // With the explanation hidden the table may take its room.
        e.resize_panel(PanelId::Table, 48., 17.).unwrap();
    }

    /// The issue's demo: move the explanation panel to the left, widen it,
    /// save as a copy, restart, the copy is still there.
    #[test]
    fn demo_save_as_copy_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path()).unwrap();
        let mut e = app.edit_layout();
        // The table fills the left, so the explanation can't go there yet.
        assert_eq!(
            e.move_panel(PanelId::Explanation, 0., 5., true),
            Err(Refusal::Overlaps(PanelId::Table))
        );
        // Park the explanation as one column on the right edge, move the
        // table and actions next to it, then bring it to the left and widen.
        e.resize_panel(PanelId::Explanation, 0.6, 22.).unwrap(); // 1 cell
        e.move_panel(PanelId::Explanation, 47., 5., true).unwrap();
        e.move_panel(PanelId::Table, 16.2, 5., true).unwrap();
        e.move_panel(PanelId::Actions, 15.8, 22., true).unwrap();
        e.move_panel(PanelId::Explanation, 0., 5., true).unwrap();
        e.resize_panel(PanelId::Explanation, 15.6, 22.).unwrap(); // 16 cells
        assert_eq!(
            e.resize_panel(PanelId::Explanation, 17., 22.),
            Err(Refusal::Overlaps(PanelId::Table))
        );
        let draft = e.draft().clone();
        let (saved, stored) = app.save_edited_layout(&draft, "Mine").unwrap();
        assert_eq!(stored.panels, draft.panels);
        assert!(!saved.builtin);
        assert_eq!(app.active_layout().0, saved.key);
        drop(app);

        let app = App::open(dir.path()).unwrap();
        let (key, l) = app.active_layout();
        assert_eq!(key, saved.key, "still active after a restart");
        assert_eq!(l.name, "Mine");
        assert_eq!(l.panels, draft.panels, "reloads identically");
        let p = l.panel(PanelId::Explanation).unwrap();
        assert_eq!((p.x, p.y, p.w, p.h), (0., 5., 16, 22));
        // Selectable like any saved copy.
        let choices = app.layout_choices().unwrap();
        assert!(
            choices
                .iter()
                .any(|c| c.key == saved.key && c.name == "Mine")
        );
        app.set_layout(&builtin_layout_key("wide-panel")).unwrap();
        assert_eq!(app.set_layout(&saved.key).unwrap().panels, draft.panels);
        // The built-in preset it started from is untouched.
        assert_eq!(
            app.layout_choices().unwrap().len(),
            BUILTIN_LAYOUTS.len() + 1
        );
        assert_eq!(
            Layout::builtin("default").unwrap().panels,
            e.before().panels
        );
    }

    #[test]
    fn free_positions_round_trip() {
        let (app, _) = app_at(1);
        let mut e = app.edit_layout();
        e.resize_panel(PanelId::Explanation, 16., 21.).unwrap();
        e.move_panel(PanelId::Explanation, 31.625, 5.125, false)
            .unwrap();
        let (saved, _) = app.save_edited_layout(e.draft(), "Free").unwrap();
        let l = app.set_layout(&saved.key).unwrap();
        let p = l.panel(PanelId::Explanation).unwrap();
        assert_eq!((p.x, p.y), (31.625, 5.125));
    }

    #[test]
    fn cancel_restores_the_layout_from_before() {
        let (app, _) = app_at(1);
        app.set_layout(&builtin_layout_key("panel-left")).unwrap();
        let (key, before) = app.active_layout();
        let mut e = app.edit_layout();
        e.resize_panel(PanelId::Controls, 10., 5.).unwrap();
        assert!(e.changed());
        assert_eq!(e.cancel(), before);
        assert_eq!(app.active_layout(), (key, before), "nothing stored");
        assert_eq!(app.layout_choices().unwrap().len(), BUILTIN_LAYOUTS.len());
    }

    #[test]
    fn saving_migrates_and_validates() {
        let (app, _) = app_at(1);
        // A schema-1 copy from an older build missing a panel: editing it
        // starts from the migrated layout, and the save is schema 2.
        let old = r#"{ "schema": 1, "name": "old", "grid": { "cols": 48, "rows": 27 },
            "panels": [ { "id": "controls", "x": 0, "y": 0, "w": 48, "h": 5 },
                        { "id": "table", "x": 0, "y": 5, "w": 31, "h": 17 },
                        { "id": "actions", "x": 0, "y": 22, "w": 31, "h": 5 } ] }"#;
        let id = app.store.insert_layout("old", 1, old).unwrap();
        app.set_layout(&format!("user:{id}")).unwrap();
        let mut e = app.edit_layout();
        assert!(
            e.draft().panel(PanelId::Explanation).is_some(),
            "the panel added later gets its default position"
        );
        e.resize_panel(PanelId::Explanation, 16., 22.).unwrap();
        let (saved, _) = app.save_edited_layout(e.draft(), "old 2").unwrap();
        let rows = app.store.layouts().unwrap();
        let row = rows.iter().find(|r| saved.key.ends_with(&r.id)).unwrap();
        assert_eq!(row.schema_version, crate::layout::SCHEMA_VERSION);

        // A broken draft (not from an editor) is refused, nothing stored.
        let mut bad = e.draft().clone();
        bad.panels[0].w = 0;
        assert!(app.save_edited_layout(&bad, "bad").is_err());
        assert!(app.save_edited_layout(e.draft(), "  ").is_err());
        assert_eq!(app.store.layouts().unwrap().len(), 2);
        assert_eq!(app.active_layout().0, saved.key);
    }
}
