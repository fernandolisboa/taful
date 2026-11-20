//! The table screen as Baralho draws it: a strip of controls, the table
//! with hero's buttons and the session under it, and the result card at
//! the right. The built-in default layout sits exactly where the mockup has
//! it; any other layout places the same four panels on its grid.

use std::rc::Rc;

use gpui::{AnyElement, Context, Div, ElementId, Stateful, Window, div, prelude::*, px};
use taful_app::layout::{Layout, PanelId, Rect};
use taful_app::model::{DrillSpot, FullHand, Grade, HandEnd, LeftReason, Trainers};
use taful_app::preflop::{Action, Chips, Seat};
use taful_app::story::Story;

use super::kit::{self, CardSize, Fill, Raised};
use super::{Bx, CANVAS_W, bx, result, scale, tabletop, typo, u};
use crate::Ui;
use crate::felt::{self, Explain, SeatExtras};
use crate::screens::table::{Busy, Mode, TableEvent, TableScreen};
use crate::theme::alpha;

/// Where the mockup puts the default layout's panels (design pixels:
/// left, top, width, height).
const CONTROLS: [f32; 4] = [48., 88., 1504., 44.];
const TABLE: [f32; 4] = [48., 150., 1010., 584.];
const ACTIONS: [f32; 4] = [48., 750., 1010., 140.];
const RESULT: [f32; 4] = [1100., 150., 452., 716.];
/// Any other layout: its grid starts here and ends at the canvas' bottom
/// margin, between the side margins.
const GRID_TOP: f32 = 80.;
const GRID_BOTTOM: f32 = 890.;
const GRID_MARGIN: f32 = 48.;
/// Room kept inside each of the grid's panels.
const INSET: f32 = 8.;
/// The session strip shows this many grades.
const DOTS: usize = 12;

/// The open dropdown.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Menu {
    Spot,
    Opponent,
}

/// What Baralho keeps on the table screen.
#[derive(Default)]
pub struct TableState {
    menu: Option<Menu>,
    /// This session's graded decisions, oldest first: grade and EV lost.
    session: Vec<(Grade, f64)>,
    /// The deal the session last counted (its seed), and how many of its
    /// decisions.
    counted: (u64, usize),
    /// The spot on the table is being answered again: the new answer is
    /// graded but not stored, and the session counts the first one only.
    repeat: bool,
    /// The screen's `ui` with the theme's sizes scaled to the window (the
    /// shared edit bar reads them).
    scaled: Option<(f32, Rc<Ui>, Rc<Ui>)>,
}

/// Adds the decisions graded since the last frame to the session.
fn count_session(s: &mut TableScreen) {
    if s.baralho.counted.0 != s.seed {
        s.baralho.counted = (s.seed, 0);
        s.baralho.repeat = false;
    }
    let graded: Vec<(Grade, f64)> = match s.mode {
        Mode::Hand => s
            .hand
            .iter()
            .flat_map(|h| &h.decisions)
            .map(|d| (d.graded.grade, d.graded.ev_loss_bb))
            .collect(),
        _ => s.answer.iter().map(|d| (d.grade, d.ev_loss_bb)).collect(),
    };
    let done = s.baralho.counted.1;
    s.baralho.session.extend(graded.iter().skip(done));
    s.baralho.counted.1 = done.max(graded.len());
}

/// Hero's answer. A repeated spot is graded here, so nothing is stored.
fn act(s: &mut TableScreen, action: Action, cx: &mut Context<TableScreen>) {
    if !s.baralho.repeat || s.mode == Mode::Hand {
        s.act(action, cx);
        return;
    }
    if s.busy.is_some() || s.playing() || s.answer.is_some() {
        return;
    }
    if let (Ok(t), Some(q)) = (&s.trainers, &s.question) {
        match t.answer(q, action) {
            Ok(d) => s.answer = Some(d),
            Err(e) => eprintln!("taful: answer: {e}"),
        }
    }
    cx.notify();
}

/// Puts the answered spot back on the table to be answered again.
fn repeat_spot(s: &mut TableScreen, cx: &mut Context<TableScreen>) {
    let Some(state) = s.question.as_ref().map(|q| q.state().clone()) else {
        return;
    };
    s.answer = None;
    s.baralho.repeat = true;
    s.show_story(&state, Some(0), cx);
    cx.notify();
}

/// Whether `layout` places its panels like the built-in default.
fn is_default(layout: &Layout) -> bool {
    let default = Layout::builtin_default();
    layout.grid.cols == default.grid.cols
        && layout.grid.rows == default.grid.rows
        && PanelId::ALL
            .iter()
            .all(|&id| layout.rect(id, 1.) == default.rect(id, 1.))
}

/// A mockup rectangle in window pixels.
fn mock_rect(id: PanelId) -> Rect {
    let [x, y, w, h] = match id {
        PanelId::Controls => CONTROLS,
        PanelId::Table => TABLE,
        PanelId::Actions => ACTIONS,
        PanelId::Explanation => RESULT,
    };
    let s = scale();
    Rect {
        x: x * s,
        y: y * s,
        w: w * s,
        h: h * s,
    }
}

/// What the table, its buttons and the card show.
struct Scene {
    table: Div,
    actions: Div,
    /// The card's head and what scrolls under it; no card without a spot.
    card: Option<(Div, Vec<AnyElement>)>,
    hero: Option<Seat>,
    /// The story with hero's answer at its end (an answered spot).
    answered: Option<Story>,
}

