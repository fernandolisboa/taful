//! Cockpit's home: a dashboard. Four numbers at the top (reviews, accuracy,
//! EV lost, mental math), decisions by grade over two weeks, the biggest
//! leaks, lesson tracks, the preflop deviation map and shortcuts.

use gpui::{
    AnyElement, Context, Div, FontWeight, SharedString, div, linear_color_stop, linear_gradient,
    prelude::*, px,
};
use taful_app::dashboard::{Dashboard, HandClass, Trend};
use taful_app::model::{Grade, Leak, LessonState, Street};
use taful_app::template::{street_id, texture_label_keys};

use super::kit::{self, Btn, GAP, HEAD, MONO, STRONG, btn, figure, head, icon, kbd, label, mono};
use super::pal::Pal;
use super::state::Lessons;
use super::{Go, HOME_KEYS, go};
use crate::Ui;
use crate::felt::num;
use crate::screen::Screen;
use crate::shell::Shell;
use crate::theme::alpha;

// The design's sizes at 1600 × 900; widths stretch with the window.
const KPI_H: f32 = 104.;
const MID_H: f32 = 326.;
const LOW_H: f32 = 370.;
/// Chart to leaks, and tracks to shortcuts, as designed.
const CHART_SHARE: f32 = 884. / 1484.;
const TRACKS_SHARE: f32 = 470. / 952.;
const DEVIATION_W: f32 = 520.;
/// A cell of the deviation map.
const CELL: f32 = 23.;

/// The home screen's body.
pub fn home(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    let ui = shell.ui.clone();
    let pal = Pal::new(&ui);
    let (w, h) = shell.cockpit.room;
    let (Some(d), Some(lessons)) = (shell.cockpit.dashboard(), shell.cockpit.lessons()) else {
        return div()
            .text_color(pal.muted)
            .child(ui.i18n.tr("cockpit-kpi-empty"))
            .into_any_element();
    };
    let row = || div().flex().flex_none().gap(px(GAP));
    let page = div().flex().flex_col().gap(px(GAP)).w(px(w));
    if w < super::frame::WIDE_ROOM {
        // A window narrower than designed: the same panels, restacked. Two
        // numbers a row, the chart and the leaks each on the whole width,
        // and the deviation map under the tracks and the shortcuts.
        let half = ((w - GAP) / 2.).floor();
        return page
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(GAP))
                    .child(crate::tour::anchor("home_stats"))
                    .child(
                        row()
                            .h(px(KPI_H))
                            .child(reviews(&ui, &pal, &d, half, cx))
                            .child(accuracy(&ui, &pal, &d.accuracy, w - GAP - half)),
                    )
                    .child(
                        row()
                            .h(px(KPI_H))
                            .child(ev_loss(&ui, &pal, &d.ev_loss, half))
                            .child(calc(&ui, &pal, &d, w - GAP - half)),
                    ),
            )
            .child(row().h(px(MID_H)).child(chart(&ui, &pal, &d, w)))
            .child(row().h(px(MID_H)).child(leaks(&ui, &pal, &d.leaks, w, cx)))
            .child(
                row()
                    .h(px(LOW_H))
                    .child(tracks(&ui, &pal, &lessons, half, cx))
                    .child(shortcuts(&ui, &pal, &d, w - GAP - half, cx)),
            )
            .child(row().child(deviation(&ui, &pal, &d, w)))
            .into_any_element();
    }
    let kpi_w = (w - 3. * GAP) / 4.;
    let chart_w = ((w - GAP) * CHART_SHARE).round();
    let leaks_w = w - GAP - chart_w;
    let low_h = LOW_H.max(h - KPI_H - MID_H - 2. * GAP);
    let tracks_w = ((w - 2. * GAP - DEVIATION_W) * TRACKS_SHARE).round();
    let shortcuts_w = w - 2. * GAP - DEVIATION_W - tracks_w;
    page.child(
        row()
            .relative()
            .h(px(KPI_H))
            .child(crate::tour::anchor("home_stats"))
            .child(reviews(&ui, &pal, &d, kpi_w, cx))
            .child(accuracy(&ui, &pal, &d.accuracy, kpi_w))
            .child(ev_loss(&ui, &pal, &d.ev_loss, kpi_w))
            .child(calc(&ui, &pal, &d, kpi_w)),
    )
    .child(
        row()
            .h(px(MID_H))
            .child(chart(&ui, &pal, &d, chart_w))
            .child(leaks(&ui, &pal, &d.leaks, leaks_w, cx)),
    )
    .child(
        row()
            .h(px(low_h))
            .child(tracks(&ui, &pal, &lessons, tracks_w, cx))
            .child(deviation(&ui, &pal, &d, DEVIATION_W))
            .child(shortcuts(&ui, &pal, &d, shortcuts_w, cx)),
    )
    .into_any_element()
}

