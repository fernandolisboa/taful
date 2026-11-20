//! Salão's table screen: the mockup's arrangement (the controls, the felt
//! with hero's actions under it, the side panel) for the default layout,
//! and the layout grid for the others and for the edit mode.

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use gpui::{AnyElement, Context, Div, FontWeight, Stateful, Window, div, prelude::*, px};
use taful_app::model::{Grade, Street, Trainers};
use taful_app::preflop::{Action, Seat};
use taful_app::story::StoryEvent;
use taful_app::template::street_id;

use super::colors::colors;
use super::felt;
use super::frame::{pad, side_w};
use super::kit::{self, CardSize};
use crate::felt::{SeatExtras, TABLE_H, bb, num};
use crate::screen::Screen;
use crate::screens::table::{Mode, StoryTag, TableScreen, measure_height, places_like_default};
use crate::theme::alpha;
use crate::tour;

/// The mockup's numbers: the title and two rows of controls end here…
const CONTROLS_H: f32 = 171.7;
/// …and the zone (the felt, hero's actions and the session's strip)
/// starts this much above that, 900 × 720.
const ZONE_LAP: f32 = 13.7;
const ZONE_W: f32 = 900.;
const ZONE_H: f32 = 720.;
/// Under the zone and under the side panel, to the window's edge.
const ZONE_BOTTOM: f32 = 22.;
const PANEL_BOTTOM: f32 = 28.;
/// Between the zone and the side panel.
const PANEL_GAP: f32 = 8.;
/// The smallest zone, against the mockup's, that still gets the mockup's
/// arrangement.
const MIN_K: f32 = 0.45;
/// In a small window the side panel gives its room to the zone until the
/// zone is this large, down to the narrowest panel that holds its rows.
const EASY_K: f32 = 0.6;
const PANEL_MIN: f32 = 280.;
/// Controls narrower than this give the spots a row of their own instead
/// of the mockup's place beside the modes.
const SPOTS_BESIDE_W: f32 = 1000.;
/// A window shorter than this gives the title's row to the table (the
/// sidebar says which screen this is).
const TITLE_VH: f32 = 700.;

/// How the controls fit the window.
#[derive(Clone, Copy)]
struct ControlsFit {
    /// The screen's side padding.
    pad: f32,
    /// The mockup's rows: the spots beside the modes, and the side panel
    /// starting beside the last row, which keeps to `room_w`.
    mocked: bool,
    room_w: f32,
    /// The title's row is shown.
    title: bool,
}
/// The grade dots the session's strip shows.
const DOTS: usize = 12;

/// What Salão keeps on the table screen.
#[derive(Default)]
pub struct TableState {
    /// The controls' height on the last frame (many spots wrap).
    controls_h: Rc<Cell<f32>>,
    session: Session,
}

/// The decisions graded since the app opened (the strip under the felt).
#[derive(Default)]
struct Session {
    count: usize,
    ev_lost: f64,
    /// The last grades, oldest first.
    recent: VecDeque<Grade>,
    /// The spot answer already counted (the screen's generation).
    spot_seen: u64,
    /// The hand being counted (its seed) and how many of its decisions
    /// already were.
    hand: u64,
    hand_seen: usize,
}

impl Session {
    fn push(&mut self, grade: Grade, ev_loss_bb: f64) {
        self.count += 1;
        self.ev_lost += ev_loss_bb;
        self.recent.push_back(grade);
        if self.recent.len() > DOTS {
            self.recent.pop_front();
        }
    }
}

/// Counts the decisions graded since the last frame.
fn record(screen: &mut TableScreen) {
    let s = &mut screen.salao.session;
    match screen.mode {
        Mode::Hand => {
            let Some(h) = &screen.hand else { return };
            if s.hand != screen.seed {
                s.hand = screen.seed;
                s.hand_seen = 0;
            }
            for d in h.decisions.iter().skip(s.hand_seen) {
                s.push(d.graded.grade, d.graded.ev_loss_bb);
            }
            s.hand_seen = h.decisions.len();
        }
        _ => {
            if let Some(d) = &screen.answer
                && s.spot_seen != screen.generation
            {
                s.spot_seen = screen.generation;
                s.push(d.grade, d.ev_loss_bb);
            }
        }
    }
}

fn title(screen: &TableScreen) -> Div {
    div().relative().child(tour::anchor("screen_title")).child(
        kit::H1
            .text(34., kit::LEADING)
            .child(screen.ui.i18n.tr(Screen::Table.nav_key())),
    )
}