impl Scene {
    /// Only a line of text on an empty table.
    fn notice(bx: &Bx, fit: (f32, f32), text: String) -> Scene {
        Scene {
            table: tabletop::fitted(fit.0, fit.1, || tabletop::empty(bx, Some(text))),
            actions: div(),
            card: None,
            hero: None,
            answered: None,
        }
    }
}

/// A line of text among the buttons.
fn say(bx: &Bx, text: String) -> Div {
    typo::text(14.5, 600)
        .on(div())
        .text_color(bx.on_felt_mut)
        .child(text)
}

fn busy_text(s: &TableScreen) -> String {
    s.ui.i18n.tr(match s.busy {
        Some(Busy::Dealing) => "table-busy-dealing",
        Some(Busy::Acting) => "table-busy-acting",
        None => "table-no-spot",
    })
}

/// A button wired to the screen.
fn button(
    id: impl Into<ElementId>,
    label: String,
    fill: Fill,
    bx: &Bx,
    cx: &mut Context<TableScreen>,
    on: impl Fn(&mut TableScreen, &mut Context<TableScreen>) + 'static,
) -> Stateful<Div> {
    kit::btn(id, label, fill, false, bx).on_click(cx.listener(move |this, _, _, cx| on(this, cx)))
}

/// Hero's buttons: exactly the tree's sizes (D4).
fn action_buttons(
    s: &TableScreen,
    bx: &Bx,
    state: &taful_app::preflop::GameState,
    actions: &[Action],
    cx: &mut Context<TableScreen>,
) -> Div {
    let buttons: Vec<_> = actions
        .iter()
        .enumerate()
        .map(|(i, &a)| {
            let fill = if matches!(a, Action::Raise(_) | Action::AllIn) {
                Fill::Amber
            } else {
                Fill::Cream
            };
            button(
                ("act", i),
                felt::action_label(&s.ui, state, a),
                fill,
                bx,
                cx,
                move |this, cx| act(this, a, cx),
            )
        })
        .collect();
    row().children(buttons)
}

/// The row the buttons sit in.
fn row() -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_center()
        .gap_x(u(12.))
        .gap_y(u(10.))
}

/// The button that stands in for hero's actions during a replay.
fn skip_bar(s: &TableScreen, bx: &Bx, cx: &mut Context<TableScreen>) -> Div {
    let i18n = &s.ui.i18n;
    let ticking = s.playback.as_ref().is_some_and(|p| p.ticking);
    row()
        .child(say(
            bx,
            i18n.tr(if ticking {
                "table-replay-playing"
            } else {
                "table-replay-earlier"
            }),
        ))
        .child(button(
            "replay_skip",
            i18n.tr(if ticking {
                "table-replay-skip"
            } else {
                "table-replay-back"
            }),
            Fill::Amber,
            bx,
            cx,
            |this, cx| this.skip_replay(cx),
        ))
}

/// The head of the card while hero has not answered: hero's seat on the
/// seal, whose turn it is and what to do.
fn waiting_head(bx: &Bx, position: &'static str, title: String, body: String) -> Div {
    result::head(
        result::seal(
            bx,
            bx.amber,
            typo::disp(17., 800, 20)
                .on(div())
                .text_color(bx.ink)
                .child(position),
        ),
        title,
        bx.ink,
        32.,
        Some(result::head_line(bx, body)),
        None,
    )
}

/// A single-decision question (spot, postflop, drill).
fn question_scene(
    s: &TableScreen,
    t: &Trainers,
    fit: (f32, f32),
    cx: &mut Context<TableScreen>,
) -> Scene {
    let (ui, p) = (s.ui.clone(), s.palette);
    let bx = bx(&ui);
    let i18n = &ui.i18n;
    if s.mode == Mode::Postflop
        && let Some(msg) = s.postflop_message(t)
    {
        return Scene::notice(&bx, fit, msg);
    }
    let Some(q) = &s.question else {
        return Scene::notice(&bx, fit, busy_text(s));
    };
    let hero = q.hero();
    let position = q.state().position(hero).label();
    let (head, mut body, actions) = match &s.answer {
        Some(d) => {
            let labels: Vec<String> = d
                .actions
                .iter()
                .map(|&a| felt::action_label(&ui, q.state(), a))
                .collect();
            (
                result::graded(&ui, &p, &bx, &Explain::of(d), Some(&labels)),
                Vec::new(),
                row()
                    .child(button(
                        "next",
                        i18n.tr("table-next-spot"),
                        Fill::Amber,
                        &bx,
                        cx,
                        |this, cx| {
                            this.deal(cx);
                            cx.notify();
                        },
                    ))
                    .child(button(
                        "repeat",
                        i18n.tr("baralho-repeat-spot"),
                        Fill::Cream,
                        &bx,
                        cx,
                        repeat_spot,
                    )),
            )
        }
        None if s.playing() => (
            waiting_head(
                &bx,
                position,
                i18n.tr("baralho-replay-title"),
                i18n.tr("table-replay-panel"),
            ),
            Vec::new(),
            skip_bar(s, &bx, cx),
        ),
        None => (
            waiting_head(
                &bx,
                position,
                i18n.tr("baralho-turn-title"),
                i18n.tr("baralho-turn-body"),
            ),
            prompt_notes(&ui, &bx, q),
            action_buttons(s, &bx, q.state(), &q.actions(t), cx),
        ),
    };
    if s.baralho.repeat {
        body.insert(
            0,
            result::note(&bx, i18n.tr("baralho-repeat-note")).into_any_element(),
        );
    }
    // Once answered, the table shows hero's action too (a fold keeps the
    // cards on the table and only says so on the plate).
    let chosen = s
        .answer
        .as_ref()
        .filter(|_| !s.playing())
        .and_then(|d| d.actions.get(d.chosen).copied());
    let applied = chosen.and_then(|a| q.state().apply(a).ok());
    let answered = applied.as_ref().and_then(|a| Story::of(a).ok());
    let after = applied.filter(|_| chosen != Some(Action::Fold));
    let said = chosen
        .filter(|_| after.is_none())
        .map(|a| felt::action_label(&ui, q.state(), a));
    let state = after.as_ref().unwrap_or_else(|| s.shown(q.state()));
    Scene {
        table: tabletop::fitted(fit.0, fit.1, || {
            tabletop::table(
                &ui,
                &p,
                &bx,
                state,
                hero,
                false,
                SeatExtras::default(),
                said,
                // An answered spot stays as the mockup draws it.
                chosen.is_none(),
            )
        }),
        actions,
        card: Some((head, body)),
        hero: Some(hero),
        answered,
    }
}