/// One of the four panels at the top: caption, big number, detail line.
fn kpi(pal: &Pal, w: f32, caption: &str, value: Div, detail: Div) -> Div {
    kit::panel(pal)
        .relative()
        .w(px(w))
        .h_full()
        .px(px(14.))
        .py(px(12.))
        .child(label(caption, pal))
        .child(value.mt(px(10.)))
        .child(
            detail
                .mt(px(9.))
                .font_family(MONO)
                .text_size(px(11.5))
                .text_color(pal.muted)
                .whitespace_nowrap(),
        )
}

/// A sparkline's place in a KPI panel.
fn spark(line: impl IntoElement) -> Div {
    div()
        .absolute()
        .right(px(16.))
        .bottom(px(30.))
        .w(px(140.))
        .h(px(48.))
        .child(line)
}

fn dash(ui: &Ui, pal: &Pal) -> Div {
    figure(&ui.i18n.tr("cockpit-none"), None, 32., pal).text_color(pal.muted)
}

fn reviews(ui: &Ui, pal: &Pal, d: &Dashboard, w: f32, cx: &mut Context<Shell>) -> Div {
    let (i18n, r) = (&ui.i18n, &d.reviews);
    let most = r.forecast.iter().copied().max().unwrap_or(0).max(1);
    let bars = r
        .forecast
        .iter()
        .zip(&r.days)
        .enumerate()
        .map(|(i, (n, day))| {
            let letter = if i == 0 {
                i18n.tr("cockpit-day-today")
            } else {
                i18n.tr(&format!("cockpit-day-{}", day.weekday))
            };
            // An empty day keeps a stub, so the week still reads as seven days.
            let height = (46. * *n as f32 / most as f32).round().max(2.);
            div()
                .relative()
                .w(px(13.))
                .h(px(height))
                .rounded_t(px(2.))
                .bg(if i == 0 { pal.amber } else { pal.forecast })
                .child(
                    div()
                        .absolute()
                        .left(px(-4.))
                        .right(px(-4.))
                        .bottom(px(-14.))
                        .flex()
                        .justify_center()
                        .font_family(MONO)
                        .text_size(px(8.5))
                        .line_height(px(12.))
                        .text_color(pal.dim)
                        .child(letter),
                )
        });
    kpi(
        pal,
        w,
        &i18n.tr("cockpit-kpi-reviews"),
        figure(
            &r.due_now.to_string(),
            (r.due_now > 0).then(|| {
                i18n.tr_args(
                    "cockpit-kpi-reviews-minutes",
                    &[("minutes", r.minutes.into())],
                )
            }),
            32.,
            pal,
        ),
        div().child(i18n.tr_args(
            "cockpit-kpi-reviews-ahead",
            &[("tomorrow", r.tomorrow.into()), ("week", r.week.into())],
        )),
    )
    .child(
        div().absolute().right(px(12.)).top(px(10.)).child(
            btn(
                "cockpit_review",
                i18n.tr("cockpit-review-go"),
                Some("R"),
                Btn::SmallAmber,
                pal,
            )
            .on_click(cx.listener(|this, _, window, cx| {
                go(this, Go::Screen(Screen::Review), window, cx);
            })),
        ),
    )
    .child(
        div()
            .absolute()
            .right(px(14.))
            .bottom(px(26.))
            .flex()
            .items_end()
            .gap(px(5.))
            .h(px(46.))
            .children(bars),
    )
}

