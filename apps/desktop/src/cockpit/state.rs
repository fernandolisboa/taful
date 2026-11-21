//! What Cockpit keeps on the shell: the numbers its chrome and dashboard
//! show, read again only when the app wrote something (or after a while,
//! since due counts and "today" also move with the clock).

use std::rc::Rc;
use std::time::{Duration, Instant};

use taful_app::dashboard::{Dashboard, LessonsSummary, Today};
use taful_app::i18n::Locale;
use taful_app::lessons::ViewBy;

use crate::screen::Screen;
use crate::shell::Shell;

/// Numbers older than this are read again even without a write.
const MAX_AGE: Duration = Duration::from_secs(30);

/// The lessons grouped as the lessons screen shows them, with the totals
/// (the groups are `summary.list`).
pub struct Lessons {
    locale: Locale,
    view: ViewBy,
    pub summary: LessonsSummary,
}

/// Cockpit's slot on the shell.
#[derive(Default)]
pub struct ShellState {
    /// The app's data stamp the numbers below were read at, and when.
    read: Option<(u64, Instant)>,
    today: Option<Rc<Today>>,
    due: u32,
    dashboard: Option<Rc<Dashboard>>,
    lessons: Option<Rc<Lessons>>,
    /// Width and height a Cockpit screen has to lay itself out in, set by
    /// the frame before it asks for the screen.
    pub room: (f32, f32),
}

impl ShellState {
    pub fn today(&self) -> Option<Rc<Today>> {
        self.today.clone()
    }

    /// Reviews due now.
    pub fn due(&self) -> u32 {
        self.due
    }

    pub fn dashboard(&self) -> Option<Rc<Dashboard>> {
        self.dashboard.clone()
    }

    pub fn lessons(&self) -> Option<Rc<Lessons>> {
        self.lessons.clone()
    }
}

/// Brings the numbers the current screen shows up to date. Called once per
/// frame, before anything is drawn.
pub fn sync(shell: &mut Shell) {
    let stamp = shell.app.data_stamp();
    let locale = shell.ui.i18n.locale();
    let fresh = shell
        .cockpit
        .read
        .is_some_and(|(s, at)| s == stamp && at.elapsed() < MAX_AGE);
    if !fresh {
        shell.cockpit = ShellState::default();
    }
    let view = shell.view_by();
    if shell
        .cockpit
        .lessons
        .as_ref()
        .is_some_and(|l| l.locale != locale || l.view != view)
    {
        shell.cockpit.lessons = None;
    }
    let log =
        |what: &str, e: &dyn std::fmt::Display| eprintln!("taful: reading {what} failed: {e}");
    if shell.cockpit.today.is_none() {
        match shell.app.today_numbers() {
            Ok(t) => shell.cockpit.today = Some(Rc::new(t)),
            Err(e) => log("today's numbers", &e),
        }
        shell.cockpit.due = shell.app.due_count().unwrap_or(0);
    }
    if shell.screen == Screen::Home && shell.cockpit.dashboard.is_none() {
        match shell.app.dashboard() {
            Ok(d) => shell.cockpit.dashboard = Some(Rc::new(d)),
            Err(e) => log("the dashboard", &e),
        }
    }
    let wants_lessons = matches!(shell.screen, Screen::Home | Screen::SkillTree);
    if wants_lessons && shell.cockpit.lessons.is_none() {
        let read = shell
            .app
            .lessons_summary(view, locale)
            .map(|summary| Lessons {
                locale,
                view,
                summary,
            });
        match read {
            Ok(l) => shell.cockpit.lessons = Some(Rc::new(l)),
            Err(e) => log("the lessons", &e),
        }
    }
    // The stamp is taken after reading: a read that writes (a first-time
    // setup) must not make the next frame read everything again.
    let at = shell.cockpit.read.map_or_else(Instant::now, |(_, at)| at);
    shell.cockpit.read = Some((shell.app.data_stamp(), at));
}