/// The title with the edit button, then the mode, spot and opponent
/// pickers as the mockup's two rows. In a narrow window the spots get a
/// row of their own, and in a short one the edit button moves to the
/// modes' row and the title goes.
fn controls(
    screen: &TableScreen,
    trainers: &Trainers,
    fit: ControlsFit,
    cx: &mut Context<TableScreen>,
) -> Div {
    let (c, p) = (colors(), screen.palette);
    let ui = screen.ui.clone();
    let i18n = &ui.i18n;
    // A row's label, pushed down to the middle of the row's first line.
    let label = |key: &str, w: f32, top: f32| {
        div().w(px(w)).flex_none().mt(px(top)).child(kit::label(
            kit::SANS_R16,
            10.5,
            c.dim,
            i18n.tr(key),
        ))
    };
    let pills = |items: Vec<Stateful<Div>>, top: f32| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.))
            .mt(px(top))
            .children(items)
    };
    let row = || div().flex().items_start().gap(px(10.));

    let spots = match screen.mode {
        Mode::Spot => screen.kind_chips(trainers, cx),
        Mode::Postflop => screen.postflop_chips(trainers, cx),
        Mode::Drill => screen
            .drill
            .as_ref()
            .map(|b| kit::pill("drill_bucket", TableScreen::bucket_label(&ui, b), true, &p))
            .into_iter()
            .collect(),
        Mode::Hand => Vec::new(),
    };
    let mut edit = Some(
        kit::btn("layout_edit", i18n.tr("layout-edit-start"), false, true, &p)
            .on_click(cx.listener(|this, _, _, cx| this.start_edit(cx))),
    );
    let mut spots = Some(spots).filter(|s| !s.is_empty());
    let beside = spots.take_if(|_| fit.mocked);
    let first = row()
        .child(label("table-row-mode", 84., 12.17))
        .child(kit::seg(&p).children(screen.mode_segments(cx)))
        // Without the title's row the edit button ends this one.
        .when(!fit.title && beside.is_none(), |d| d.child(div().flex_1()))
        .when_some(beside, |d, spots| {
            d.child(div().w(px(22.)).flex_none())
                .child(label("table-row-spot", 74., 12.17))
                .child(pills(spots, 3.36))
        })
        .children(edit.take_if(|_| !fit.title).map(|b| b.mt(px(4.))));
    // A narrow window: the spots under the modes, like the opponents.
    let spots_row = spots.map(|spots| {
        row()
            .mt(px(10.))
            .child(label("table-row-spot", 84., 8.81))
            .child(pills(spots, 0.))
    });
    let opponents = matches!(screen.mode, Mode::Spot | Mode::Postflop | Mode::Hand).then(|| {
        row()
            .mt(px(10.))
            .when(fit.mocked, |d| d.max_w(px(fit.room_w)))
            .child(label("table-opponent", 84., 8.81))
            .child(pills(screen.opponent_chips(trainers, cx), 0.))
    });
    div()
        .relative()
        .flex()
        .flex_col()
        .pt(px(26.))
        .px(px(fit.pad))
        .child(measure_height(screen.salao.controls_h.clone()))
        .when(fit.title, |d| {
            d.child(
                div()
                    .flex()
                    .items_end()
                    .justify_between()
                    .child(title(screen))
                    .children(edit),
            )
        })
        .child(
            div()
                .relative()
                .when(fit.title, |d| d.mt(px(14.)))
                .flex()
                .flex_col()
                .child(tour::anchor("table_controls"))
                .child(first)
                .children(spots_row)
                .children(opponents),
        )
}

/// The session so far: how many decisions, the last grades as dots, the
/// EV they lost.
fn strip(screen: &TableScreen) -> Option<Div> {
    let s = &screen.salao.session;
    if s.count == 0 {
        return None;
    }
    let (p, ui) = (screen.palette, &screen.ui);
    let text = |t: String| kit::sans_text(12.5, FontWeight::NORMAL).child(t);
    Some(
        div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .text_color(p.text_muted)
            .whitespace_nowrap()
            .child(text(
                ui.i18n
                    .tr_args("salao-session", &[("count", s.count.into())]),
            ))
            .child(
                div().flex().gap(px(5.)).children(
                    s.recent
                        .iter()
                        .map(|g| div().size(px(9.)).rounded(px(4.5)).bg(p.grade(*g))),
                ),
            )
            .child(text(ui.i18n.tr_args(
                "salao-session-ev",
                &[("amount", num(ui, s.ev_lost, 1).into())],
            ))),
    )
}