/// "▲ 4 pts vs. semana anterior": the change, colored by whether it is
/// good news, then what it is compared with.
fn change(ui: &Ui, pal: &Pal, delta: Option<f64>, amount: String, good_when_up: bool) -> Div {
    let Some(delta) = delta else {
        return div().child(ui.i18n.tr("cockpit-kpi-no-trend"));
    };
    let up = delta >= 0.;
    let color = if up == good_when_up {
        pal.grades[0]
    } else {
        pal.grades[3]
    };
    div()
        .flex()
        .gap(px(7.))
        .child(
            div()
                .text_color(color)
                .child(format!("{} {amount}", if up { "▲" } else { "▼" })),
        )
        .child(ui.i18n.tr("cockpit-kpi-vs-last-week"))
}

fn accuracy(ui: &Ui, pal: &Pal, t: &Trend, w: f32) -> Div {
    let i18n = &ui.i18n;
    let value = match t.value {
        Some(v) => figure(
            &super::frame::percent(v),
            Some(i18n.tr("cockpit-unit-percent")),
            32.,
            pal,
        ),
        None => dash(ui, pal),
    };
    let points = t.delta.map(|d| format!("{:.0}", (d * 100.).abs()));
    let detail = match t.value {
        Some(_) => change(
            ui,
            pal,
            t.delta,
            i18n.tr_args(
                "cockpit-kpi-points",
                &[("points", points.unwrap_or_default().into())],
            ),
            true,
        ),
        None => div().child(i18n.tr("cockpit-kpi-empty")),
    };
    let green = pal.grades[0];
    kpi(pal, w, &i18n.tr("cockpit-kpi-accuracy"), value, detail).child(spark(kit::sparkline(
        t.series.clone(),
        None,
        green,
        green,
        Some(alpha(green, 0.12)),
        pal.line2,
    )))
}

fn ev_loss(ui: &Ui, pal: &Pal, t: &Trend, w: f32) -> Div {
    let i18n = &ui.i18n;
    let value = match t.value {
        Some(v) => figure(&num(ui, v, 2), Some(i18n.tr("cockpit-unit-bb")), 32., pal),
        None => dash(ui, pal),
    };
    let detail = match t.value {
        Some(_) => change(
            ui,
            pal,
            t.delta,
            num(ui, t.delta.unwrap_or(0.).abs(), 2),
            false,
        ),
        None => div().child(i18n.tr("cockpit-kpi-empty")),
    };
    kpi(pal, w, &i18n.tr("cockpit-kpi-ev"), value, detail).child(spark(kit::sparkline(
        t.series.clone(),
        None,
        pal.amber,
        pal.amber,
        Some(pal.amber_at(0.1)),
        pal.line2,
    )))
}

fn calc(ui: &Ui, pal: &Pal, d: &Dashboard, w: f32) -> Div {
    let (i18n, c) = (&ui.i18n, &d.calc);
    let value = match c.mean_secs {
        Some(s) => figure(
            &num(ui, s, 1),
            Some(i18n.tr("cockpit-unit-seconds")),
            32.,
            pal,
        ),
        None => dash(ui, pal),
    };
    let detail = match (c.accuracy, c.target_secs) {
        (Some(a), Some(target)) => i18n.tr_args(
            "cockpit-kpi-calc-detail",
            &[
                ("accuracy", super::frame::percent(a).into()),
                (
                    "target",
                    taful_app::fmt::number_trim(target, 1, i18n.locale()).into(),
                ),
            ],
        ),
        _ => i18n.tr("cockpit-kpi-empty"),
    };
    kpi(
        pal,
        w,
        &i18n.tr("cockpit-kpi-calc"),
        value,
        div().child(detail),
    )
    .child(spark(kit::sparkline(
        c.series.iter().copied().map(Some).collect(),
        c.target_secs,
        pal.muted,
        pal.text,
        None,
        pal.line2,
    )))
}

/// The top of the chart's scale: four even steps that hold `most`.
fn scale_top(most: u32) -> u32 {
    const STEPS: [u32; 12] = [10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000];
    let step = STEPS
        .into_iter()
        .find(|s| s * 4 >= most)
        .unwrap_or(most.div_ceil(4));
    step * 4
}

