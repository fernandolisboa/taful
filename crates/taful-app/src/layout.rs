//! Table-screen layouts (spec §15.3, D7).
//!
//! A layout places the table screen's panels on a resolution-independent
//! grid (48 × 27 cells). The UI turns cells into pixels from the space it
//! has and scales the table to fit its panel. Built-in presets live in
//! `content/layouts/`; user copies are stored in the `layouts` table.
//!
//! The schema is versioned: a file from an older version is migrated when it
//! is read, and a panel the file does not know yet gets its position from the
//! default preset.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Layout schema version this build reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

/// Grid size every layout uses.
pub const GRID_COLS: u16 = 48;
/// Grid rows.
pub const GRID_ROWS: u16 = 27;

/// Id of the default preset.
pub const DEFAULT_LAYOUT: &str = "default";

/// Every built-in preset as `(id, JSON)`, in the order settings lists them.
/// A test checks that this is exactly the set of files in `content/layouts/`.
pub const BUILTIN_LAYOUTS: &[(&str, &str)] = &[
    (
        DEFAULT_LAYOUT,
        include_str!("../../../content/layouts/default.json"),
    ),
    (
        "panel-left",
        include_str!("../../../content/layouts/panel-left.json"),
    ),
    (
        "wide-panel",
        include_str!("../../../content/layouts/wide-panel.json"),
    ),
];

/// A panel of the table screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelId {
    /// Mode and spot pickers.
    Controls,
    /// The felt.
    Table,
    /// Hero's action buttons.
    Actions,
    /// Grade, EV table, hand log and hand-over buttons.
    Explanation,
}

impl PanelId {
    /// Every panel, in drawing order.
    pub const ALL: [PanelId; 4] = [
        PanelId::Controls,
        PanelId::Table,
        PanelId::Actions,
        PanelId::Explanation,
    ];

    /// Id in layout files.
    pub fn id(self) -> &'static str {
        match self {
            PanelId::Controls => "controls",
            PanelId::Table => "table",
            PanelId::Actions => "actions",
            PanelId::Explanation => "explanation",
        }
    }

    pub fn parse(s: &str) -> Option<PanelId> {
        PanelId::ALL.into_iter().find(|p| p.id() == s)
    }

    /// Panels the table screen cannot work without.
    pub fn required(self) -> bool {
        !matches!(self, PanelId::Explanation)
    }
}

fn yes() -> bool {
    true
}

/// One panel's place on the grid, in cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Panel {
    pub id: String,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    #[serde(default = "yes")]
    pub visible: bool,
}

/// Grid size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
}

/// A complete layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub schema: u32,
    /// Preset id, or the user's name for a saved copy.
    pub name: String,
    pub grid: Grid,
    pub panels: Vec<Panel>,
}

/// A layout that cannot be used.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutError {
    /// Not valid JSON, or a field is missing, unknown or malformed.
    Parse(String),
    /// Written for a newer schema than this build reads.
    Schema(u32),
    /// Parsed, but breaks a rule (see [`Layout::problems`]).
    Invalid(Vec<String>),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::Parse(e) => write!(f, "invalid layout: {e}"),
            LayoutError::Schema(v) => write!(
                f,
                "layout schema {v} is not supported (expected {SCHEMA_VERSION})"
            ),
            LayoutError::Invalid(p) => write!(f, "invalid layout: {}", p.join("; ")),
        }
    }
}

impl std::error::Error for LayoutError {}

