//! Cockpit's colors: the style file's standard tokens under the names the
//! design uses, plus its extra ones (`content/styles/cockpit.json`).

use taful_app::model::Grade;

use crate::Ui;
use crate::theme::{Palette, alpha};

/// A color (GPUI's representation of a token).
pub type Tint = gpui::Rgba;

/// Every color a Cockpit view draws with.
#[derive(Clone, Copy)]
pub struct Pal {
    /// Panel background.
    pub panel: Tint,
    /// Controls drawn on a panel (key caps, icon tiles, the active rail item).
    pub panel2: Tint,
    /// Buttons.
    pub raised: Tint,
    /// Hairlines between and inside panels.
    pub line: Tint,
    /// Outlines of controls.
    pub line2: Tint,
    pub text: Tint,
    pub muted: Tint,
    /// Tertiary text: axis labels, indexes, locked items.
    pub dim: Tint,
    pub amber: Tint,
    pub on_amber: Tint,
    /// Done (lesson progress).
    pub teal: Tint,
    pub rail_bg: Tint,
    /// The knot of the mark at the top of the rail.
    pub logo_knot: Tint,
    pub bar_bg: Tint,
    /// The lit top edge of a panel header.
    pub head_top: Tint,
    /// Key cap on an amber button.
    pub kbd_on_amber: Tint,
    pub kbd_on_amber_line: Tint,
    /// Empty progress segments.
    pub track: Tint,
    /// Forecast bars of the days after today.
    pub forecast: Tint,
    /// Backdrop dots.
    pub dot: Tint,
    /// Heat steps 1–4 (4 is amber).
    pub heat: [Tint; 4],
    /// Text on heat step 1 (the others take `cell_text` or `on_amber`).
    pub heat1_text: Tint,
    /// A range-matrix cell and its text.
    pub cell: Tint,
    pub cell_text: Tint,
    /// A cell with nothing to show.
    pub cell_out_text: Tint,
    /// Title of a locked lesson.
    pub locked_text: Tint,
    /// Grade colors, best first.
    pub grades: [Tint; 5],
}

impl Pal {
    pub fn new(ui: &Ui) -> Self {
        let p = Palette::new(&ui.theme.colors);
        let x = |name: &str| ui.extra.get(name);
        Self {
            panel: p.surface,
            panel2: x("panel2"),
            raised: p.surface_raised,
            line: x("line"),
            line2: p.border,
            text: p.text,
            muted: p.text_muted,
            dim: x("dim"),
            amber: p.accent,
            on_amber: p.on_accent,
            teal: x("teal"),
            rail_bg: x("rail_bg"),
            logo_knot: x("logo_knot"),
            bar_bg: x("bar_bg"),
            head_top: x("head_top"),
            kbd_on_amber: x("kbd_on_amber"),
            kbd_on_amber_line: x("kbd_on_amber_line"),
            track: x("track"),
            forecast: x("forecast"),
            dot: x("dot"),
            heat: [x("heat1"), x("heat2"), x("heat3"), p.accent],
            heat1_text: x("heat1_text"),
            cell: x("cell"),
            cell_text: x("cell_text"),
            cell_out_text: x("cell_out_text"),
            locked_text: x("locked_text"),
            grades: [
                p.grade_best,
                p.grade_correct,
                p.grade_inaccuracy,
                p.grade_mistake,
                p.grade_blunder,
            ],
        }
    }

    pub fn grade(&self, g: Grade) -> Tint {
        self.grades[Grade::ALL.iter().position(|x| *x == g).unwrap_or(0)]
    }

    /// Amber at `a` opacity (tints behind the next lesson).
    pub fn amber_at(&self, a: f32) -> Tint {
        alpha(self.amber, a)
    }
}

#[cfg(test)]
mod tests {
    use taful_app::i18n::Locale;
    use taful_app::style::Style;
    use taful_app::theme::Theme;

    use super::*;

    /// Every name `Pal::new` asks for is in the style file (a missing one
    /// panics in debug builds), and the style's own theme breaks no rule.
    #[test]
    fn the_style_file_has_every_color() {
        let ui = Ui::new(Locale::SOURCE, Theme::builtin_default(), Style::Cockpit);
        let _ = Pal::new(&ui);
        assert_eq!(ui.theme.warnings(), Vec::new());
    }
}