/// Decisions per day, stacked by grade.
fn chart(ui: &Ui, pal: &Pal, d: &Dashboard, w: f32) -> Div {
    let i18n = &ui.i18n;
    // The plot's box inside the panel, and its margins.
    let (plot_w, plot_h) = (w - 2., MID_H - HEAD - 2.);
    let (left, right, top, bottom) = (44., 16., 18., 30.);
    let top_value = scale_top(d.days.iter().map(|x| x.total()).max().unwrap_or(0));
    let inner_h = plot_h - top - bottom;
    let slot = (plot_w - left - right) / d.days.len().max(1) as f32;
    let small = |size: f32| {
        div()
            .absolute()
            .font_family(MONO)
            .text_size(px(size))
            .line_height(px(14.))
    };
    let mut plot = div().relative().w(px(plot_w)).h(px(plot_h));
    for i in 0..=4u32 {
        let value = top_value * i / 4;
        let y = top + inner_h - inner_h * i as f32 / 4.;
        plot = plot
            .child(
                div()
                    .absolute()
                    .left(px(left))
                    .right(px(right))
                    .top(px(y.round()))
                    .h(px(1.))
                    .bg(pal.line),
            )
            .child(
                small(10.5)
                    .left(px(0.))
                    .w(px(left - 8.))
                    .top(px(y - 7.))
                    .flex()
                    .justify_end()
                    .text_color(pal.dim)
                    .child(value.to_string()),
            );
    }
    let today = d.days.len().saturating_sub(1);
    for (i, day) in d.days.iter().enumerate() {
        let x = left + i as f32 * slot;
        let (bar_x, bar_w) = (x + slot * 0.2, slot * 0.6);
        let mut y = top + inner_h;
        for (k, n) in day.counts.iter().enumerate() {
            let height = *n as f32 / top_value as f32 * inner_h;
            if height <= 0. {
                continue;
            }
            y -= height;
            plot = plot.child(
                div()
                    .absolute()
                    .left(px(bar_x))
                    .top(px(y + 1.))
                    .w(px(bar_w))
                    .h(px((height - 1.).max(1.)))
                    .rounded(px(1.))
                    .bg(pal.grades[k]),
            );
        }
        let centered = |el: Div| el.left(px(x)).w(px(slot)).flex().justify_center();
        plot = plot.child(
            centered(small(10.5))
                .top(px(plot_h - 21.))
                .text_color(if i == today { pal.amber } else { pal.dim })
                .child(format!("{:02}", day.day.day)),
        );
        if day.total() > 0 {
            plot = plot.child(
                centered(small(10.))
                    .top(px(y - 16.))
                    .text_color(pal.muted)
                    .child(day.total().to_string()),
            );
        }
    }
    if d.days.iter().all(|x| x.total() == 0) {
        plot = plot.child(
            div()
                .absolute()
                .left(px(left))
                .right(px(right))
                .top(px(top))
                .h(px(inner_h))
                .flex()
                .items_center()
                .justify_center()
                .text_color(pal.muted)
                .child(i18n.tr("cockpit-chart-empty")),
        );
    }
    let legend = div()
        .flex()
        .gap(px(12.))
        .children(Grade::ALL.iter().map(|g| {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .child(div().size(px(8.)).rounded(px(2.)).bg(pal.grade(*g)))
                .child(i18n.tr(&crate::felt::grade_key(*g)))
        }));
    kit::panel(pal)
        .w(px(w))
        .h_full()
        .child(head(
            &i18n.tr("cockpit-chart-title"),
            Some(legend.into_any_element()),
            true,
            pal,
        ))
        .child(plot)
}

/// A leak as the table names it: where it is, then what kind of decision.
fn leak_names(ui: &Ui, l: &Leak) -> (String, String) {
    let (i18n, b) = (&ui.i18n, &l.bucket);
    // A preflop leak's spot is already called "Preflop".
    let mut place = crate::felt::spot_label(ui, &b.spot);
    if b.street != Street::Preflop {
        place = format!(
            "{place} · {}",
            i18n.tr(&format!("street-{}", street_id(b.street)))
        );
    }
    let mut kind = vec![i18n.tr(&format!("node-type-{}", b.node_type.id()))];
    if let Some(keys) = b.texture.as_deref().and_then(texture_label_keys) {
        kind.push(keys.map(|k| i18n.tr(&k)).join(", "));
    }
    if let Some(profile) = b.opponent.as_deref() {
        kind.push(i18n.tr_args(
            "leak-vs-profile",
            &[("profile", i18n.opponent(profile).into())],
        ));
    }
    (place, kind.join(" · "))
}

