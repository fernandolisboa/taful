//! NLHE rules engine (spec §5.4).
//!
//! [`GameState`] is an immutable value: [`GameState::apply`] returns a new
//! state. The engine knows nothing about strategy; spot mode and full-hand mode
//! (§10) are drivers over it.
//!
//! Flow: [`Phase::ToAct`] → `apply` … → [`Phase::NeedsBoard`] → `deal_next` /
//! `deal_board` → … → [`Phase::Finished`] → [`GameState::settle`]. The board is
//! dealt by the driver so it can choose library flops (D13); `deal_next` takes
//! the next card of the seeded deck.
//!
//! Rules implemented:
//! - Posting: heads-up the button posts the small blind and acts first preflop.
//!   Per-player antes are live contributions; a big-blind ante is dead money in
//!   the main pot. A short blind posts what it has; the amount to call stays
//!   the full big blind.
//! - Min-raise = last full raise increment (at least the big blind). An all-in
//!   for less than a full raise does not reopen betting for players who have
//!   already acted, unless short all-ins add up to a full raise.
//! - Uncalled bets return to the bettor; side pots for any number of players;
//!   split pots with odd chips to the first winners left of the button; rake
//!   per [`Rake`](crate::config::Rake).

use std::sync::Arc;

use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;

use crate::action::{Action, ActionHistory, BetSize, BetTree, Street};
use crate::cards::{Card, CardSet, Hand};
use crate::config::{AnteKind, Chips, GameConfig};
use crate::eval::{HandEvaluator, HandRank};
use crate::position::{Position, Seat};

/// Per-seat state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub hole: Hand,
    /// Chips behind.
    pub stack: Chips,
    /// Committed on the current street.
    pub street_commit: Chips,
    /// Committed this hand, including antes (but not a big-blind ante).
    pub total_commit: Chips,
    pub folded: bool,
    /// Has acted since the street started (blinds do not count).
    acted: bool,
    /// The street's current bet right after this player's last action.
    faced: Chips,
}

impl Player {
    pub fn all_in(&self) -> bool {
        self.stack == 0
    }
    fn can_act(&self) -> bool {
        !self.folded && !self.all_in()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Waiting for this seat's action.
    ToAct(Seat),
    /// A betting round closed; the driver deals this street.
    NeedsBoard(Street),
    /// The hand is over; call [`GameState::settle`].
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    InvalidConfig(String),
    /// Hole cards or board cards collide.
    CardConflict,
    NotYourTurn,
    IllegalAction(String),
    WrongPhase,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::InvalidConfig(m) => write!(f, "invalid config: {m}"),
            EngineError::CardConflict => f.write_str("card conflict"),
            EngineError::NotYourTurn => f.write_str("not this player's turn"),
            EngineError::IllegalAction(m) => write!(f, "illegal action: {m}"),
            EngineError::WrongPhase => f.write_str("wrong phase for this operation"),
        }
    }
}

impl std::error::Error for EngineError {}

/// What the player to act may do, independent of any bet tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionBounds {
    pub seat: Seat,
    /// Chips needed to call (0 = may check).
    pub to_call: Chips,
    /// Calling puts the player all-in.
    pub call_is_all_in: bool,
    /// Smallest legal bet/raise-to, if any aggression is allowed.
    pub min_to: Option<Chips>,
    /// Largest bet/raise-to (all-in), if any aggression is allowed.
    pub max_to: Option<Chips>,
    /// Whether the aggressive action would be a bet (no bet yet) or a raise.
    pub is_bet: bool,
}

/// One pot after showdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotResult {
    /// Chips in the pot before rake.
    pub amount: Chips,
    pub rake: Chips,
    pub eligible: Vec<Seat>,
    pub winners: Vec<Seat>,
}