/// Who the spot is against, when it is not the reference.
fn prompt_notes(ui: &Ui, bx: &Bx, q: &DrillSpot) -> Vec<AnyElement> {
    let i18n = &ui.i18n;
    let (opponent, fallback) = match q {
        DrillSpot::Preflop(s) => (s.opponent.as_deref(), None),
        DrillSpot::Postflop(s) => (s.opponent.as_deref(), s.fallback.as_deref()),
    };
    let mut out = Vec::new();
    if let Some(id) = fallback {
        out.push(i18n.tr_args(
            "table-profile-fallback",
            &[("profile", i18n.tr(&format!("bot-profile-{id}")).into())],
        ));
    }
    if let Some(id) = opponent {
        // A drill of a mixed-table decision (#29) is against the table
        // its key names.
        out.push(if taful_app::mixed::is_table_key(id) {
            i18n.tr_args(
                "table-vs-table-prompt",
                &[("table", i18n.opponent(id).into())],
            )
        } else {
            i18n.tr_args(
                "table-vs-profile-prompt",
                &[
                    ("profile", i18n.tr(&format!("bot-profile-{id}")).into()),
                    ("about", i18n.tr(&format!("bot-profile-{id}-about")).into()),
                ],
            )
        });
    }
    out.into_iter()
        .map(|t| result::note(bx, t).into_any_element())
        .collect()
}

/// How a full hand ended, in words.
fn end_text(ui: &Ui, h: &FullHand) -> Option<String> {
    let i18n = &ui.i18n;
    h.end.as_ref().map(|e| match e {
        HandEnd::Settled(_) | HandEnd::HeroFolded => i18n.tr_args(
            "table-hand-over-settled",
            &[(
                "amount",
                felt::num(ui, h.hero_net_bb().unwrap_or(0.), 1).into(),
            )],
        ),
        HandEnd::LeftLibrary { history, reason } => {
            let key = match reason {
                LeftReason::NoLibrary => "table-hand-over-flop",
                LeftReason::Unsolved => "table-hand-over-unsolved",
                LeftReason::OffTree => "table-hand-over-left",
                LeftReason::NotInRange => "table-hand-over-range",
                LeftReason::BotOffRange => "table-hand-over-bot-range",
            };
            let profile = h
                .remaining_bot_profile()
                .or(h.table_profile())
                .map(|id| i18n.tr(&format!("bot-profile-{id}")))
                .unwrap_or_default();
            i18n.tr_args(
                key,
                &[
                    ("line", history.to_string().into()),
                    ("profile", profile.into()),
                ],
            )
        }
    })
}