fn leaks(ui: &Ui, pal: &Pal, list: &[Leak], w: f32, cx: &mut Context<Shell>) -> Div {
    let i18n = &ui.i18n;
    // Columns after the spot: mistakes, bb lost, the button.
    let (errors_w, lost_w, action_w) = (62., 176., 88.);
    let cell = |width: f32| div().flex_none().w(px(width)).px(px(12.));
    let th = |text: String| {
        kit::tracked(&text.to_uppercase(), 10., 0.08)
            .font_weight(FontWeight::MEDIUM)
            .text_color(pal.dim)
    };
    let header = div()
        .flex()
        .flex_none()
        .items_center()
        .pt(px(8.))
        .pb(px(6.))
        .border_b_1()
        .border_color(pal.line)
        .child(
            div()
                .flex_1()
                .px(px(12.))
                .child(th(i18n.tr("cockpit-leaks-spot"))),
        )
        .child(cell(errors_w).child(th(i18n.tr("cockpit-leaks-mistakes"))))
        .child(cell(lost_w).child(th(i18n.tr("cockpit-leaks-bb"))))
        .child(cell(action_w));
    let worst = list
        .iter()
        .map(|l| l.total_ev_loss_bb)
        .fold(0., f64::max)
        .max(f64::MIN_POSITIVE);
    let last = list.len().saturating_sub(1);
    let rows = list.iter().enumerate().map(|(i, l)| {
        let (place, kind) = leak_names(ui, l);
        let bucket = l.bucket.clone();
        let share = (l.total_ev_loss_bb / worst) as f32;
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(51.))
            .when(i < last, |d| d.border_b_1().border_color(pal.line))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .px(px(12.))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(place),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(pal.muted)
                            .truncate()
                            .child(kind),
                    ),
            )
            .child(
                cell(errors_w)
                    .font_family(MONO)
                    .text_size(px(12.5))
                    .whitespace_nowrap()
                    .child(format!("{}/{}", l.mistakes, l.decisions)),
            )
            .child(
                cell(lost_w)
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .font_family(MONO)
                    .text_size(px(12.5))
                    .child(
                        div()
                            .flex_none()
                            .w(px(110.))
                            .h(px(6.))
                            .rounded(px(1.))
                            .overflow_hidden()
                            .bg(pal.raised)
                            .child(div().h_full().w(px(110. * share)).bg(pal.grades[3])),
                    )
                    .child(num(ui, l.total_ev_loss_bb, 1)),
            )
            .child(
                cell(action_w).child(
                    btn(
                        ("cockpit_drill", i),
                        i18n.tr("cockpit-leaks-train"),
                        None,
                        Btn::Small,
                        pal,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let b = bucket.clone();
                        this.table.update(cx, |t, cx| t.start_drill(b, cx));
                        this.select(Screen::Table, window, cx);
                    })),
                ),
            )
    });
    let rows: Vec<_> = rows.collect();
    kit::panel(pal)
        .w(px(w))
        .h_full()
        .child(head(
            &i18n.tr("cockpit-leaks-title"),
            Some(div().child(i18n.tr("cockpit-leaks-by")).into_any_element()),
            true,
            pal,
        ))
        .child(header)
        .when(rows.is_empty(), |d| {
            d.child(
                div()
                    .p(px(14.))
                    .text_color(pal.muted)
                    .child(i18n.tr("home-leaks-body")),
            )
        })
        .children(rows)
}

