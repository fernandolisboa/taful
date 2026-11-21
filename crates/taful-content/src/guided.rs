//! Guided hands (D26): a scripted hand played on the real table. Seats,
//! cards, the board and every opponent action are fixed; the hand pauses at
//! each step with a coach line, and at hero's turns waits for the one right
//! click. A wrong click is explained and retried: it is never played, graded
//! or stored.
//!
//! One TOML file per hand under `content/guided/`:
//!
//! ```toml
//! id = "first-hand"
//! version = 1
//! title = { en-US = "…", pt-BR = "…" }
//! players = 6            # cash, 100 bb each unless `stack_bb` says otherwise
//! hero = "BB"            # hero's position
//! board = "Kd7c2h4s9d"   # dealt street by street, in this order
//!
//! [cards]                # hole cards by position; hero and every player
//! BB = "Ks9s"            # who reaches showdown need theirs
//! SB = "Qd7d"
//!
//! [[steps]]              # a coach line alone: the user clicks "continue"
//! say = { en-US = "…", pt-BR = "…" }
//!
//! [[steps]]              # opponents act (canonical `POS:act`, comma-
//! act = "UTG:f,HJ:f"     # separated); with `say`, the hand pauses after
//!
//! [[steps]]              # hero's turn: the right action (amounts in chips,
//! hero = "x"             # 100 = 1 bb), the coach line while waiting, and
//! say = { … }            # why when another button is clicked
//! why = { … }
//! offer = ["x", "r300"]  # optional: the buttons (default: fold if facing
//!                        # a bet, check or call, and one bet or raise)
//! [[steps.wrong]]        # optional: a better reason for one wrong button
//! act = "r"              # an action kind (f x c b r a) or an exact action
//! why = { … }
//! ```
//!
//! The board comes out as soon as a betting round closes, so a `say` right
//! after the last action of a street talks about the new cards. The hand
//! must play to its end: every step legal, every board card dealt.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use taful_core::action::{Action, BetSize, BetTree, StreetSizes};
use taful_core::cards::{Card, CardSet, Hand, parse_cards};
use taful_core::config::GameConfig;
use taful_core::engine::{GameState, Phase, Settlement, legal_actions};
use taful_core::eval::RsPokerEvaluator;
use taful_core::position::{Position, Seat};

use crate::library::LOCALES;
use crate::quiz::Localized;

/// The button's seat in every guided hand; positions follow from it.
const BUTTON: Seat = 0;

/// A guided-hand file as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub id: String,
    pub version: u32,
    pub title: Localized,
    #[serde(default = "default_players")]
    pub players: u8,
    #[serde(default = "default_stack_bb")]
    pub stack_bb: u32,
    /// Hero's position label (`BB`, `BTN`, …).
    pub hero: String,
    /// Hole cards by position label.
    pub cards: BTreeMap<String, String>,
    /// Board cards in deal order.
    #[serde(default)]
    pub board: String,
    pub steps: Vec<ScriptStep>,
}

fn default_players() -> u8 {
    6
}

fn default_stack_bb() -> u32 {
    100
}

/// One `[[steps]]` entry as written; which keys it has says what it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptStep {
    #[serde(default)]
    pub say: Option<Localized>,
    /// Opponents' actions, `POS:act` joined by `,`.
    #[serde(default)]
    pub act: Option<String>,
    /// Hero's right action.
    #[serde(default)]
    pub hero: Option<String>,
    #[serde(default)]
    pub why: Option<Localized>,
    #[serde(default)]
    pub offer: Option<Vec<String>>,
    #[serde(default)]
    pub wrong: Vec<ScriptWrong>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptWrong {
    pub act: String,
    pub why: Localized,
}