/// A panel's rectangle in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Layout {
    /// Parses, migrates and validates a layout file.
    pub fn from_json(json: &str) -> Result<Layout, LayoutError> {
        let v: serde_json::Value =
            serde_json::from_str(json).map_err(|e| LayoutError::Parse(e.to_string()))?;
        let schema = v
            .get("schema")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| LayoutError::Parse("missing `schema`".into()))?;
        let schema = u32::try_from(schema).unwrap_or(u32::MAX);
        if schema > SCHEMA_VERSION || schema == 0 {
            return Err(LayoutError::Schema(schema));
        }
        // Schema 1 is current: no migration step yet. A bump adds one here.
        let mut layout: Layout =
            serde_json::from_value(v).map_err(|e| LayoutError::Parse(e.to_string()))?;
        layout.schema = SCHEMA_VERSION;
        layout.add_missing_panels();
        let problems = layout.problems();
        if problems.is_empty() {
            Ok(layout)
        } else {
            Err(LayoutError::Invalid(problems))
        }
    }

    /// Serializes for storage.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("a layout serializes")
    }

    /// The built-in default preset.
    pub fn builtin_default() -> Layout {
        Layout::builtin(DEFAULT_LAYOUT).expect("the default preset exists")
    }

    /// A built-in preset by id.
    pub fn builtin(id: &str) -> Option<Layout> {
        BUILTIN_LAYOUTS
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, json)| Layout::from_json(json).expect("built-in layouts are valid (tested)"))
    }

    /// Panels a layout from an older build does not have get the default
    /// preset's position (spec §15.3), hidden if they would overlap.
    fn add_missing_panels(&mut self) {
        let missing: Vec<PanelId> = PanelId::ALL
            .into_iter()
            .filter(|p| !self.panels.iter().any(|q| q.id == p.id()))
            .collect();
        if missing.is_empty() {
            return;
        }
        let Some((_, json)) = BUILTIN_LAYOUTS.iter().find(|(i, _)| *i == DEFAULT_LAYOUT) else {
            return;
        };
        let Ok(default) = serde_json::from_str::<Layout>(json) else {
            return;
        };
        for p in missing {
            if let Some(mut d) = default.panels.iter().find(|q| q.id == p.id()).cloned() {
                if self.panels.iter().any(|q| q.visible && overlaps(q, &d)) {
                    d.visible = p.required();
                }
                self.panels.push(d);
            }
        }
    }

    /// Every rule this layout breaks: known and unique panel ids, every
    /// panel present, panels inside the grid and non-empty, required panels
    /// visible, visible panels not overlapping.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.grid.cols != GRID_COLS || self.grid.rows != GRID_ROWS {
            out.push(format!(
                "grid must be {GRID_COLS}×{GRID_ROWS}, is {}×{}",
                self.grid.cols, self.grid.rows
            ));
        }
        for (i, p) in self.panels.iter().enumerate() {
            match PanelId::parse(&p.id) {
                None => out.push(format!("unknown panel `{}`", p.id)),
                Some(id) => {
                    if id.required() && !p.visible {
                        out.push(format!("panel `{}` must be visible", p.id));
                    }
                }
            }
            if self.panels[..i].iter().any(|q| q.id == p.id) {
                out.push(format!("panel `{}` appears twice", p.id));
            }
            if p.w == 0 || p.h == 0 {
                out.push(format!("panel `{}` is empty", p.id));
            }
            if u32::from(p.x) + u32::from(p.w) > u32::from(self.grid.cols)
                || u32::from(p.y) + u32::from(p.h) > u32::from(self.grid.rows)
            {
                out.push(format!("panel `{}` leaves the grid", p.id));
            }
            for q in &self.panels[..i] {
                if p.visible && q.visible && overlaps(p, q) {
                    out.push(format!("panels `{}` and `{}` overlap", q.id, p.id));
                }
            }
        }
        for p in PanelId::ALL {
            if !self.panels.iter().any(|q| q.id == p.id()) {
                out.push(format!("panel `{}` is missing", p.id()));
            }
        }
        out
    }

    /// A panel by id, if it is visible.
    pub fn panel(&self, id: PanelId) -> Option<&Panel> {
        self.panels.iter().find(|p| p.id == id.id() && p.visible)
    }

    /// Size of one grid cell in pixels for an area of `w` × `h` pixels:
    /// the largest square cell that fits, but at least `min_cell`.
    pub fn cell_size(&self, w: f32, h: f32, min_cell: f32) -> f32 {
        let cols = f32::from(self.grid.cols.max(1));
        let rows = f32::from(self.grid.rows.max(1));
        (w / cols).min(h / rows).max(min_cell)
    }

    /// A visible panel's rectangle for a cell size.
    pub fn rect(&self, id: PanelId, cell: f32) -> Option<Rect> {
        self.panel(id).map(|p| Rect {
            x: f32::from(p.x) * cell,
            y: f32::from(p.y) * cell,
            w: f32::from(p.w) * cell,
            h: f32::from(p.h) * cell,
        })
    }
}