/// The table screen.
pub fn table(
    screen: &mut TableScreen,
    window: &mut Window,
    cx: &mut Context<TableScreen>,
) -> AnyElement {
    record(screen);
    let (c, p) = (colors(), screen.palette);
    let ui = screen.ui.clone();
    let i18n = &ui.i18n;
    let vp = window.viewport_size();
    let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
    let pad = pad(vw);
    let page = || {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(14.))
            .pt(px(26.))
            .px(px(pad))
            .pb(px(PANEL_BOTTOM))
    };
    let trainers = match &screen.trainers {
        Ok(t) => t.clone(),
        Err(e) => {
            return page()
                .child(title(screen))
                .child(crate::widgets::muted(
                    i18n.tr_args("table-chart-error", &[("error", e.clone().into())]),
                    &ui,
                    &p,
                ))
                .into_any_element();
        }
    };
    // The zone takes what the window leaves beside the side panel and
    // under the controls, at the mockup's proportions.
    let (pane_w, pane_h) = ((vw - side_w(vw)).max(0.), vh);
    // The zone and the panel share the pane: the mockup's panel wherever
    // the zone stays large beside it, a narrower one in a small window.
    let both = pane_w - 2. * pad - PANEL_GAP;
    let panel_w = (both - ZONE_W * EASY_K).clamp(PANEL_MIN, felt::PANEL_W);
    let room_w = both - panel_w;
    let fits = room_w >= ZONE_W * MIN_K
        && pane_h - (CONTROLS_H - ZONE_LAP) - ZONE_BOTTOM >= ZONE_H * MIN_K;
    // The mockup is the default layout; any other, the edit mode and a
    // window too small for the zone beside the panel get the grid with
    // this style's panels (it scrolls).
    if screen.is_editing() || !places_like_default(&screen.layout) || !fits {
        // The grid's explanation panel, at the last frame's cell size.
        felt::set_panel_width(
            screen
                .shown_layout()
                .rect(taful_app::layout::PanelId::Explanation, screen.cell)
                .map_or(felt::PANEL_W, |r| r.w),
        );
        // What the page leaves the grid: its padding and the title's row.
        screen.grid_room = Some((
            pane_w - 2. * pad,
            pane_h - 26. - PANEL_BOTTOM - 14. - 34. * kit::LEADING,
        ));
        return page()
            .id("salao_table_grid")
            .overflow_y_scroll()
            .child(title(screen))
            .child(screen.render_classic(window, cx))
            .into_any_element();
    }

    let controls_fit = ControlsFit {
        pad,
        mocked: pane_w - 2. * pad >= SPOTS_BESIDE_W,
        room_w,
        title: vh >= TITLE_VH,
    };
    // Controls that wrap push the zone down: it may shrink a little more.
    // With the mockup's rows the zone and the panel start beside the last
    // one; otherwise under it.
    let controls_h = screen.salao.controls_h.get();
    let (top, panel_top) = if controls_fit.mocked {
        let top = if controls_fit.title {
            controls_h.max(CONTROLS_H)
        } else {
            controls_h
        } - ZONE_LAP;
        (top, top)
    } else {
        (controls_h, controls_h + PANEL_GAP)
    };
    let room_h = pane_h - top - ZONE_BOTTOM;
    let k = (room_w / ZONE_W)
        .min(room_h / ZONE_H)
        .clamp(MIN_K * 0.7, 1.6);
    let left = pad + ((room_w - ZONE_W * k) / 2.).max(0.);

    felt::set_panel_width(panel_w);
    let parts = match screen.mode {
        Mode::Hand => screen.hand_parts(&trainers, k * felt::H / TABLE_H, cx),
        _ => screen.question_parts(&trainers, k * felt::H / TABLE_H, cx),
    };
    let dealt = match screen.mode {
        Mode::Hand => screen.hand.is_some(),
        Mode::Postflop if screen.postflop_message(&trainers).is_some() => false,
        _ => screen.question.is_some(),
    };
    let answered = screen.mode != Mode::Hand && screen.answer.is_some();
    // After the answer the felt shows it played: hero's chips in front,
    // the pot with them. A fold changes no chips: the same table, with
    // hero out.
    let after = match (&screen.question, &screen.answer) {
        (Some(q), Some(d)) if answered && !screen.playing() => match d.actions.get(d.chosen) {
            Some(Action::Fold) => Some(felt::draw_folded(&ui, &p, q.state(), q.hero(), k)),
            Some(&a) => q
                .state()
                .apply(a)
                .ok()
                .map(|s| felt::draw(&ui, &p, &s, q.hero(), false, k, SeatExtras::default())),
            None => None,
        },
        _ => None,
    };
    let table = if dealt {
        after.unwrap_or(parts.table)
    } else {
        // Nothing dealt: the empty table, with why.
        felt::base(k).child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .max_w(px(440. * k))
                        .px(px(18.))
                        .py(px(12.))
                        .rounded(px(12.))
                        .bg(alpha(c.bet_bg, 0.6))
                        .child(parts.table),
                ),
        )
    };
    let repeat = answered.then(|| {
        kit::btn(
            "repeat_spot",
            i18n.tr("salao-repeat-spot"),
            false,
            false,
            &p,
        )
        .on_click(cx.listener(|this, _, _, cx| this.repeat_spot(cx)))
    });

    let zone = div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(ZONE_W * k))
        .h(px(ZONE_H * k))
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(felt::TOP * k))
                .child(tour::anchor("table_felt"))
                .child(table),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(608. * k - 20.))
                .w_full()
                .flex()
                .justify_center()
                .child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(tour::anchor("table_actions"))
                        .child(parts.actions)
                        .children(repeat),
                ),
        )
        .children(strip(screen).map(|s| s.absolute().left_0().top(px(657.87 * k)).w_full()));

    let side = kit::panel(&p)
        .absolute()
        .right(px(pad))
        .top(px(panel_top))
        .bottom(px(PANEL_BOTTOM))
        .w(px(panel_w))
        .child(tour::anchor("table_explanation"))
        .child(
            div()
                .id("salao_explanation")
                .relative()
                .size_full()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .px(px(22.))
                .pt(px(22.))
                .pb(px(18.))
                .child(parts.panel.flex_1()),
        );

    div()
        .relative()
        .size_full()
        .overflow_hidden()
        .child(controls(screen, &trainers, controls_fit, cx))
        .child(zone)
        .child(side)
        .into_any_element()
}