impl Script {
    pub fn parse(text: &str) -> Result<Script, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Every localized string with where it sits, for the locale check.
    fn texts(&self) -> Vec<(String, &Localized)> {
        let mut out = vec![("title".to_string(), &self.title)];
        for (i, s) in self.steps.iter().enumerate() {
            let n = i + 1;
            if let Some(t) = &s.say {
                out.push((format!("step {n} `say`"), t));
            }
            if let Some(t) = &s.why {
                out.push((format!("step {n} `why`"), t));
            }
            for w in &s.wrong {
                out.push((format!("step {n} `wrong` {:?}", w.act), &w.why));
            }
        }
        out
    }
}

/// Which wrong click an explanation is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrongMatch {
    /// Any action of this kind (`f x c b r a`).
    Kind(char),
    Exact(Action),
}

impl WrongMatch {
    fn parse(s: &str) -> Result<WrongMatch, String> {
        match s {
            "f" | "x" | "c" | "b" | "r" | "a" => Ok(WrongMatch::Kind(s.chars().next().unwrap())),
            _ => s.parse::<Action>().map(WrongMatch::Exact),
        }
    }

    fn matches(self, a: Action) -> bool {
        match self {
            WrongMatch::Kind(k) => kind(a) == k,
            WrongMatch::Exact(x) => x == a,
        }
    }
}

/// The kind letter of an action (its canonical form's first letter).
fn kind(a: Action) -> char {
    a.to_string().chars().next().unwrap_or('?')
}

/// A step, parsed.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// A coach line; the user continues.
    Say(Localized),
    /// Opponents act; with a line, the hand pauses after them.
    Bots {
        actions: Vec<(Position, Action)>,
        say: Option<Localized>,
    },
    /// Hero's turn.
    Hero {
        expected: Action,
        say: Localized,
        why: Localized,
        offer: Option<Vec<Action>>,
        wrong: Vec<(WrongMatch, Localized)>,
    },
}

impl Step {
    /// Whether the hand stops here for the user (silent opponent steps
    /// play on by themselves).
    pub fn pauses(&self) -> bool {
        !matches!(self, Step::Bots { say: None, .. })
    }
}

/// A guided hand ready to play: positions, cards and actions parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct Guide {
    pub id: String,
    pub version: u32,
    pub title: Localized,
    pub config: GameConfig,
    pub hero: Seat,
    /// Every seat's hole cards (the unscripted ones filled from the deck).
    pub holes: Vec<Hand>,
    /// Seats whose cards the script names.
    pub scripted: Vec<Seat>,
    pub board: Vec<Card>,
    pub steps: Vec<Step>,
}

/// The seat at `pos` with the button on [`BUTTON`].
fn seat_of(n: usize, pos: Position) -> Option<Seat> {
    (0..n).find(|&s| Position::of(n, BUTTON, s) == pos)
}

