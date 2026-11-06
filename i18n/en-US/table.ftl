# Table screen (spec §10, §15.1): preflop spot mode and full hands (M3).

table-subtitle = Preflop, cash 6-max 100bb. Every decision is graded by EV loss against the reference charts.
table-mode-spot = Spot mode
table-mode-hand = Full hand
table-mode-postflop = Postflop
table-spot-kind-rfi = Raise first in
table-spot-kind-vs-open = Vs open
table-spot-kind-squeeze = Squeeze
table-spot-kind-vs-3bet = Vs 3-bet
table-spot-kind-vs-4bet = Vs 4-bet
table-spot-kind-vs-allin = Vs all-in
table-spot-kind-bvb = Blind vs blind
table-pot = Pot { $amount } bb
table-stack = { $amount } bb
table-spot-prompt = Your move. Pick an action below.
table-no-spot = No spot available.
table-chart-error = Could not load the preflop chart: { $error }

## Actions (hero buttons show exactly the sizes in the tree, D4)

table-action-fold = Fold
table-action-check = Check
table-action-call = Call { $amount }
table-action-raise = Raise to { $amount }
table-action-bet = Bet { $amount }
table-action-allin = All-in { $amount }
table-log-call = Call
table-log-allin = All-in

## Grading (spec §8)

table-grade-best = Best
table-grade-correct = Correct
table-grade-inaccuracy = Inaccuracy
table-grade-mistake = Mistake
table-grade-blunder = Blunder
table-ev-loss = EV loss: { $amount } bb
table-decision-context = { $position } with { $hand }
table-col-action = Action
table-col-hand-freq = Your hand
table-col-ev = EV (bb)
table-col-range-freq = Range
table-next-spot = Next spot

## Full hand

table-decisions = Your decisions
table-new-hand = New hand
table-hand-over-settled = Hand over. Your result: { $amount } bb.
table-hand-over-flop = Preflop is done. Postflop play needs the postflop library, which is not installed.
table-hand-over-left = This line is outside the solved strategy, so the hand ends here: { $line }
table-hand-ev-loss = EV lost this hand: { $amount } bb

## Postflop spot mode (M4, spec §7)

table-subtitle-postflop = Postflop, cash 6-max 100bb. Flop and turn decisions graded against the solved library.
table-postflop-any = All spots
table-postflop-btn-bb-srp = BTN vs BB
table-postflop-co-bb-srp = CO vs BB
table-postflop-sb-bb-srp = SB vs BB
table-postflop-btn-sb-3bp = BTN vs SB 3-bet
table-postflop-co-btn-3bp = CO vs BTN 3-bet
table-postflop-missing = The postflop library is not on this computer yet. It is built by tools/library-build and read from { $dir }.
table-postflop-error = Could not load the postflop library: { $error }
table-busy-dealing = Dealing…
table-busy-acting = The other players are acting…

## Layout edit mode (#22)

layout-edit-start = Edit layout
layout-edit-title = Editing the layout
layout-edit-hint = Drag a panel by its header; it snaps to the grid when you let go (hold Alt to place it freely). Drag the corner to resize.
layout-edit-save = Save as a copy
layout-edit-cancel = Cancel
layout-edit-refused-overlap = It can't go there: it would cover the { $panel } panel.
layout-edit-refused-grid = It can't go there: panels stay inside the grid.
layout-edit-refused-empty = A panel can't be empty.
layout-edit-refused-invalid = That change would break the layout.
layout-edit-save-failed = Could not save the layout. Your changes are still here.
layout-panel-controls = Controls
layout-panel-table = Table
layout-panel-actions = Actions
layout-panel-explanation = Explanation

## Opponent profiles (spec §9). Names and one-line descriptions.
table-opponent = Opponent
bot-profile-reference = Reference (GTO)
bot-profile-reference-about = Plays the reference strategy.
bot-profile-nit = Nit
bot-profile-nit-about = Plays few hands and folds too much to aggression.
bot-profile-calling-station = Calling station
bot-profile-calling-station-about = Calls too much and rarely raises.
bot-profile-maniac = Maniac
bot-profile-maniac-about = Plays too many hands and raises far too often.
bot-profile-tag = TAG
bot-profile-tag-about = Tight-aggressive regular, close to the reference.
table-opponent-practice = Practice only for now: decisions against profiles don't go to reviews or leaks yet.
table-col-ev-profile = EV vs bot
table-col-ev-gto = GTO EV
table-vs-profile = Graded against { $profile }: EV against this bot.
table-exploit-line = Exploitative line: { $action }
table-gto-line = GTO line: { $action }
table-ev-gap = EV gap: { $amount } bb
table-vs-profile-prompt = Opponent: { $profile }. { $about } Graded by EV against this bot.
