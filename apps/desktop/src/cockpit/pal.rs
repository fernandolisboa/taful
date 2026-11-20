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
    /// Amber outlines (hero's actions on the hand line).
    pub amber_dim: Tint,
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
            amber_dim: p.accent_muted,
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

/// The colors only the table screen draws with.
#[derive(Clone, Copy)]
pub struct TablePal {
    /// Actions: bets and raises, checks and calls, folds.
    pub raise: Tint,
    pub call: Tint,
    pub fold: Tint,
    /// Card tiles by suit index (clubs, diamonds, hearts, spades) and
    /// their ink.
    pub tiles: [Tint; 4],
    pub tile_text: Tint,
    pub tile_diamonds_text: Tint,
    pub tile_edge: Tint,
    /// A face-down card: its two stripes and its edge.
    pub back: [Tint; 2],
    pub back_edge: Tint,
    /// The table: its rim, the felt from top to bottom and its lines.
    pub oval_rim: Tint,
    pub felt: [Tint; 2],
    pub felt_line: Tint,
    pub felt_dash: Tint,
    /// A seat, a folded seat and the empty part of a stack bar.
    pub pod: Tint,
    pub pod_fold: Tint,
    pub pod_bar: Tint,
    /// A bet on the felt.
    pub bet_bg: Tint,
    pub bet_edge: Tint,
    pub bet_shade: Tint,
    pub pot_label: Tint,
    pub pot_unit: Tint,
    /// An empty board slot.
    pub slot: Tint,
    pub dealer: Tint,
    pub dealer_text: Tint,
    /// The strip under the felt.
    pub actbar: Tint,
    /// Text on a grade tag.
    pub on_grade: Tint,
    pub shadow: Tint,
    /// The outline of hero's hand in a range matrix.
    pub hero_outline: Tint,
    /// A cell outside the range.
    pub cell_out: Tint,
}

impl TablePal {
    pub fn new(ui: &Ui) -> Self {
        let x = |name: &str| ui.extra.get(name);
        Self {
            raise: x("a_raise"),
            call: x("a_call"),
            fold: x("a_fold"),
            tiles: [
                x("tile_clubs"),
                x("tile_diamonds"),
                x("tile_hearts"),
                x("tile_spades"),
            ],
            tile_text: x("tile_text"),
            tile_diamonds_text: x("tile_diamonds_text"),
            tile_edge: x("tile_edge"),
            back: [x("back_a"), x("back_b")],
            back_edge: x("back_edge"),
            oval_rim: x("oval_rim"),
            felt: [x("felt_top"), x("felt_bottom")],
            felt_line: x("felt_edge"),
            felt_dash: x("felt_dash"),
            pod: x("pod"),
            pod_fold: x("pod_fold"),
            pod_bar: x("pod_bar"),
            bet_bg: x("bet_bg"),
            bet_edge: x("bet_edge"),
            bet_shade: x("bet_shade"),
            pot_label: x("pot_label"),
            pot_unit: x("pot_unit"),
            slot: x("slot"),
            dealer: x("dealer"),
            dealer_text: x("dealer_text"),
            actbar: x("actbar"),
            on_grade: x("on_grade"),
            shadow: x("shadow"),
            hero_outline: x("hero_outline"),
            cell_out: x("cell_out"),
        }
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
        let _ = TablePal::new(&ui);
        assert_eq!(ui.theme.warnings(), Vec::new());
    }
}