/// A full hand, from the deal to its end.
fn hand_scene(
    s: &TableScreen,
    t: &Trainers,
    fit: (f32, f32),
    cx: &mut Context<TableScreen>,
) -> Scene {
    let (ui, p) = (s.ui.clone(), s.palette);
    let bx = bx(&ui);
    let i18n = &ui.i18n;
    let Some(h) = &s.hand else {
        return Scene::notice(&bx, fit, busy_text(s));
    };
    let playing = s.playing();
    let over = h.end.is_some() && !playing;
    let position = h.state.position(h.hero).label();
    let name = |id: &str| i18n.tr(&format!("bot-profile-{id}"));
    let para = |text: String| result::note(&bx, text).into_any_element();

    let head = match h.decisions.last() {
        Some(d) => result::graded(&ui, &p, &bx, &Explain::of(&d.graded), None),
        None if playing => waiting_head(
            &bx,
            position,
            i18n.tr("baralho-replay-title"),
            i18n.tr("table-replay-panel"),
        ),
        None if over => waiting_head(
            &bx,
            position,
            i18n.tr("baralho-over-title"),
            i18n.tr("baralho-over-body"),
        ),
        None => waiting_head(
            &bx,
            position,
            i18n.tr("baralho-turn-title"),
            i18n.tr("baralho-turn-body"),
        ),
    };
    let mut body: Vec<AnyElement> = Vec::new();
    if let Some(id) = h.table_profile() {
        body.push(para(i18n.tr_args(
            "table-hand-vs-profile",
            &[
                ("profile", name(id).into()),
                ("about", i18n.tr(&format!("bot-profile-{id}-about")).into()),
            ],
        )));
    } else if h.is_mixed() {
        body.push(para(i18n.tr("table-hand-vs-mixed")));
    }
    if let Some(id) = &h.fallback
        && h.walk().is_some()
    {
        body.push(para(i18n.tr_args(
            "table-hand-profile-fallback",
            &[("profile", name(id).into())],
        )));
    }
    let next = s.table_bots(t);
    if !over && next != h.bots() {
        let first = next.first().cloned().flatten();
        body.push(para(if next.iter().all(|b| *b == first) {
            let next = first.as_deref().unwrap_or("reference");
            i18n.tr_args("table-hand-next-table", &[("profile", name(next).into())])
        } else {
            i18n.tr("table-hand-next-mixed")
        }));
    }
    if !h.decisions.is_empty() {
        body.push(result::section(&bx, i18n.tr("table-decisions"), None).into_any_element());
        for d in &h.decisions {
            body.push(
                typo::text(13.5, 600)
                    .on(div())
                    .flex()
                    .flex_none()
                    .justify_between()
                    .gap(u(10.))
                    .py(u(2.))
                    .px(u(2.))
                    .child(format!(
                        "{} · {} · {}",
                        tabletop::street_name(&ui, d.context.street),
                        d.graded.position.label(),
                        felt::action_short(&ui, d.graded.actions[d.graded.chosen])
                    ))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(p.grade(d.graded.grade))
                            .child(i18n.tr(&felt::grade_key(d.graded.grade))),
                    )
                    .into_any_element(),
            );
        }
    }
    if let Some(walk) = h.walk()
        && walk.on_solved_river()
        && !over
    {
        body.push(para(i18n.tr("table-river-solved")));
    }
    let actions = if playing {
        skip_bar(s, &bx, cx)
    } else if let Some(text) = end_text(&ui, h) {
        body.push(
            typo::text(14.5, 700)
                .on(div())
                .mt(u(14.))
                .child(text)
                .into_any_element(),
        );
        body.push(para(i18n.tr_args(
            "table-hand-ev-loss",
            &[("amount", felt::num(&ui, h.total_ev_loss_bb(), 2).into())],
        )));
        row()
            .child(button(
                "new_hand",
                i18n.tr("table-new-hand"),
                Fill::Amber,
                &bx,
                cx,
                |this, cx| {
                    this.deal(cx);
                    cx.notify();
                },
            ))
            .when(!h.decisions.is_empty(), |d| {
                d.child(button(
                    "review_hand",
                    i18n.tr("table-review-hand"),
                    Fill::Cream,
                    &bx,
                    cx,
                    |_, cx| cx.emit(TableEvent::ReviewHand),
                ))
            })
    } else if s.busy.is_some() {
        row().child(say(&bx, busy_text(s)))
    } else {
        action_buttons(s, &bx, &h.state, &h.hero_actions(&t.preflop), cx)
    };
    // Winners' takes (their pot shares after rake), slid from the pot once
    // the hand is settled.
    let mut wins: Vec<(Seat, Chips)> = Vec::new();
    if let (Some(HandEnd::Settled(st)), true) = (&h.end, over) {
        for pot in &st.pots {
            let share = (pot.amount - pot.rake) / pot.winners.len().max(1) as Chips;
            for &w in &pot.winners {
                match wins.iter_mut().find(|(seat, _)| *seat == w) {
                    Some((_, c)) => *c += share,
                    None => wins.push((w, share)),
                }
            }
        }
        wins.retain(|(_, c)| *c > 0);
    }
    let tags = felt::seat_tags(&ui, &h.seat_profiles);
    Scene {
        table: tabletop::fitted(fit.0, fit.1, || {
            tabletop::table(
                &ui,
                &p,
                &bx,
                s.shown(&h.state),
                h.hero,
                over,
                SeatExtras {
                    tags: &tags,
                    wins: &wins,
                },
                None,
                !over,
            )
        }),
        actions,
        card: Some((head, body)),
        hero: Some(h.hero),
        answered: None,
    }
}

/// How the hand got here (#53): a block per street (its board and the pot
/// it started with) with a row per action. A row shows the table at that
/// moment. `answered` is the story with hero's answer at its end.
fn story(
    s: &TableScreen,
    hero: Seat,
    answered: Option<&Story>,
    cx: &mut Context<TableScreen>,
) -> Vec<AnyElement> {
    let Some(pb) = &s.playback else {
        return Vec::new();
    };
    let (ui, p) = (s.ui.clone(), s.palette);
    let bx = bx(&ui);
    let i18n = &ui.i18n;
    let (at, playing) = (pb.at, pb.playing());
    let mut out = vec![result::section(&bx, i18n.tr("table-story-title"), None).into_any_element()];
    // With hero's answer as the last row, once the replay is over.
    let told = answered.filter(|_| !playing).unwrap_or(&pb.story);
    let blocks = tabletop::blocks(&ui, told, hero, true);
    let shown = blocks.into_iter().enumerate().filter(|(i, b)| {
        // A street the replay has not dealt yet stays hidden.
        *i == 0 || !playing || b.lines.first().is_some_and(|l| l.step <= at)
    });
    for (i, block) in shown {
        let header = typo::disp(16., 800, 20)
            .lh(1.4)
            .on(div())
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .py(u(5.))
            .mb(u(3.))
            .when(i > 0, |d| d.mt(u(8.)))
            .border_b(u(2.))
            .border_color(alpha(bx.ink, 0.14))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(u(4.))
                    .child(
                        div()
                            .mr(u(4.))
                            .child(tabletop::street_name(&ui, block.street)),
                    )
                    .children(
                        block
                            .cards
                            .iter()
                            .map(|&c| kit::playing_card(&bx, &p, Some(c), CardSize::XS)),
                    ),
            )
            .child(
                typo::text(12.5, 600)
                    .on(div())
                    .text_color(bx.mut_ink)
                    .child(i18n.tr_args(
                        "table-tl-pot",
                        &[("amount", felt::bb(&ui, block.pot).into())],
                    )),
            );
        out.push(header.into_any_element());
        for line in block.lines.into_iter().filter(|l| !playing || l.step <= at) {
            let step = line.step;
            let tone = if line.hero { p.suit_hearts } else { bx.mut_ink };
            let wash = bx.cream2;
            out.push(
                typo::text(13.5, 600)
                    .on(div())
                    .id(("story", step))
                    .flex()
                    .flex_none()
                    .py(u(2.))
                    .px(u(2.))
                    .rounded(u(5.))
                    .cursor_pointer()
                    .text_color(tone)
                    .when(playing && step == at, |d| d.bg(wash))
                    .hover(move |d| d.bg(wash))
                    .on_click(cx.listener(move |this, _, _, cx| this.jump_to(step, cx)))
                    .child(div().flex_none().w(u(120.)).child(line.who))
                    .child(
                        div()
                            .when(!line.fold, |d| d.font_weight(gpui::FontWeight::BOLD))
                            .when(!line.fold && !line.hero, |d| d.text_color(bx.ink))
                            .child(line.what),
                    )
                    .into_any_element(),
            );
        }
    }
    out
}

