# Table screen (spec §10, §15.1): preflop spot mode and full hands (M3).

table-subtitle = Preflop, cash 6-max 100bb. Every decision is graded by EV loss against the reference charts.
table-mode-spot = Spot mode
table-mode-hand = Full hand
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
table-hand-over-flop = Preflop is done. Postflop play arrives with the postflop library.
table-hand-over-left = This line is outside the charts, so the hand ends here: { $line }
table-hand-ev-loss = EV lost this hand: { $amount } bb