/// Hero's answer as the story's last line (the spot modes grade one
/// decision; the story the screen keeps ends before it).
fn answer_tag(screen: &TableScreen, hero: Seat) -> Option<StoryTag> {
    if screen.mode == Mode::Hand {
        return None;
    }
    let (q, d) = (screen.question.as_ref()?, screen.answer.as_ref()?);
    let pb = screen.playback.as_ref()?;
    let i18n = &screen.ui.i18n;
    let state = q.state();
    let a = *d.actions.get(d.chosen)?;
    let amount = |chips| bb(&screen.ui, chips).into();
    let what = match a {
        // A preflop raise is an open or an n-bet, as in the story's lines.
        Action::Raise(to) if state.street() == Street::Preflop => {
            let raises = pb
                .story
                .events
                .iter()
                .filter(|e| {
                    matches!(e, StoryEvent::Action(x)
                        if x.street == Street::Preflop && matches!(x.action, Action::Raise(_)))
                })
                .count();
            if raises == 0 {
                i18n.tr_args("table-tl-open", &[("amount", amount(to))])
            } else {
                i18n.tr_args(
                    "table-tl-nbet",
                    &[("n", (raises + 2).into()), ("amount", amount(to))],
                )
            }
        }
        _ => crate::felt::action_label(&screen.ui, state, a),
    };
    Some(StoryTag {
        who: i18n.tr_args(
            "table-story-you",
            &[("position", state.position(hero).label().into())],
        ),
        what,
        step: pb.last(),
        hero: true,
        fold: a == Action::Fold,
        aggressive: a.is_aggressive(),
    })
}