/// The replay's controls at the foot of the card.
fn replay_controls(s: &TableScreen, cx: &mut Context<TableScreen>) -> Option<Div> {
    let pb = s.playback.as_ref()?;
    let bx = bx(&s.ui);
    let i18n = &s.ui.i18n;
    let (at, last) = (pb.at, pb.last());
    let small = |id: &'static str, label: String, fill: Fill, pad: f32| {
        kit::btn_padded(id, label, fill, true, pad, &bx)
    };
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(u(8.))
            .pt(u(12.))
            .pb(u(3.))
            .child(
                small(
                    "story_play",
                    i18n.tr("table-replay-again"),
                    Fill::Cream,
                    14.,
                )
                .on_click(cx.listener(|this, _, _, cx| this.replay_again(cx))),
            )
            .child(small("story_prev", "‹".into(), Fill::Cream, 12.).on_click(
                cx.listener(move |this, _, _, cx| this.jump_to(at.saturating_sub(1), cx)),
            ))
            .child(
                small("story_next", "›".into(), Fill::Cream, 12.).on_click(
                    cx.listener(move |this, _, _, cx| this.jump_to((at + 1).min(last), cx)),
                ),
            )
            .child(div().flex_1())
            .child(
                small(
                    "auto_replay",
                    i18n.tr("table-replay-auto"),
                    if s.auto_replay {
                        Fill::Ink
                    } else {
                        Fill::Cream
                    },
                    14.,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_auto_replay(cx))),
            ),
    )
}

/// One mode of the segmented control.
fn segment(
    s: &TableScreen,
    bx: &Bx,
    id: &'static str,
    key: &str,
    mode: Mode,
    cx: &mut Context<TableScreen>,
) -> Stateful<Div> {
    let on = s.mode == mode;
    let (amber, ink, deep) = (bx.amber, bx.ink, bx.deep);
    typo::text(14., 700)
        .on(div())
        .id(id)
        .px(u(16.))
        .py(u(6.))
        .rounded_full()
        .whitespace_nowrap()
        .cursor_pointer()
        .map(|d| {
            if on {
                // The ring around the lit segment takes no room.
                d.bg(amber)
                    .text_color(ink)
                    .border(u(2.))
                    .border_color(deep)
                    .m(u(-2.))
            } else {
                d.opacity(0.85).hover(|d| d.opacity(1.))
            }
        })
        .child(s.ui.i18n.tr(key))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.baralho.menu = None;
            this.set_mode(mode, cx);
            cx.notify();
        }))
}

/// A dropdown's pill: a small label, the value and, when it opens, a caret.
fn pill(bx: &Bx, label: String, value: String, opens: bool) -> Div {
    typo::text(14., 700)
        .on(div())
        .flex()
        .flex_none()
        .items_center()
        .gap(u(8.))
        .h(u(42.))
        .px(u(16.))
        .rounded_full()
        .bg(alpha(bx.well, 0.5))
        .border(u(2.))
        .border_color(bx.deep)
        .whitespace_nowrap()
        .child(
            typo::caps(11., 700, 100)
                .on(div())
                .text_color(bx.on_felt_mut)
                .child(label.to_uppercase()),
        )
        .child(value)
        .when(opens, |d| {
            d.child(kit::icon(kit::CARET, 10., alpha(bx.on_felt, 0.7)).ml(u(4.)))
        })
}

/// One row of a dropdown's list.
fn menu_row(bx: &Bx, id: impl Into<ElementId>, label: String, selected: bool) -> Stateful<Div> {
    let wash = bx.cream2;
    typo::text(14., if selected { 700 } else { 600 })
        .on(div())
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .gap(u(16.))
        .px(u(12.))
        .py(u(7.))
        .rounded(u(9.))
        .text_color(bx.ink)
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(move |d| d.bg(wash))
        .child(label)
        .children(selected.then(|| kit::icon(kit::CHECK, 14., bx.ink)))
}