fn overlaps(a: &Panel, b: &Panel) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn builtin_list_matches_the_preset_files() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/layouts");
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        let mut listed: Vec<String> = BUILTIN_LAYOUTS.iter().map(|(i, _)| i.to_string()).collect();
        files.sort();
        listed.sort();
        assert_eq!(files, listed);
        for (id, json) in BUILTIN_LAYOUTS {
            let l = Layout::from_json(json).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(l.name, *id, "name must match the file");
            for p in PanelId::ALL {
                assert!(l.panel(p).is_some(), "{id}: {} hidden", p.id());
            }
        }
    }

    #[test]
    fn spec_example_shape_parses_and_gets_missing_panels() {
        // The spec's example only places two panels; the others come from
        // the default preset (as a schema migration would add them).
        let json = r#"{ "schema": 1, "name": "mine", "grid": { "cols": 48, "rows": 27 },
            "panels": [ { "id": "explanation", "x": 33, "y": 5, "w": 14, "h": 18, "visible": true },
                        { "id": "actions", "x": 0, "y": 22, "w": 20, "h": 4 } ] }"#;
        let l = Layout::from_json(json).unwrap();
        assert_eq!(l.panels.len(), 4);
        assert!(l.panel(PanelId::Table).is_some());
        assert!(l.panel(PanelId::Controls).is_some());
        assert_eq!(l.panel(PanelId::Explanation).unwrap().x, 33);
    }

    #[test]
    fn rules() {
        let base = Layout::builtin_default();
        let bad = |f: &dyn Fn(&mut Layout)| {
            let mut l = base.clone();
            f(&mut l);
            assert!(
                matches!(
                    Layout::from_json(&l.to_json()),
                    Err(LayoutError::Invalid(_))
                ),
                "{l:?}"
            );
        };
        bad(&|l| l.panels[1].x = 40); // overlaps the explanation, leaves the grid
        bad(&|l| l.panels[1].w = 0);
        bad(&|l| l.panels[1].visible = false); // the table is required
        bad(&|l| l.panels[0].id = "chat".into());
        bad(&|l| l.grid.cols = 24);
        bad(&|l| {
            let p = l.panels[0].clone();
            l.panels.push(p);
        });
        // The explanation may be hidden, and then may overlap.
        let mut l = base.clone();
        l.panels[3].visible = false;
        l.panels[3].x = 0;
        assert!(Layout::from_json(&l.to_json()).is_ok());

        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["schema"] = (SCHEMA_VERSION + 1).into();
        assert!(matches!(
            Layout::from_json(&v.to_string()),
            Err(LayoutError::Schema(_))
        ));
        assert!(matches!(
            Layout::from_json("{}"),
            Err(LayoutError::Parse(_))
        ));
    }

    #[test]
    fn round_trip_and_rects() {
        let l = Layout::builtin("wide-panel").unwrap();
        assert_eq!(Layout::from_json(&l.to_json()).unwrap(), l);
        // 960 × 540 px → 20 px cells.
        let cell = l.cell_size(960.0, 600.0, 12.0);
        assert_eq!(cell, 20.0);
        let r = l.rect(PanelId::Explanation, cell).unwrap();
        assert_eq!((r.x, r.y, r.w, r.h), (520.0, 100.0, 440.0, 440.0));
        assert_eq!(l.cell_size(100.0, 100.0, 12.0), 12.0);
    }
}
