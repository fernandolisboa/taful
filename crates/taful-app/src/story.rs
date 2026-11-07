//! The story of a hand so far (#53): every table state from the deal and
//! one entry per action, so the table can show how a spot or a hand got to
//! hero's decision (the action log) and play it back (the replay).

use taful_core::action::{Action, Street};
use taful_core::cards::Card;
use taful_core::config::Chips;
use taful_core::engine::GameState;
use taful_core::position::{Position, Seat};

use crate::hand::Replay;

/// One action of the story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryAction {
    pub street: Street,
    pub seat: Seat,
    pub position: Position,
    pub action: Action,
    /// What the actor has in front after the action (the street's
    /// commitment): the size of a call, raise or all-in.
    pub to: Chips,
    /// What the action put in (a call's size).
    pub paid: Chips,
    /// Index into [`Story::states`] of the state right after the action.
    pub step: usize,
}

/// One line of the story: an action, or a street being dealt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoryEvent {
    Action(StoryAction),
    /// Cards dealt to the board (the flop, turn or river), with the state
    /// index right after the deal.
    Dealt {
        street: Street,
        cards: Vec<Card>,
        step: usize,
    },
}

impl StoryEvent {
    /// Index into [`Story::states`] of the state right after this event.
    pub fn step(&self) -> usize {
        match self {
            StoryEvent::Action(a) => a.step,
            StoryEvent::Dealt { step, .. } => *step,
        }
    }
}

/// Every table state from the deal (blinds posted) to `state`, and the
/// events between them: `events[i]` leads to `states[i + 1]`.
#[derive(Debug, Clone)]
pub struct Story {
    pub states: Vec<GameState>,
    pub events: Vec<StoryEvent>,
}

impl Story {
    /// The story that led to `state`. Fails only on a state the engine
    /// cannot rebuild (never for states it produced).
    pub fn of(state: &GameState) -> Result<Story, String> {
        // `Replay::states` deals only the cards already on `state`'s
        // board, so its last state is `state`'s table.
        let states = Replay::of(state).states(state.config())?;
        let events = states
            .windows(2)
            .enumerate()
            .map(|(i, w)| event(&w[0], &w[1], i + 1))
            .collect::<Result<_, _>>()?;
        Ok(Story { states, events })
    }

    /// The last state (the one the story was built from).
    pub fn last(&self) -> &GameState {
        self.states.last().expect("a story has the deal")
    }
}

/// What happened between two consecutive states.
fn event(before: &GameState, after: &GameState, step: usize) -> Result<StoryEvent, String> {
    if after.board().len() > before.board().len() {
        return Ok(StoryEvent::Dealt {
            street: after.street(),
            cards: after.board()[before.board().len()..].to_vec(),
            step,
        });
    }
    let (seat, action) = after
        .actions()
        .iter()
        .rev()
        .find_map(|s| s.last().copied())
        .ok_or_else(|| "a step without an action".to_string())?;
    let player = &after.players()[seat];
    // A fold or check leaves the commitment as it was; the street can end
    // on a call and reset it, so read it from before plus what went in.
    let paid = before.players()[seat].stack - player.stack;
    // An all-in that closes the round gets its uncalled part back at
    // once; the log shows the shove, not what was called.
    let to = match action {
        Action::AllIn => before.players()[seat].street_commit + before.players()[seat].stack,
        _ => before.players()[seat].street_commit + paid,
    };
    Ok(StoryEvent::Action(StoryAction {
        street: before.street(),
        seat,
        position: before.position(seat),
        action,
        to,
        paid,
        step,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preflop::{PreflopTrainer, SpotKind};

    /// Every spot's story rebuilds its chart line and ends on hero's
    /// decision.
    #[test]
    fn spot_stories_rebuild_the_chart_line() {
        let t = PreflopTrainer::builtin().expect("built-in chart");
        for kind in t.available_kinds() {
            for seed in 0..20 {
                let s = t.spot(kind, seed).expect("spot");
                let story = Story::of(&s.state).expect("story");
                // The rebuilt deal has its own seed; the table is the same.
                assert_eq!(story.last().players(), s.state.players());
                assert_eq!(story.last().history(), s.state.history());
                assert_eq!(story.last().pot(), s.state.pot());
                assert_eq!(story.events.len() + 1, story.states.len());
                let line: Vec<(Position, Action)> = story
                    .events
                    .iter()
                    .map(|e| match e {
                        StoryEvent::Action(a) => (a.position, a.action),
                        StoryEvent::Dealt { .. } => panic!("preflop spot dealt a board"),
                    })
                    .collect();
                assert_eq!(line, t.chart().line(s.node).0, "{kind:?} seed {seed}");
                assert_eq!(story.last().to_act(), Some(s.hero));
            }
        }
    }

    /// The sizes are what each player has in front after acting; a jam
    /// shows the whole stack.
    #[test]
    fn sizes_are_commitments_after_the_action() {
        let t = PreflopTrainer::builtin().expect("built-in chart");
        let s = t.spot(SpotKind::VsAllIn, 1).expect("spot");
        let story = Story::of(&s.state).expect("story");
        for e in &story.events {
            let StoryEvent::Action(a) = e else { continue };
            let after = &story.states[a.step].players()[a.seat];
            match a.action {
                Action::Raise(to) => assert_eq!(a.to, to),
                Action::AllIn => assert_eq!(after.stack, 0),
                Action::Fold | Action::Check => {
                    assert_eq!(
                        a.to,
                        story.states[a.step - 1].players()[a.seat].street_commit
                    )
                }
                _ => {}
            }
        }
    }
}