/// A dropdown's list, under its pill.
fn menu(bx: &Bx, rows: Vec<AnyElement>) -> Div {
    Raised {
        face: bx.cream,
        edge: bx.deep,
        radius: 14.,
        border: 2.5,
        drop: 5.,
    }
    .wrap(div().flex().flex_col().p(u(6.)).children(rows))
    .absolute()
    .left_0()
    .top(u(50.))
    .min_w(u(230.))
    .shadow(kit::soft_shadow(bx, 14., 24., 0.3))
    .occlude()
}

/// A line between two groups of a list.
fn divider(bx: &Bx) -> AnyElement {
    div()
        .flex_none()
        .h(u(2.))
        .my(u(5.))
        .mx(u(8.))
        .bg(alpha(bx.ink, 0.12))
        .into_any_element()
}

/// A pill that opens `which`'s list.
fn dropdown(
    s: &TableScreen,
    bx: &Bx,
    which: Menu,
    label: String,
    (value, rows): (String, Vec<AnyElement>),
    cx: &mut Context<TableScreen>,
) -> Div {
    let open = s.baralho.menu == Some(which);
    let id = match which {
        Menu::Spot => "spot_menu",
        Menu::Opponent => "opponent_menu",
    };
    div()
        .relative()
        .flex_none()
        .child(
            pill(bx, label, value, true)
                .id(id)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.baralho.menu = (this.baralho.menu != Some(which)).then_some(which);
                    cx.notify();
                })),
        )
        .when(open, |d| d.child(menu(bx, rows)))
}

/// The rows of the spot list: the kinds of preflop spot, or the postflop
/// configurations.
fn spot_rows(
    s: &TableScreen,
    t: &Trainers,
    bx: &Bx,
    cx: &mut Context<TableScreen>,
) -> (String, Vec<AnyElement>) {
    let ui = s.ui.clone();
    let i18n = &ui.i18n;
    let mut rows = Vec::new();
    if s.mode == Mode::Postflop {
        let any = i18n.tr("table-postflop-any");
        let mut value = any.clone();
        rows.push(
            menu_row(bx, "pf_any", any, s.postflop_spot.is_none())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.baralho.menu = None;
                    this.set_postflop_spot(None, cx);
                }))
                .into_any_element(),
        );
        for (i, spot) in t
            .post()
            .map(|pf| pf.spots())
            .into_iter()
            .flatten()
            .enumerate()
        {
            let id = spot.id.clone();
            let selected = s.postflop_spot.as_deref() == Some(id.as_str());
            let label = felt::spot_label(&ui, &id);
            if selected {
                value = label.clone();
            }
            rows.push(
                menu_row(bx, ("pf", i), label, selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.baralho.menu = None;
                        this.set_postflop_spot(Some(id.clone()), cx);
                    }))
                    .into_any_element(),
            );
        }
        return (value, rows);
    }
    let mut value = String::new();
    for (i, kind) in t.preflop.available_kinds().into_iter().enumerate() {
        let label = i18n.tr(&format!("table-spot-kind-{}", kind.id()));
        let selected = s.kind == kind;
        if selected {
            value = label.clone();
        }
        rows.push(
            menu_row(bx, ("kind", i), label, selected)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.baralho.menu = None;
                    this.set_kind(kind, cx);
                }))
                .into_any_element(),
        );
    }
    (value, rows)
}

/// The rows of the opponent list (spec §9, D8): the reference, then each
/// profile; in full hands also one profile per seat (#29).
fn opponent_rows(
    s: &TableScreen,
    t: &Trainers,
    bx: &Bx,
    cx: &mut Context<TableScreen>,
) -> (String, Vec<AnyElement>) {
    let i18n = &s.ui.i18n;
    let name = |id: Option<&str>| i18n.tr(&format!("bot-profile-{}", id.unwrap_or("reference")));
    let per_seat = s.mode == Mode::Hand && s.seats.is_some();
    let mut rows = Vec::new();
    if !per_seat {
        let ids = std::iter::once(None).chain(
            t.preflop
                .profiles()
                .iter()
                .map(|p| Some(p.bot.profile().id.clone())),
        );
        for (i, id) in ids.enumerate() {
            let selected = s.opponent == id;
            rows.push(
                menu_row(bx, ("opp", i), name(id.as_deref()), selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.baralho.menu = None;
                        this.set_opponent(id.clone(), cx);
                    }))
                    .into_any_element(),
            );
        }
    }
    if s.mode == Mode::Hand {
        if !rows.is_empty() {
            rows.push(divider(bx));
        }
        for (id, key, on) in [
            ("seats_same", "table-seats-same", false),
            ("seats_each", "table-seats-each", true),
        ] {
            rows.push(
                menu_row(bx, id, i18n.tr(key), per_seat == on)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_per_seat(on, cx)))
                    .into_any_element(),
            );
        }
    }
    if per_seat {
        rows.push(divider(bx));
        rows.push(
            typo::caps(11., 700, 100)
                .on(div())
                .px(u(12.))
                .py(u(4.))
                .text_color(bx.mut_ink)
                .child(i18n.tr("table-seats-from-left").to_uppercase())
                .into_any_element(),
        );
        // A click moves the seat to the next profile.
        for (i, bot) in s.seats.iter().flatten().enumerate() {
            let label = format!(
                "{} {}",
                i18n.tr_args("table-seat-left", &[("n", (i + 1).into())]),
                name(bot.as_deref())
            );
            rows.push(
                menu_row(bx, ("seat", i), label, false)
                    .on_click(cx.listener(move |this, _, _, cx| this.cycle_seat(i, cx)))
                    .into_any_element(),
            );
        }
    }
    let value = if per_seat {
        i18n.tr("table-seats-each")
    } else {
        name(s.opponent.as_deref())
    };
    (value, rows)
}