fn tracks(ui: &Ui, pal: &Pal, lessons: &Lessons, w: f32, cx: &mut Context<Shell>) -> Div {
    let (i18n, s) = (&ui.i18n, &lessons.summary);
    let next = match &s.next {
        Some(n) => div()
            .flex()
            .items_center()
            .gap(px(14.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .child(label(&i18n.tr("cockpit-next-lesson"), pal).text_color(pal.amber))
                    .child(
                        div()
                            .mt(px(3.))
                            .text_size(px(21.))
                            .font_weight(STRONG)
                            .truncate()
                            .child(n.title.clone()),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .text_size(px(12.))
                            .text_color(pal.muted)
                            .truncate()
                            .child(next_meta(ui, n)),
                    ),
            )
            .child(
                btn(
                    "cockpit_continue",
                    i18n.tr("cockpit-continue"),
                    Some("↵"),
                    Btn::Amber,
                    pal,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    go(this, Go::NextLesson, window, cx);
                })),
            ),
        None => div()
            .flex()
            .items_center()
            .text_color(pal.muted)
            .child(i18n.tr("home-lesson-none")),
    };
    let levels = &lessons.summary.levels;
    let last = levels.len().saturating_sub(1);
    let rows = levels.iter().enumerate().map(|(i, track)| {
        let total = track.lessons.len();
        let done = track
            .lessons
            .iter()
            .filter(|l| l.state == LessonState::Done)
            .count();
        let open = track.lessons.iter().any(|l| l.state != LessonState::Locked);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(12.))
            .h(px(47.))
            .px(px(14.))
            .when(i < last, |d| d.border_b_1().border_color(pal.line))
            .child(
                div()
                    .flex_none()
                    .w(px(150.))
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .when(!open, |d| d.text_color(pal.dim))
                    .child(i18n.tr(&track.title_key())),
            )
            .child(
                div().flex_1().flex().gap(px(3.)).children(
                    kit::steps(&track.lessons, s.next.as_ref().map(|n| n.id.as_str()), pal)
                        .into_iter()
                        .map(|b| b.flex_1().h(px(10.)).rounded(px(1.5))),
                ),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(44.))
                    .flex()
                    .justify_end()
                    .font_family(MONO)
                    .text_size(px(12.))
                    .text_color(pal.muted)
                    .child(format!("{done}/{total}")),
            )
    });
    kit::panel(pal)
        .w(px(w))
        .h_full()
        .child(head(
            &i18n.tr("cockpit-tracks-title"),
            Some(mono(format!("{}/{}", s.done, s.total)).into_any_element()),
            true,
            pal,
        ))
        .child(
            next.relative()
                .flex_none()
                .h(px(92.))
                .p(px(14.))
                .border_b_1()
                .border_color(pal.line)
                .bg(linear_gradient(
                    90.,
                    linear_color_stop(pal.amber_at(0.09), 0.),
                    linear_color_stop(pal.amber_at(0.), 0.7),
                ))
                .child(crate::tour::anchor("home_continue")),
        )
        .children(rows)
}

/// "Matemática do poker · 4 de 10 · 7 min".
pub(super) fn next_meta(ui: &Ui, n: &taful_app::dashboard::NextLesson) -> String {
    let i18n = &ui.i18n;
    i18n.tr_args(
        "cockpit-next-meta",
        &[
            ("track", i18n.tr(&format!("level-{}", n.level)).into()),
            ("n", n.number.into()),
            ("total", n.level_len.into()),
            ("minutes", n.minutes.unwrap_or(0).into()),
        ],
    )
}

/// The 13 × 13 map of where preflop play leaves the reference.
fn deviation(ui: &Ui, pal: &Pal, d: &Dashboard, w: f32) -> Div {
    let (i18n, dev) = (&ui.i18n, &d.deviation);
    let mut grid = div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(1.))
        .border_1()
        .border_color(pal.line)
        .bg(pal.line);
    for r in 0..13 {
        let mut line = div().flex().gap(px(1.));
        for c in 0..13 {
            let class = HandClass::from_grid_pos(r, c);
            let level = dev.cells.get(class.index()).map_or(0, |x| x.level);
            let (bg, fg) = match level {
                0 => (pal.cell, pal.cell_out_text),
                1 => (pal.heat[0], pal.heat1_text),
                2 => (pal.heat[1], pal.cell_text),
                n => (pal.heat[usize::from(n.min(4)) - 1], pal.on_amber),
            };
            line = line.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(CELL))
                    .bg(bg)
                    .text_color(fg)
                    .font_family(MONO)
                    .font_weight(if level >= 4 {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_size(px(8.5))
                    .child(SharedString::from(class.to_string())),
            );
        }
        grid = grid.child(line);
    }
    let top = dev.top.iter().map(|t| {
        let play = i18n.tr(&format!("cockpit-play-{}", t.play.id()));
        let key = if t.too_much {
            "cockpit-dev-too-much"
        } else {
            "cockpit-dev-too-little"
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .h(px(36.))
            .border_b_1()
            .border_color(pal.line)
            .text_size(px(12.))
            .child(
                mono(t.class.to_string())
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(13.)),
            )
            .child(
                div()
                    .text_color(pal.muted)
                    .child(i18n.tr_args(key, &[("play", play.into())])),
            )
    });
    let top: Vec<_> = top.collect();
    let scale = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .mt(px(12.))
        .text_size(px(10.5))
        .text_color(pal.muted)
        .child(i18n.tr("cockpit-dev-same"))
        .children(pal.heat.map(|c| div().w(px(18.)).h(px(8.)).bg(c)))
        .child(i18n.tr("cockpit-dev-far"));
    kit::panel(pal)
        .w(px(w))
        .h_full()
        .child(head(
            &i18n.tr("cockpit-dev-title"),
            Some(
                div()
                    .child(i18n.tr_args("cockpit-dev-count", &[("count", dev.decisions.into())]))
                    .into_any_element(),
            ),
            true,
            pal,
        ))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(16.))
                .p(px(14.))
                .child(grid)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .child(label(&i18n.tr("cockpit-dev-top"), pal).mb(px(4.)))
                        .when(top.is_empty(), |d| {
                            d.child(
                                div()
                                    .py(px(8.))
                                    .text_size(px(12.))
                                    .text_color(pal.muted)
                                    .child(i18n.tr("cockpit-dev-none")),
                            )
                        })
                        .children(top)
                        .child(scale),
                ),
        )
}