impl Guide {
    /// Parses positions, cards and actions. Legality is checked by
    /// playing the hand ([`check_script`]).
    pub fn compile(script: &Script) -> Result<Guide, Vec<String>> {
        let mut errs = Vec::new();
        let n = usize::from(script.players);
        let config = GameConfig::cash(script.players, script.stack_bb);
        if let Err(e) = config.validate() {
            return Err(vec![format!("players/stack_bb: {}", e.0)]);
        }
        let position = |label: &str, errs: &mut Vec<String>| -> Option<Seat> {
            match label.parse::<Position>() {
                Ok(p) => {
                    let seat = seat_of(n, p);
                    if seat.is_none() {
                        errs.push(format!("no {label} at a {n}-handed table"));
                    }
                    seat
                }
                Err(e) => {
                    errs.push(e);
                    None
                }
            }
        };
        let hero = position(&script.hero, &mut errs);
        let mut used = CardSet::EMPTY;
        let mut fixed: Vec<Option<Hand>> = vec![None; n];
        for (label, cards) in &script.cards {
            let Some(seat) = position(label, &mut errs) else {
                continue;
            };
            match cards.parse::<Hand>() {
                Ok(h) if used.intersects(h.mask()) => {
                    errs.push(format!("cards {cards:?} ({label}) are dealt twice"))
                }
                Ok(h) => {
                    used = used.union(h.mask());
                    fixed[seat] = Some(h);
                }
                Err(e) => errs.push(format!("cards of {label}: {e}")),
            }
        }
        let board = match parse_cards(&script.board) {
            Ok(b) => b,
            Err(e) => {
                errs.push(format!("board: {e}"));
                Vec::new()
            }
        };
        if board.len() > 5 {
            errs.push(format!("board has {} cards (at most 5)", board.len()));
        }
        for &c in &board {
            if used.contains(c) {
                errs.push(format!("board card {c} is dealt twice"));
            }
            used = used.with(c);
        }
        if let Some(h) = hero
            && fixed[h].is_none()
        {
            errs.push(format!("hero's cards ({}) are missing", script.hero));
        }
        let mut steps = Vec::new();
        for (i, s) in script.steps.iter().enumerate() {
            let n = i + 1;
            let step = match (&s.act, &s.hero) {
                (Some(_), Some(_)) => {
                    errs.push(format!("step {n}: `act` and `hero` in one step"));
                    continue;
                }
                (Some(act), None) => {
                    if s.why.is_some() || s.offer.is_some() || !s.wrong.is_empty() {
                        errs.push(format!(
                            "step {n}: `why`, `offer` and `wrong` belong to hero's steps"
                        ));
                    }
                    let mut actions = Vec::new();
                    for part in act.split(',').map(str::trim) {
                        let parsed = part
                            .split_once(':')
                            .ok_or_else(|| format!("expected `POS:act`, got {part:?}"))
                            .and_then(|(p, a)| Ok((p.parse::<Position>()?, a.parse::<Action>()?)));
                        match parsed {
                            Ok(pa) => actions.push(pa),
                            Err(e) => errs.push(format!("step {n}: {e}")),
                        }
                    }
                    Step::Bots {
                        actions,
                        say: s.say.clone(),
                    }
                }
                (None, Some(expected)) => {
                    let expected = match expected.parse::<Action>() {
                        Ok(a) => a,
                        Err(e) => {
                            errs.push(format!("step {n}: {e}"));
                            continue;
                        }
                    };
                    let (Some(say), Some(why)) = (&s.say, &s.why) else {
                        errs.push(format!("step {n}: hero's step needs `say` and `why`"));
                        continue;
                    };
                    let offer = s.offer.as_ref().map(|o| {
                        o.iter()
                            .filter_map(|a| match a.parse::<Action>() {
                                Ok(a) => Some(a),
                                Err(e) => {
                                    errs.push(format!("step {n}: offer: {e}"));
                                    None
                                }
                            })
                            .collect()
                    });
                    let wrong = s
                        .wrong
                        .iter()
                        .filter_map(|w| match WrongMatch::parse(&w.act) {
                            Ok(m) => Some((m, w.why.clone())),
                            Err(e) => {
                                errs.push(format!("step {n}: wrong: {e}"));
                                None
                            }
                        })
                        .collect();
                    Step::Hero {
                        expected,
                        say: say.clone(),
                        why: why.clone(),
                        offer,
                        wrong,
                    }
                }
                (None, None) => match &s.say {
                    Some(say) => {
                        if s.why.is_some() || s.offer.is_some() || !s.wrong.is_empty() {
                            errs.push(format!(
                                "step {n}: `why`, `offer` and `wrong` belong to hero's steps"
                            ));
                        }
                        Step::Say(say.clone())
                    }
                    None => {
                        errs.push(format!("step {n}: needs `say`, `act` or `hero`"));
                        continue;
                    }
                },
            };
            steps.push(step);
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let hero = hero.expect("no errors");
        // The seats the script leaves out get the lowest cards nobody holds:
        // they never reach showdown, so their cards are never shown.
        let mut spare = (0..52u8)
            .filter_map(Card::from_index)
            .filter(|c| !used.contains(*c));
        let scripted = (0..n).filter(|&s| fixed[s].is_some()).collect();
        let holes = fixed
            .into_iter()
            .map(|h| {
                h.unwrap_or_else(|| {
                    let (a, b) = (spare.next(), spare.next());
                    Hand::new(a.expect("deck"), b.expect("deck")).expect("distinct")
                })
            })
            .collect();
        Ok(Guide {
            id: script.id.clone(),
            version: script.version,
            title: script.title.clone(),
            config,
            hero,
            holes,
            scripted,
            board,
            steps,
        })
    }

    /// Hero's buttons at hero step `step` on `state`: the script's `offer`,
    /// else fold when facing a bet, check or call, and one bet or raise
    /// (the right one when it is aggressive, else a standard size).
    pub fn offered(&self, step: &Step, state: &GameState) -> Vec<Action> {
        let Step::Hero {
            expected, offer, ..
        } = step
        else {
            return Vec::new();
        };
        if let Some(o) = offer {
            return o.clone();
        }
        let mut out: Vec<Action> = legal_actions(state, &standard_tree())
            .into_iter()
            .filter(|a| !a.is_aggressive())
            .collect();
        let aggressive = legal_actions(state, &standard_tree())
            .into_iter()
            .filter(|a| a.is_aggressive())
            .collect::<Vec<_>>();
        if expected.is_aggressive() {
            out.push(*expected);
        } else if let Some(&a) = aggressive.first() {
            out.push(a);
        }
        out
    }
}

/// The default sizes a guided hand offers: preflop 2.5 bb opens and 3x
/// raises, after it half the pot or a 3x raise.
fn standard_tree() -> BetTree {
    let post = StreetSizes {
        bets: vec![BetSize::PotPct(50)],
        raises: vec![vec![BetSize::Mult(300)]],
        allin_threshold_pct: 67,
    };
    BetTree {
        preflop: BetTree::default_srp().preflop,
        flop: post.clone(),
        turn: post.clone(),
        river: post,
    }
}

/// Where a guided hand waits for the user.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pause<'a> {
    /// A coach line; the user continues.
    Say(&'a Localized),
    /// Hero's turn: the coach line and the buttons.
    Hero(&'a Localized),
    /// The hand is over.
    Done,
}

/// What a click on hero's turn did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// The right action: played, and the hand went on.
    Right,
    /// Another one: not played; [`Run::wrong`] explains it.
    Wrong,
}

/// A guided hand being played: the step machine.
///
/// The run always rests on a pause: a coach line, hero's turn or the end.
/// Opponents' actions without a line and the board cards play on their
/// own until the next pause.
#[derive(Debug, Clone)]
pub struct Run {
    guide: Arc<Guide>,
    state: GameState,
    /// The step the run rests on (`steps.len()` at the end). A `Bots`
    /// step it rests on has been played.
    at: usize,
    /// The last wrong click at this hero step.
    wrong: Option<Action>,
}

impl Run {
    pub fn start(guide: Arc<Guide>) -> Result<Run, String> {
        let holes: Vec<Option<Hand>> = guide.holes.iter().copied().map(Some).collect();
        let state = GameState::new_with_holes(guide.config.clone(), BUTTON, 0, &holes)
            .map_err(|e| e.to_string())?;
        let mut run = Run {
            guide,
            state,
            at: 0,
            wrong: None,
        };
        run.play_on()?;
        Ok(run)
    }