/// The strip of controls: the mode, the spot, the opponent and, at the
/// right, the layout editor.
fn controls(s: &TableScreen, t: &Trainers, cx: &mut Context<TableScreen>) -> Div {
    let ui = s.ui.clone();
    let bx = bx(&ui);
    let i18n = &ui.i18n;
    let mut modes = div()
        .flex()
        .flex_none()
        .p(u(4.))
        .rounded_full()
        .bg(alpha(bx.well, 0.5))
        .border(u(2.))
        .border_color(bx.deep)
        .child(segment(
            s,
            &bx,
            "mode_spot",
            "table-mode-spot",
            Mode::Spot,
            cx,
        ))
        .child(segment(
            s,
            &bx,
            "mode_postflop",
            "table-mode-postflop",
            Mode::Postflop,
            cx,
        ))
        .child(segment(
            s,
            &bx,
            "mode_hand",
            "table-mode-hand",
            Mode::Hand,
            cx,
        ));
    if s.drill.is_some() {
        modes = modes.child(segment(
            s,
            &bx,
            "mode_drill",
            "table-mode-drill",
            Mode::Drill,
            cx,
        ));
    }
    // It takes the row's free width: sized by its content, rounding at
    // some scales wraps the last pill.
    let mut left = div()
        .relative()
        .flex()
        .flex_1()
        .min_w_0()
        .flex_wrap()
        .items_center()
        .gap(u(12.))
        .child(crate::tour::anchor("table_controls"))
        .child(modes);
    match s.mode {
        Mode::Spot | Mode::Postflop => {
            let rows = spot_rows(s, t, &bx, cx);
            left = left.child(dropdown(
                s,
                &bx,
                Menu::Spot,
                i18n.tr("table-row-spot"),
                rows,
                cx,
            ));
        }
        // A drill's spot is the leak being trained.
        Mode::Drill => {
            if let Some(b) = &s.drill {
                left = left.child(pill(
                    &bx,
                    i18n.tr("table-row-spot"),
                    TableScreen::bucket_label(&ui, b),
                    false,
                ));
            }
        }
        Mode::Hand => {}
    }
    if s.mode != Mode::Drill {
        let rows = opponent_rows(s, t, &bx, cx);
        left = left.child(dropdown(
            s,
            &bx,
            Menu::Opponent,
            i18n.tr("table-opponent"),
            rows,
            cx,
        ));
    }
    let edit = (!s.is_editing()).then(|| {
        kit::btn(
            "layout_edit",
            i18n.tr("layout-edit-start"),
            Fill::Cream,
            true,
            &bx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.baralho.menu = None;
            this.start_edit(cx);
        }))
    });
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(u(12.))
        .child(left)
        .children(edit)
}

/// The session under the buttons: how many decisions, the last grades and
/// the EV lost.
fn session_strip(s: &TableScreen, bx: &Bx) -> Div {
    let (ui, p) = (&s.ui, s.palette);
    let session = &s.baralho.session;
    let lost = session.iter().fold(0., |sum, (_, loss)| sum + loss);
    let from = session.len().saturating_sub(DOTS);
    typo::text(13.5, 700)
        .on(div())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(u(14.))
        .text_color(bx.on_felt_mut)
        .child(
            ui.i18n
                .tr_args("baralho-session", &[("count", session.len().into())]),
        )
        .when(!session.is_empty(), |d| {
            d.child(
                div().flex().gap(u(5.)).children(
                    session[from..]
                        .iter()
                        .map(|(grade, _)| kit::dot(bx, p.grade(*grade))),
                ),
            )
        })
        .child(ui.i18n.tr_args(
            "baralho-session-loss",
            &[("amount", felt::num(ui, lost, 1).into())],
        ))
}

/// The screen's `ui` with the theme's sizes scaled to the window.
fn scaled_ui(s: &mut TableScreen) -> Rc<Ui> {
    super::chrome::scaled(&mut s.baralho.scaled, &s.ui, scale())
}