fn shortcuts(ui: &Ui, pal: &Pal, d: &Dashboard, w: f32, cx: &mut Context<Shell>) -> Div {
    let i18n = &ui.i18n;
    let top_leak = d.leaks.first().map(|l| {
        let (place, _) = leak_names(ui, l);
        let kind = i18n.tr(&format!("node-type-{}", l.bucket.node_type.id()));
        format!("{place} · {kind}")
    });
    // Icon, name and description of each entry of `HOME_KEYS`.
    let entries: [(&str, String, String); 6] = [
        (
            "table",
            i18n.tr("cockpit-sc-spot"),
            i18n.tr("cockpit-sc-spot-body"),
        ),
        (
            "table",
            i18n.tr("cockpit-sc-postflop"),
            i18n.tr("cockpit-sc-postflop-body"),
        ),
        (
            "table",
            i18n.tr("cockpit-sc-hand"),
            i18n.tr("cockpit-sc-hand-body"),
        ),
        (
            "calc",
            i18n.tr("cockpit-sc-calc"),
            i18n.tr("cockpit-sc-calc-body"),
        ),
        (
            "hands",
            i18n.tr("cockpit-sc-hands"),
            i18n.tr("cockpit-sc-hands-body"),
        ),
        (
            "home",
            i18n.tr("cockpit-sc-leak"),
            top_leak
                .clone()
                .unwrap_or_else(|| i18n.tr("cockpit-sc-leak-none")),
        ),
    ];
    let last = entries.len() - 1;
    let hover = pal.panel2;
    let rows = entries.into_iter().zip(HOME_KEYS).enumerate().map(
        |(i, ((glyph, name, body), (key, to)))| {
            let active = to != Go::TopLeak || top_leak.is_some();
            div()
                .id(("cockpit_shortcut", i))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(10.))
                .h(px(55.))
                .px(px(14.))
                .when(i < last, |d| d.border_b_1().border_color(pal.line))
                .when(active, |d| {
                    d.cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            go(this, to, window, cx);
                        }))
                })
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(34.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(pal.line2)
                        .bg(pal.panel2)
                        .child(icon(glyph, 18., pal.amber)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(name),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(pal.muted)
                                .truncate()
                                .child(body),
                        ),
                )
                .child(kbd(&key.to_uppercase(), pal))
        },
    );
    let rows: Vec<_> = rows.collect();
    kit::panel(pal)
        .w(px(w))
        .h_full()
        .child(head(&i18n.tr("cockpit-shortcuts-title"), None, true, pal))
        .children(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_holds_the_tallest_day() {
        assert_eq!(scale_top(0), 40);
        assert_eq!(scale_top(37), 40);
        assert_eq!(scale_top(40), 40);
        assert_eq!(scale_top(41), 80);
        assert_eq!(scale_top(90), 100);
        assert_eq!(scale_top(30_000), 30_000);
    }
}