    /// The same hand from the deal.
    pub fn restart(&self) -> Result<Run, String> {
        Run::start(self.guide.clone())
    }

    pub fn guide(&self) -> &Guide {
        &self.guide
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn hero(&self) -> Seat {
        self.guide.hero
    }

    /// The step the run rests on and how many there are (progress).
    pub fn progress(&self) -> (usize, usize) {
        let pauses = |steps: &[Step]| steps.iter().filter(|s| s.pauses()).count();
        let steps = &self.guide.steps;
        (pauses(&steps[..self.at.min(steps.len())]), pauses(steps))
    }

    pub fn pause(&self) -> Pause<'_> {
        match self.guide.steps.get(self.at) {
            None => Pause::Done,
            Some(Step::Say(say)) => Pause::Say(say),
            Some(Step::Bots { say: Some(say), .. }) => Pause::Say(say),
            // Only after a broken script failed partway through a silent
            // opponent step; the checks keep shipped scripts from it.
            Some(Step::Bots { say: None, .. }) => Pause::Done,
            Some(Step::Hero { say, .. }) => Pause::Hero(say),
        }
    }

    /// The index of the step the run rests on (`steps.len()` at the end).
    pub fn step_at(&self) -> usize {
        self.at
    }

    pub fn is_done(&self) -> bool {
        self.at >= self.guide.steps.len()
    }