pub fn view(
    s: &mut TableScreen,
    _window: &mut Window,
    cx: &mut Context<TableScreen>,
) -> AnyElement {
    let ui = s.ui.clone();
    let bx = bx(&ui);
    let k = scale();
    let trainers = match &s.trainers {
        Ok(t) => t.clone(),
        Err(e) => {
            let text = ui
                .i18n
                .tr_args("table-chart-error", &[("error", e.clone().into())]);
            return div()
                .size_full()
                .child(
                    TableScreen::place(mock_rect(PanelId::Table))
                        .child(tabletop::empty(&bx, Some(text))),
                )
                .into_any_element();
        }
    };
    count_session(s);
    // A list closes from its pill or from the layer under it: forget one
    // whose pill this mode does not draw (a drill started from the home),
    // or that layer would take a click.
    let drawn = match s.baralho.menu {
        Some(Menu::Spot) => matches!(s.mode, Mode::Spot | Mode::Postflop),
        Some(Menu::Opponent) => s.mode != Mode::Drill,
        None => true,
    };
    if !drawn {
        s.baralho.menu = None;
    }

    // Where the panels go: the mockup's places, or the layout's grid.
    let editing = s.is_editing();
    let layout = s.shown_layout();
    let mock = !editing && is_default(&layout);
    let bar = if editing {
        let plain = s.ui.clone();
        s.ui = scaled_ui(s);
        let width = (CANVAS_W - 2. * GRID_MARGIN - 44.) * k;
        let bar = s.edit_bar(width, cx);
        s.ui = plain;
        Some(
            Raised {
                face: bx.cream,
                edge: bx.deep,
                radius: 16.,
                border: 2.5,
                drop: 5.,
            }
            .wrap(div().px(u(19.5)).child(bar))
            .absolute()
            .left(u(GRID_MARGIN))
            .top(u(GRID_TOP)),
        )
    } else {
        None
    };
    // The edit bar is as tall as the shared code makes it, in window pixels.
    let bar_h = if editing {
        crate::screens::table::edit::EDIT_BAR + (2. * 2.5 + 5. + 10.) * k
    } else {
        0.
    };
    let (origin, cell) = if mock {
        ((0., 0.), 0.)
    } else {
        let top = GRID_TOP * k + bar_h;
        let cell = layout.cell_size((CANVAS_W - 2. * GRID_MARGIN) * k, GRID_BOTTOM * k - top, 4.);
        s.cell = cell;
        let width = cell * f32::from(layout.grid.cols);
        (((CANVAS_W * k - width) / 2., top), cell)
    };
    let inset = if mock { 0. } else { INSET };
    let rect = |s: &TableScreen, id: PanelId| {
        if mock {
            Some(mock_rect(id))
        } else {
            s.panel_rect(&layout, id, cell)
        }
    };
    // A panel's room for its content, in design pixels.
    let room = |r: Rect| (r.w / k - 2. * inset, r.h / k - 2. * inset);

    let fit = rect(s, PanelId::Table).map_or((tabletop::W, tabletop::H), room);
    let scene = match s.mode {
        Mode::Hand => hand_scene(s, &trainers, fit, cx),
        _ => question_scene(s, &trainers, fit, cx),
    };
    let Scene {
        table,
        actions,
        card,
        hero,
        answered,
    } = scene;

    let mut grid = div().absolute().left(px(origin.0)).top(px(origin.1));
    grid = if mock {
        grid.size_full()
    } else {
        grid.w(px(cell * f32::from(layout.grid.cols)))
            .h(px(cell * f32::from(layout.grid.rows)))
    };
    grid = s.edit_grid(grid, &layout, cell, cx);

    let mut on_top = None;
    let mut add =
        |s: &TableScreen, grid: Div, id: PanelId, el: Div, cx: &mut Context<TableScreen>| {
            let el = s.edit_chrome(id, el, cx);
            if s.dragged_panel() == Some(id) {
                on_top = Some(el);
                grid
            } else {
                grid.child(el)
            }
        };

    if let Some(r) = rect(s, PanelId::Table) {
        let el = TableScreen::place(r)
            .flex()
            .items_center()
            .justify_center()
            .child(table)
            .child(crate::tour::anchor("table_felt"));
        grid = add(s, grid, PanelId::Table, el, cx);
    }
    if let Some(r) = rect(s, PanelId::Actions) {
        let el = TableScreen::place(r)
            .flex()
            .flex_col()
            .items_center()
            .pt(u(11. + inset))
            .px(u(inset))
            .child(
                div()
                    .relative()
                    .min_h(u(46.))
                    .child(actions)
                    .child(crate::tour::anchor("table_actions")),
            )
            .child(session_strip(s, &bx).mt(u(33.)));
        grid = add(s, grid, PanelId::Actions, el, cx);
    }
    if let Some(r) = rect(s, PanelId::Explanation) {
        let (w, h) = room(r);
        let sheet = card.map(|(head, body)| {
            let body = body.into_iter().chain(
                hero.map(|h| story(s, h, answered.as_ref(), cx))
                    .unwrap_or_default(),
            );
            kit::sheet(
                &bx,
                div()
                    .flex()
                    .flex_col()
                    .pt(u(26.))
                    .px(u(28.))
                    .pb(u(24.))
                    .text_color(bx.ink)
                    .child(head)
                    .child(
                        div()
                            .id("explanation")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .children(body),
                    )
                    .children(replay_controls(s, cx)),
            )
            .w(u(w))
            // The grid's panels keep room for the sheet's drop.
            .h(u(if mock { h + 7. } else { h }))
        });
        let el = TableScreen::place(r)
            .p(u(inset))
            .children(sheet)
            .child(crate::tour::anchor("table_explanation"));
        grid = add(s, grid, PanelId::Explanation, el, cx);
    }
    // An open list closes on a click anywhere else; the controls come last
    // so the list covers the other panels.
    if s.baralho.menu.is_some() {
        grid = grid.child(
            div()
                .id("menu_away")
                .absolute()
                .left(px(-origin.0))
                .top(px(-origin.1))
                .w(u(CANVAS_W))
                .h(u(super::CANVAS_H))
                .occlude()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.baralho.menu = None;
                    cx.notify();
                })),
        );
    }
    if let Some(r) = rect(s, PanelId::Controls) {
        let el = TableScreen::place(r)
            .p(u(inset))
            .child(controls(s, &trainers, cx));
        grid = add(s, grid, PanelId::Controls, el, cx);
    }
    let grid = grid.children(on_top).children(s.landing_outline(cell));
    div()
        .size_full()
        .relative()
        .children(bar)
        .child(grid)
        .into_any_element()
}