/// How the hand got here: a block per street with a line per action (a
/// line shows the felt at that moment), and the replay's controls at the
/// panel's bottom.
pub fn story(screen: &TableScreen, hero: Seat, cx: &mut Context<TableScreen>) -> Option<Div> {
    let pb = screen.playback.as_ref()?;
    let mut cols = screen.story_columns(hero)?;
    if let (Some(tag), Some(col)) = (answer_tag(screen, hero), cols.last_mut()) {
        col.tags.push(tag);
    }
    let (c, p) = (colors(), screen.palette);
    let ui = screen.ui.clone();
    let i18n = &ui.i18n;
    let (at, last, playing) = (pb.at, pb.last(), pb.playing());

    let blocks: Vec<Div> = cols
        .into_iter()
        .enumerate()
        // A street the replay has not dealt yet stays hidden.
        .filter(|(_, col)| {
            col.street == Street::Preflop
                || !playing
                || col.tags.first().is_some_and(|t| t.step <= at)
        })
        .map(|(ci, col)| {
            let name = kit::TEXT
                .text(15.5, kit::LEADING)
                .text_color(p.text)
                .child(i18n.tr(&format!("street-{}", street_id(col.street))));
            let head = div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .pt(px(6.))
                .pb(px(5.))
                .mb(px(3.))
                .border_b_1()
                .border_color(c.line)
                .when(ci > 0, |d| d.mt(px(8.)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(name)
                        .children(
                            col.cards
                                .iter()
                                .map(|&card| kit::playing_card(&p, Some(card), CardSize::Xs, 1.)),
                        ),
                )
                .child(
                    kit::sans_text(12., FontWeight::NORMAL)
                        .text_color(p.text_muted)
                        .child(
                            i18n.tr_args("table-tl-pot", &[("amount", bb(&ui, col.pot).into())]),
                        ),
                );
            let rows = col
                .tags
                .into_iter()
                .enumerate()
                .filter(|(_, t)| !playing || t.step <= at)
                .map(|(ri, t)| {
                    let step = t.step;
                    let (who, what) = if t.hero {
                        (c.gold2, c.gold2)
                    } else if t.aggressive {
                        (p.text_muted, p.text)
                    } else {
                        (p.text_muted, p.text_muted)
                    };
                    let weight = if t.hero || t.aggressive {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    };
                    div()
                        .id(("salao_story", ci * 100 + ri))
                        .flex()
                        .px(px(4.))
                        .py(px(2.5))
                        .rounded(px(5.))
                        .cursor_pointer()
                        .when(playing && step == at, |d| d.bg(alpha(p.accent, 0.12)))
                        .hover(|s| s.bg(alpha(p.text, 0.05)))
                        .on_click(cx.listener(move |this, _, _, cx| this.jump_to(step, cx)))
                        .child(
                            div().w(px(118.)).flex_none().child(
                                kit::sans_text(13., FontWeight::NORMAL)
                                    .text_color(who)
                                    .child(t.who),
                            ),
                        )
                        .child(kit::sans_text(13., weight).text_color(what).child(t.what))
                });
            div().flex().flex_col().child(head).children(rows)
        })
        .collect();

    let small = |id: &'static str, label: String, enabled: bool| {
        kit::btn(id, label, false, true, &p).when(!enabled, |b| b.opacity(0.4))
    };
    // Pinned to the panel's bottom (out of the flow, so the lines above
    // do not round it a pixel off). A narrow panel has the automatic
    // replay's switch on a row of its own.
    let narrow = felt::narrow_panel();
    let replay = div()
        .absolute()
        .left_0()
        .bottom_0()
        .w_full()
        .flex()
        .when(narrow, |d| d.flex_wrap())
        .items_center()
        .gap(px(8.))
        .child(
            small(
                "story_play",
                i18n.tr("table-replay-again"),
                !pb.story.events.is_empty(),
            )
            .on_click(cx.listener(|this, _, _, cx| this.replay_again(cx))),
        )
        .child(
            small("story_prev", "‹".into(), at > 0)
                .px(px(11.))
                .on_click(
                    cx.listener(move |this, _, _, cx| this.jump_to(at.saturating_sub(1), cx)),
                ),
        )
        .child(
            small("story_next", "›".into(), at < last)
                .px(px(11.))
                .on_click(cx.listener(move |this, _, _, cx| this.jump_to((at + 1).min(last), cx))),
        )
        .child(if narrow {
            div().w_full().h(px(0.))
        } else {
            div().flex_1()
        })
        .child(
            kit::btn("auto_replay", i18n.tr("table-replay-auto"), false, true, &p)
                .when(screen.auto_replay, |b| {
                    b.text_color(c.gold2).border_color(alpha(p.accent, 0.45))
                })
                .on_click(cx.listener(|this, _, _, cx| this.toggle_auto_replay(cx))),
        );
    Some(
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            // Room for the replay's controls and the least space over them.
            .pb(px(if narrow { 2. * 32. + 8. } else { 32. } + 14.))
            // The mockup's 22 pixels over the last option, less the gap
            // the panel already puts between its parts.
            .mt(px(22. - ui.theme.spacing.lg))
            .child(felt::section(i18n.tr("table-story-title"), None).mb(px(8.)))
            .children(blocks)
            .child(replay),
    )
}