    /// Hero's buttons, on hero's turn.
    pub fn actions(&self) -> Vec<Action> {
        match self.guide.steps.get(self.at) {
            Some(step @ Step::Hero { .. }) => self.guide.offered(step, &self.state),
            _ => Vec::new(),
        }
    }

    /// The last wrong click on this turn and why it is not the move.
    pub fn wrong(&self) -> Option<(Action, &Localized)> {
        let a = self.wrong?;
        let Some(Step::Hero { why, wrong, .. }) = self.guide.steps.get(self.at) else {
            return None;
        };
        let why = wrong
            .iter()
            .find(|(m, _)| matches!(m, WrongMatch::Exact(x) if *x == a))
            .or_else(|| wrong.iter().find(|(m, _)| m.matches(a)))
            .map_or(why, |(_, w)| w);
        Some((a, why))
    }

    /// Past a coach line.
    pub fn proceed(&mut self) -> Result<(), String> {
        match self.pause() {
            Pause::Say(_) => {
                self.at += 1;
                self.play_on()
            }
            _ => Err("not on a coach line".into()),
        }
    }

    /// A click on hero's turn.
    pub fn act(&mut self, action: Action) -> Result<Click, String> {
        let Some(Step::Hero { expected, .. }) = self.guide.steps.get(self.at) else {
            return Err("not hero's turn".into());
        };
        if action != *expected {
            self.wrong = Some(action);
            return Ok(Click::Wrong);
        }
        self.apply(Some(self.guide.hero), action)?;
        self.wrong = None;
        self.at += 1;
        self.play_on()?;
        Ok(Click::Right)
    }

    /// Who won, once the hand is over.
    pub fn settlement(&self) -> Option<Settlement> {
        (self.state.phase() == Phase::Finished)
            .then(|| self.state.settle(&RsPokerEvaluator).ok())
            .flatten()
    }

    /// Plays the steps that need no click, up to the next pause.
    fn play_on(&mut self) -> Result<(), String> {
        let guide = self.guide.clone();
        while let Some(step) = guide.steps.get(self.at) {
            match step {
                Step::Say(_) | Step::Hero { .. } => return Ok(()),
                Step::Bots { actions, say } => {
                    for &(pos, a) in actions {
                        let seat = seat_of(self.state.players().len(), pos)
                            .ok_or_else(|| format!("no {pos} at this table"))?;
                        if seat == self.guide.hero {
                            return Err(format!("{pos} is hero: use a `hero` step"));
                        }
                        self.apply(Some(seat), a)?;
                    }
                    if say.is_some() {
                        return Ok(());
                    }
                    self.at += 1;
                }
            }
        }
        Ok(())
    }