/// Outcome of a finished hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    pub pots: Vec<PotResult>,
    pub rake: Chips,
    /// Stack of every seat after the hand.
    pub final_stacks: Vec<Chips>,
    /// `final - initial` per seat.
    pub net: Vec<i64>,
    /// Hand went to showdown (two or more players reached the end).
    pub showdown: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameState {
    config: Arc<GameConfig>,
    seed: u64,
    button: Seat,
    players: Vec<Player>,
    initial_stacks: Vec<Chips>,
    board: Vec<Card>,
    /// Undealt cards; the next card is the last element.
    deck: Vec<Card>,
    street: Street,
    current_bet: Chips,
    last_full_raise: Chips,
    /// Bets/raises made on this street (the big blind does not count).
    aggressions: usize,
    /// Big-blind ante: dead money in the main pot.
    dead: Chips,
    phase: Phase,
    history: Vec<Vec<(Seat, Action)>>,
}

impl GameState {
    /// New hand with every hole card dealt from a deck shuffled by `seed`.
    pub fn new(config: GameConfig, button: Seat, seed: u64) -> Result<GameState, EngineError> {
        let n = usize::from(config.table_size);
        GameState::new_with_holes(config, button, seed, &vec![None; n])
    }

    /// New hand with some hole cards fixed (spot mode). The other seats are
    /// dealt from the seeded deck after removing the fixed cards.
    pub fn new_with_holes(
        config: GameConfig,
        button: Seat,
        seed: u64,
        holes: &[Option<Hand>],
    ) -> Result<GameState, EngineError> {
        config
            .validate()
            .map_err(|e| EngineError::InvalidConfig(e.0))?;
        let n = usize::from(config.table_size);
        if button >= n || holes.len() != n {
            return Err(EngineError::InvalidConfig(
                "button or holes out of range".into(),
            ));
        }
        let mut fixed = CardSet::EMPTY;
        for h in holes.iter().flatten() {
            if fixed.intersects(h.mask()) {
                return Err(EngineError::CardConflict);
            }
            fixed = fixed.union(h.mask());
        }
        let mut deck: Vec<Card> = fixed.complement().iter().collect();
        deck.shuffle(&mut ChaCha8Rng::seed_from_u64(seed));

        let mut players = Vec::with_capacity(n);
        for (seat, h) in holes.iter().enumerate() {
            let hole = match h {
                Some(h) => *h,
                None => {
                    let (a, b) = (deck.pop().expect("deck"), deck.pop().expect("deck"));
                    Hand::new(a, b).expect("distinct cards")
                }
            };
            players.push(Player {
                hole,
                stack: config.stack(seat),
                street_commit: 0,
                total_commit: 0,
                folded: false,
                acted: false,
                faced: 0,
            });
        }
        let initial_stacks = players.iter().map(|p| p.stack).collect();
        let bb = config.blinds.bb;
        let mut s = GameState {
            config: Arc::new(config),
            seed,
            button,
            players,
            initial_stacks,
            board: Vec::new(),
            deck,
            street: Street::Preflop,
            current_bet: bb,
            last_full_raise: bb,
            aggressions: 0,
            dead: 0,
            phase: Phase::Finished,
            history: vec![Vec::new()],
        };
        s.post_blinds_and_antes();
        s.phase = match s.first_to_act(s.preflop_first()) {
            Some(seat) => Phase::ToAct(seat),
            None => s.close_round(),
        };
        Ok(s)
    }

    fn n(&self) -> usize {
        self.players.len()
    }

    fn sb_seat(&self) -> Seat {
        if self.n() == 2 {
            self.button
        } else {
            (self.button + 1) % self.n()
        }
    }

    fn bb_seat(&self) -> Seat {
        (self.sb_seat() + 1) % self.n()
    }

    fn preflop_first(&self) -> Seat {
        (self.bb_seat() + 1) % self.n()
    }