    /// Applies `seat`'s action and deals the streets it opens.
    fn apply(&mut self, seat: Option<Seat>, action: Action) -> Result<(), String> {
        let to_act = self.state.to_act();
        if seat.is_some() && to_act != seat {
            let who = |s: Option<Seat>| {
                s.map_or("nobody".to_string(), |s| self.state.position(s).to_string())
            };
            return Err(format!(
                "{} acts, but it is {}'s turn",
                who(seat),
                who(to_act)
            ));
        }
        self.state = self.state.apply(action).map_err(|e| {
            format!(
                "{}: {e}",
                to_act.map_or("?".into(), |s| self.state.position(s).to_string())
            )
        })?;
        while let Phase::NeedsBoard(st) = self.state.phase() {
            let have = self.state.board().len();
            let need = st.board_len();
            let cards = self
                .guide
                .board
                .get(have..need)
                .ok_or_else(|| format!("the board needs {need} cards for the {st:?}"))?;
            self.state = self
                .state
                .deal_board(cards)
                .map_err(|e| format!("dealing the {st:?}: {e}"))?;
        }
        Ok(())
    }
}

/// Every problem with a script: parsing, locales, and playing it through
/// (each action legal and in turn, each right action among the buttons,
/// each `wrong` one a button that is not the right one, the hand played to
/// its end with every board card dealt, every player at showdown scripted).
pub fn check_script(script: &Script) -> Vec<String> {
    let mut errs = Vec::new();
    for (place, text) in script.texts() {
        for loc in LOCALES {
            if text.get(loc).is_none_or(|t| t.trim().is_empty()) {
                errs.push(format!("{place}: missing `{loc}`"));
            }
        }
        for loc in text.keys() {
            if !LOCALES.contains(&loc.as_str()) {
                errs.push(format!("{place}: unknown locale `{loc}`"));
            }
        }
    }
    if script.steps.is_empty() {
        errs.push("no steps".into());
    }
    let guide = match Guide::compile(script) {
        Ok(g) => Arc::new(g),
        Err(e) => {
            errs.extend(e);
            return errs;
        }
    };
    let mut run = match Run::start(guide.clone()) {
        Ok(r) => r,
        Err(e) => {
            errs.push(format!("step 1: {e}"));
            return errs;
        }
    };
    while !run.is_done() {
        let n = run.at + 1;
        let step = &guide.steps[run.at];
        let res = match step {
            Step::Hero {
                expected, wrong, ..
            } => {
                let offered = run.actions();
                for &a in &offered {
                    if run.state.apply(a).is_err() {
                        errs.push(format!("step {n}: button {a} is not legal here"));
                    }
                }
                if !offered.contains(expected) {
                    errs.push(format!("step {n}: {expected} is not among the buttons"));
                }
                for (m, _) in wrong {
                    if !offered.iter().any(|&a| a != *expected && m.matches(a)) {
                        errs.push(format!(
                            "step {n}: no wrong button matches `wrong` {}",
                            match m {
                                WrongMatch::Kind(k) => k.to_string(),
                                WrongMatch::Exact(a) => a.to_string(),
                            }
                        ));
                    }
                }
                if run.state.to_act() != Some(guide.hero) {
                    errs.push(format!("step {n}: it is not hero's turn"));
                    break;
                }
                run.act(*expected).map(|_| ())
            }
            _ => run.proceed(),
        };
        if let Err(e) = res {
            // The step that failed is the one after the pause just left.
            errs.push(format!("after step {n}: {e}"));
            return errs;
        }
    }
    if run.state.phase() != Phase::Finished {
        errs.push("the hand does not reach its end".into());
    } else {
        if run.state.board().len() != guide.board.len() {
            errs.push(format!(
                "the board has {} cards but only {} are dealt",
                guide.board.len(),
                run.state.board().len()
            ));
        }
        let live: Vec<Seat> = (0..run.state.players().len())
            .filter(|&s| !run.state.players()[s].folded)
            .collect();
        if live.len() > 1 {
            for s in live {
                if !guide.scripted.contains(&s) {
                    errs.push(format!(
                        "{} reaches showdown without scripted cards",
                        run.state.position(s)
                    ));
                }
            }
        }
    }
    errs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(s: &str) -> String {
        format!("{{ en-US = \"{s}\", pt-BR = \"{s}\" }}")
    }

    /// Folds to the small blind, who completes; hero checks in the big
    /// blind, bets the flop and calls the river to win at showdown.
    fn script() -> String {
        let l = loc;
        format!(
            r#"
id = "t"
version = 1
title = {title}
hero = "BB"
board = "Kd7c2h4s9d"

[cards]
BB = "Ks9s"
SB = "Qd7d"

[[steps]]
say = {intro}

[[steps]]
act = "UTG:f,HJ:f,CO:f,BTN:f"

[[steps]]
act = "SB:c"
say = {limp}

[[steps]]
hero = "x"
say = {check}
why = {check_why}

[[steps.wrong]]
act = "r"
why = {raise_why}

[[steps]]
act = "SB:x"

[[steps]]
hero = "b100"
say = {bet}
why = {bet_why}

[[steps]]
act = "SB:c"

[[steps]]
act = "SB:x"

[[steps]]
hero = "x"
say = {turn}
why = {turn_why}

[[steps]]
act = "SB:b200"

[[steps]]
hero = "c"
say = {call}
why = {call_why}

[[steps]]
say = {won}
"#,
            title = l("T"),
            intro = l("intro"),
            limp = l("limp"),
            check = l("check"),
            check_why = l("check why"),
            raise_why = l("raise why"),
            bet = l("bet"),
            bet_why = l("bet why"),
            turn = l("turn"),
            turn_why = l("turn why"),
            call = l("call"),
            call_why = l("call why"),
            won = l("won"),
        )
    }

    fn start(text: &str) -> Run {
        let s = Script::parse(text).unwrap();
        Run::start(Arc::new(Guide::compile(&s).unwrap())).unwrap()
    }

    #[test]
    fn a_valid_script_passes() {
        let s = Script::parse(&script()).unwrap();
        assert_eq!(check_script(&s), Vec::<String>::new());
    }

    #[test]
    fn the_step_machine_pauses_retries_and_plays_on() {
        let mut run = start(&script());
        assert_eq!(
            run.pause(),
            Pause::Say(&BTreeMap::from([
                ("en-US".into(), "intro".into()),
                ("pt-BR".into(), "intro".into()),
            ]))
        );
        assert!(run.act(Action::Check).is_err(), "no click on a coach line");
        run.proceed().unwrap();
        // The folds play on their own; the small blind's call pauses.
        assert!(matches!(run.pause(), Pause::Say(t) if t["en-US"] == "limp"));
        assert_eq!(run.state().players().iter().filter(|p| p.folded).count(), 4);
        run.proceed().unwrap();
        assert!(matches!(run.pause(), Pause::Hero(t) if t["en-US"] == "check"));
        assert_eq!(
            run.actions(),
            vec![Action::Check, Action::Raise(250)],
            "check, and one raise to try"
        );
        // A wrong click is explained, not played.
        let before = run.state().clone();
        assert_eq!(run.act(Action::Raise(250)).unwrap(), Click::Wrong);
        assert_eq!(run.state(), &before);
        let (a, why) = run.wrong().unwrap();
        assert_eq!(
            (a, why["en-US"].as_str()),
            (Action::Raise(250), "raise why")
        );
        // The right one plays; the flop comes and the small blind checks.
        assert_eq!(run.act(Action::Check).unwrap(), Click::Right);
        assert!(run.wrong().is_none());
        assert_eq!(run.state().board().len(), 3);
        assert!(matches!(run.pause(), Pause::Hero(t) if t["en-US"] == "bet"));
        assert_eq!(
            run.actions(),
            vec![Action::Check, Action::Bet(100)],
            "an aggressive right action is the one bet offered"
        );
        assert_eq!(run.act(Action::Check).unwrap(), Click::Wrong);
        assert_eq!(run.wrong().unwrap().1["en-US"], "bet why");
        run.act(Action::Bet(100)).unwrap();
        // Call, turn, check-check, river bet: hero to call.
        assert!(matches!(run.pause(), Pause::Hero(t) if t["en-US"] == "turn"));
        run.act(Action::Check).unwrap();
        assert!(matches!(run.pause(), Pause::Hero(t) if t["en-US"] == "call"));
        assert_eq!(
            run.actions(),
            vec![Action::Fold, Action::Call, Action::Raise(600)]
        );
        run.act(Action::Call).unwrap();
        assert!(matches!(run.pause(), Pause::Say(t) if t["en-US"] == "won"));
        let st = run.settlement().unwrap();
        assert!(st.showdown);
        assert_eq!(st.pots[0].winners, vec![run.hero()]);
        run.proceed().unwrap();
        assert!(run.is_done() && run.pause() == Pause::Done);
        assert_eq!(run.progress(), (7, 7));
    }

    #[test]
    fn broken_scripts_are_caught() {
        let flop_bet = "[[steps]]\nhero = \"b100\"";
        let cases: Vec<(String, &str)> = vec![
            (
                script().replacen(", pt-BR = \"intro\"", "", 1),
                "step 1 `say`: missing `pt-BR`",
            ),
            (
                script().replace("UTG:f,HJ:f,CO:f,BTN:f", "HJ:f,UTG:f,CO:f,BTN:f"),
                "HJ acts, but it is UTG's turn",
            ),
            (
                script().replace("act = \"SB:c\"\nsay", "act = \"SB:x\"\nsay"),
                "SB: illegal action: x: facing a bet",
            ),
            (
                script().replace(
                    flop_bet,
                    "[[steps]]\nhero = \"b100\"\noffer = [\"x\", \"b150\"]",
                ),
                "b100 is not among the buttons",
            ),
            (
                script().replace(
                    flop_bet,
                    "[[steps]]\nhero = \"b100\"\noffer = [\"x\", \"b100\", \"c\"]",
                ),
                "button c is not legal here",
            ),
            (
                script().replace(
                    "act = \"SB:c\"\n\n[[steps]]\nact = \"SB:x\"",
                    "act = \"SB:c,SB:x,SB:x\"",
                ),
                "SB acts, but it is BB's turn",
            ),
            (
                script().replace("[[steps]]\nact = \"SB:b200\"\n\n", ""),
                "it is not hero's turn",
            ),
            (
                script().replace("act = \"r\"\n", "act = \"f\"\n"),
                "no wrong button matches `wrong` f",
            ),
            (
                script().replace("SB = \"Qd7d\"", "SB = \"Kd7d\""),
                "board card Kd is dealt twice",
            ),
            (
                script().replace("BB = \"Ks9s\"\n", ""),
                "hero's cards (BB) are missing",
            ),
            (
                script().replace("SB = \"Qd7d\"\n", ""),
                "SB reaches showdown without scripted cards",
            ),
            (
                script().replace("hero = \"BB\"", "hero = \"UTG2\""),
                "no UTG2 at a 6-handed table",
            ),
            (
                script().replace("act = \"SB:x\"\n\n[[steps]]\nhero = \"b100\"", "act = \"SB:x\"\nwhy = { en-US = \"a\", pt-BR = \"a\" }\n\n[[steps]]\nhero = \"b100\""),
                "`why`, `offer` and `wrong` belong to hero's steps",
            ),
        ];
        for (text, expect) in cases {
            let errs = match Script::parse(&text) {
                Ok(s) => check_script(&s),
                Err(e) => vec![e],
            };
            assert!(
                errs.iter().any(|e| e.contains(expect)),
                "expected `{expect}` in {errs:#?}"
            );
        }
    }

    #[test]
    fn a_short_hand_and_a_long_board_are_caught() {
        // Without the river's steps the hand stops short.
        let short = script()
            .split("[[steps]]\nact = \"SB:b200\"")
            .next()
            .unwrap()
            .to_string();
        let errs = check_script(&Script::parse(&short).unwrap());
        assert!(
            errs.iter().any(|e| e.contains("does not reach its end")),
            "{errs:#?}"
        );
        // The small blind folds the flop: the turn and river stay undealt.
        let folded = script()
            .split("[[steps]]\nact = \"SB:c\"\n\n[[steps]]\nact = \"SB:x\"\n\n[[steps]]\nhero = \"x\"\nsay = { en-US = \"turn\"")
            .next()
            .unwrap()
            .to_string()
            + "[[steps]]\nact = \"SB:f\"\n";
        let errs = check_script(&Script::parse(&folded).unwrap());
        assert!(
            errs.iter().any(|e| e.contains("only 3 are dealt")),
            "{errs:#?}"
        );
    }
}