    fn post_blinds_and_antes(&mut self) {
        let b = self.config.blinds;
        if b.ante_kind == AnteKind::PerPlayer {
            for p in &mut self.players {
                let a = b.ante.min(p.stack);
                p.stack -= a;
                p.total_commit += a;
            }
        }
        let blinds = [(self.sb_seat(), b.sb), (self.bb_seat(), b.bb)];
        for (seat, amount) in blinds {
            let p = &mut self.players[seat];
            let a = amount.min(p.stack);
            p.stack -= a;
            p.street_commit += a;
            p.total_commit += a;
        }
        if b.ante_kind == AnteKind::BigBlind {
            // Blind first, then the ante from what is left.
            let bb_seat = self.bb_seat();
            let p = &mut self.players[bb_seat];
            let a = b.ante.min(p.stack);
            p.stack -= a;
            self.dead += a;
        }
    }

    // ---- accessors -------------------------------------------------------

    pub fn config(&self) -> &GameConfig {
        &self.config
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn button(&self) -> Seat {
        self.button
    }
    pub fn players(&self) -> &[Player] {
        &self.players
    }
    pub fn initial_stacks(&self) -> &[Chips] {
        &self.initial_stacks
    }
    pub fn board(&self) -> &[Card] {
        &self.board
    }
    pub fn street(&self) -> Street {
        self.street
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn to_act(&self) -> Option<Seat> {
        match self.phase {
            Phase::ToAct(s) => Some(s),
            _ => None,
        }
    }
    pub fn current_bet(&self) -> Chips {
        self.current_bet
    }
    /// Bets/raises so far on this street (the big blind does not count).
    pub fn aggressions(&self) -> usize {
        self.aggressions
    }
    /// All chips in the middle, including the current street and dead money.
    pub fn pot(&self) -> Chips {
        self.dead + self.players.iter().map(|p| p.total_commit).sum::<Chips>()
    }
    pub fn position(&self, seat: Seat) -> Position {
        Position::of(self.n(), self.button, seat)
    }
    /// Actions per street by seat, in order.
    pub fn actions(&self) -> &[Vec<(Seat, Action)>] {
        &self.history
    }
    /// Actions per street by position (canonical form, spec §5.3).
    pub fn history(&self) -> ActionHistory {
        ActionHistory {
            streets: self
                .history
                .iter()
                .map(|st| st.iter().map(|&(s, a)| (self.position(s), a)).collect())
                .collect(),
        }
    }
    /// Cards not yet dealt to a player or the board.
    pub fn undealt(&self) -> &[Card] {
        &self.deck
    }

    // ---- betting ---------------------------------------------------------

    fn can_raise(&self, p: &Player) -> bool {
        !p.acted || self.current_bet.saturating_sub(p.faced) >= self.last_full_raise
    }

    /// Bounds for the player to act, or `None` outside [`Phase::ToAct`].
    pub fn bounds(&self) -> Option<ActionBounds> {
        let seat = self.to_act()?;
        let p = &self.players[seat];
        let to_call = self.current_bet.saturating_sub(p.street_commit);
        let max_to = p.street_commit + p.stack;
        let call_is_all_in = to_call > 0 && to_call >= p.stack;
        let aggress = !call_is_all_in && max_to > self.current_bet && self.can_raise(p);
        let (min_to, max_to_opt) = if aggress {
            let min = if self.current_bet == 0 {
                self.config.blinds.bb
            } else {
                self.current_bet + self.last_full_raise
            };
            (Some(min.min(max_to)), Some(max_to))
        } else {
            (None, None)
        };
        Some(ActionBounds {
            seat,
            to_call,
            call_is_all_in,
            min_to,
            max_to: max_to_opt,
            is_bet: self.current_bet == 0,
        })
    }

    /// Apply the action of the player to act.
    pub fn apply(&self, action: Action) -> Result<GameState, EngineError> {
        let b = self.bounds().ok_or(EngineError::WrongPhase)?;
        let seat = b.seat;
        let illegal = |m: &str| Err(EngineError::IllegalAction(format!("{action}: {m}")));
        let mut s = self.clone();
        let p_commit = s.players[seat].street_commit;
        let p_max = p_commit + s.players[seat].stack;

        // Normalize to a target street commitment and a recorded action.
        let (target, recorded) = match action {
            Action::Fold => {
                if b.to_call == 0 {
                    return illegal("nothing to call; check instead");
                }
                s.players[seat].folded = true;
                (p_commit, Action::Fold)
            }
            Action::Check => {
                if b.to_call > 0 {
                    return illegal("facing a bet");
                }
                (p_commit, Action::Check)
            }
            Action::Call => {
                if b.to_call == 0 {
                    return illegal("nothing to call");
                }
                ((p_commit + b.to_call).min(p_max), Action::Call)
            }
            Action::Bet(to) | Action::Raise(to) => {
                let is_bet = matches!(action, Action::Bet(_));
                if is_bet != b.is_bet {
                    return illegal(if b.is_bet {
                        "no bet to raise; bet instead"
                    } else {
                        "facing a bet; raise instead"
                    });
                }
                let (Some(min), Some(max)) = (b.min_to, b.max_to) else {
                    return illegal("raising is closed");
                };
                if to > max || (to < min && to != max) {
                    return illegal(&format!("amount must be in {min}..={max}"));
                }
                (to, if to == max { Action::AllIn } else { action })
            }
            Action::AllIn => {
                if p_max <= s.current_bet {
                    if b.to_call == 0 {
                        return illegal("no chips");
                    }
                    (p_max, Action::AllIn)
                } else if b.max_to.is_some() {
                    (p_max, Action::AllIn)
                } else {
                    return illegal("raising is closed; call or fold");
                }
            }
        };

        // Move chips.
        let p = &mut s.players[seat];
        let add = target - p.street_commit;
        p.stack -= add;
        p.street_commit = target;
        p.total_commit += add;
        if target > s.current_bet {
            let inc = target - s.current_bet;
            let full = if s.current_bet == 0 {
                target >= s.config.blinds.bb
            } else {
                inc >= s.last_full_raise
            };
            if full {
                s.last_full_raise = inc;
            }
            s.current_bet = target;
            s.aggressions += 1;
        }
        let p = &mut s.players[seat];
        p.acted = true;
        p.faced = s.current_bet;
        s.history.last_mut().expect("street").push((seat, recorded));

        s.phase = match s.next_to_act(seat) {
            Some(next) => Phase::ToAct(next),
            None => s.close_round(),
        };
        Ok(s)
    }

    fn needs_action(&self, p: &Player) -> bool {
        p.can_act() && (!p.acted || p.street_commit < self.current_bet)
    }

    /// First seat at or after `from` that still needs to act, if the round is open.
    fn first_to_act(&self, from: Seat) -> Option<Seat> {
        let live = self.players.iter().filter(|p| !p.folded).count();
        if live < 2 {
            return None;
        }
        let actors: Vec<&Player> = self.players.iter().filter(|p| p.can_act()).collect();
        // A lone player who can act and owes nothing has no decision to make.
        if actors.is_empty() || (actors.len() == 1 && actors[0].street_commit >= self.current_bet) {
            return None;
        }
        (0..self.n())
            .map(|i| (from + i) % self.n())
            .find(|&s| self.needs_action(&self.players[s]))
    }

    fn next_to_act(&self, after: Seat) -> Option<Seat> {
        self.first_to_act((after + 1) % self.n())
    }

    /// Betting round over: return any uncalled bet and decide what comes next.
    fn close_round(&mut self) -> Phase {
        self.return_uncalled();
        let live = self.players.iter().filter(|p| !p.folded).count();
        if live < 2 {
            return Phase::Finished;
        }
        match self.street.next() {
            Some(next) => Phase::NeedsBoard(next),
            None => Phase::Finished,
        }
    }

    fn return_uncalled(&mut self) {
        let mut commits: Vec<(Chips, Seat)> = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| (p.total_commit, i))
            .collect();
        commits.sort_unstable_by(|a, b| b.cmp(a));
        let (top, seat) = commits[0];
        let second = commits.get(1).map_or(0, |c| c.0);
        if top > second {
            let excess = top - second;
            let p = &mut self.players[seat];
            // Only chips from this street can be uncalled.
            let refund = excess.min(p.street_commit);
            p.stack += refund;
            p.street_commit -= refund;
            p.total_commit -= refund;
        }
    }

    // ---- dealing ---------------------------------------------------------

    /// Deal the pending street from the seeded deck.
    pub fn deal_next(&self) -> Result<GameState, EngineError> {
        let Phase::NeedsBoard(st) = self.phase else {
            return Err(EngineError::WrongPhase);
        };
        let k = st.board_len() - self.board.len();
        let cards: Vec<Card> = self.deck.iter().rev().take(k).copied().collect();
        self.deal_board(&cards)
    }

    /// Deal specific cards for the pending street (e.g. a library flop).
    pub fn deal_board(&self, cards: &[Card]) -> Result<GameState, EngineError> {
        let Phase::NeedsBoard(st) = self.phase else {
            return Err(EngineError::WrongPhase);
        };
        if self.board.len() + cards.len() != st.board_len() {
            return Err(EngineError::IllegalAction(format!(
                "{st:?} needs {} cards",
                st.board_len() - self.board.len()
            )));
        }
        let mut s = self.clone();
        for &c in cards {
            let i = s
                .deck
                .iter()
                .position(|&d| d == c)
                .ok_or(EngineError::CardConflict)?;
            s.deck.remove(i);
            s.board.push(c);
        }
        s.street = st;
        s.current_bet = 0;
        s.last_full_raise = s.config.blinds.bb;
        s.aggressions = 0;
        for p in &mut s.players {
            p.street_commit = 0;
            p.acted = false;
            p.faced = 0;
        }
        s.history.push(Vec::new());
        s.phase = match s.first_to_act((s.button + 1) % s.n()) {
            Some(seat) => Phase::ToAct(seat),
            None => s.close_round(),
        };
        Ok(s)
    }

    // ---- showdown --------------------------------------------------------

    /// Split the pots. Only valid in [`Phase::Finished`].
    pub fn settle(&self, eval: &dyn HandEvaluator) -> Result<Settlement, EngineError> {
        if self.phase != Phase::Finished {
            return Err(EngineError::WrongPhase);
        }
        let n = self.n();
        let contenders: Vec<Seat> = (0..n).filter(|&s| !self.players[s].folded).collect();
        let showdown = contenders.len() > 1;
        if showdown && self.board.len() != 5 {
            return Err(EngineError::WrongPhase);
        }

        // Pot layers at each distinct contender commitment.
        let mut levels: Vec<Chips> = contenders
            .iter()
            .map(|&s| self.players[s].total_commit)
            .collect();
        levels.sort_unstable();
        levels.dedup();
        let mut pots: Vec<PotResult> = Vec::new();
        let mut prev = 0;
        for (k, &lvl) in levels.iter().enumerate() {
            let last = k + 1 == levels.len();
            let amount: Chips = self
                .players
                .iter()
                .map(|p| {
                    let c = p.total_commit;
                    // Money above the top contender level (only possible from
                    // folded players) goes to the last pot.
                    let hi = if last { c } else { c.min(lvl) };
                    hi.saturating_sub(prev.min(c))
                })
                .sum();
            let eligible: Vec<Seat> = contenders
                .iter()
                .copied()
                .filter(|&s| self.players[s].total_commit >= lvl)
                .collect();
            pots.push(PotResult {
                amount,
                rake: 0,
                eligible,
                winners: vec![],
            });
            prev = lvl;
        }
        // Every hand has at least one contender, so there is a main pot.
        pots[0].amount += self.dead;

        // Rake, proportional to pot size; rounding remainder from the main pot.
        let total: Chips = pots.iter().map(|p| p.amount).sum();
        let rake = self.config.rake.amount(total, !self.board.is_empty());
        if rake > 0 {
            let mut taken = 0;
            for p in pots.iter_mut() {
                p.rake = (u128::from(rake) * u128::from(p.amount) / u128::from(total)) as Chips;
                taken += p.rake;
            }
            pots[0].rake += rake - taken;
        }

        let ranks: Vec<Option<HandRank>> = (0..n)
            .map(|s| {
                (showdown && !self.players[s].folded).then(|| {
                    let h = self.players[s].hole.cards();
                    let mut seven = [h[0]; 7];
                    seven[1] = h[1];
                    seven[2..].copy_from_slice(&self.board);
                    eval.rank7(&seven)
                })
            })
            .collect();

        let mut final_stacks: Vec<Chips> = self.players.iter().map(|p| p.stack).collect();
        // Seats in odd-chip order: first seat left of the button first.
        let order: Vec<Seat> = (1..=n).map(|i| (self.button + i) % n).collect();
        for pot in pots.iter_mut() {
            let best = pot.eligible.iter().filter_map(|&s| ranks[s]).max();
            let mut winners: Vec<Seat> = pot
                .eligible
                .iter()
                .copied()
                .filter(|&s| !showdown || ranks[s] == best)
                .collect();
            winners.sort_by_key(|w| order.iter().position(|o| o == w));
            let net = pot.amount - pot.rake;
            let share = net / winners.len() as Chips;
            let odd = (net % winners.len() as Chips) as usize;
            for (i, &w) in winners.iter().enumerate() {
                final_stacks[w] += share + Chips::from(i < odd);
            }
            pot.winners = winners;
        }
        let net = final_stacks
            .iter()
            .zip(&self.initial_stacks)
            .map(|(&f, &i)| f as i64 - i as i64)
            .collect();
        Ok(Settlement {
            pots,
            rake,
            final_stacks,
            net,
            showdown,
        })
    }
}

// ---- bet tree → legal actions ------------------------------------------------

fn round_div(num: u128, den: u128) -> Chips {
    ((num + den / 2) / den) as Chips
}

/// Legal actions for the player to act, with sizes from `tree` (spec §5.3).
///
/// Fold is offered only when facing a bet. Sizes below the minimum raise are
/// raised to it, sizes at or above the all-in threshold become [`Action::AllIn`],
/// and duplicates are removed. Empty outside [`Phase::ToAct`].
pub fn legal_actions(state: &GameState, tree: &BetTree) -> Vec<Action> {
    let Some(b) = state.bounds() else {
        return vec![];
    };
    let mut out = Vec::new();
    if b.to_call > 0 {
        out.push(Action::Fold);
        out.push(Action::Call);
    } else {
        out.push(Action::Check);
    }
    let (Some(min), Some(max)) = (b.min_to, b.max_to) else {
        return out;
    };
    let p = &state.players()[b.seat];
    let st = state.street();
    let street_tree = tree.street(st);
    let pot = state.pot();
    let bb = state.config().blinds.bb;
    let mut sizes: Vec<Chips> = Vec::new();
    let mut allin = false;
    for &size in tree.sizes(st, state.aggressions()) {
        let to = match size {
            BetSize::AllIn => {
                allin = true;
                continue;
            }
            BetSize::PotPct(pct) => {
                if b.is_bet {
                    round_div(u128::from(pot) * u128::from(pct), 100)
                } else {
                    let after_call = pot + b.to_call;
                    state.current_bet() + round_div(u128::from(after_call) * u128::from(pct), 100)
                }
            }
            BetSize::Mult(m) => {
                round_div(u128::from(state.current_bet().max(bb)) * u128::from(m), 100)
            }
            BetSize::Bb(h) => round_div(u128::from(bb) * u128::from(h), 100),
        };
        let to = to.max(min);
        let committed_pct = u128::from(to - p.street_commit) * 100;
        let threshold = street_tree.allin_threshold_pct;
        let stack = u128::from(p.stack);
        if to >= max || (threshold > 0 && committed_pct >= u128::from(threshold) * stack) {
            allin = true;
        } else if !sizes.contains(&to) {
            sizes.push(to);
        }
    }
    sizes.sort_unstable();
    for to in sizes {
        out.push(if b.is_bet {
            Action::Bet(to)
        } else {
            Action::Raise(to)
        });
    }
    if allin {
        out.push(Action::AllIn);
    }
    out
}

#[cfg(test)]
mod tests;
