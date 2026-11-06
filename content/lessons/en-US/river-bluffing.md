+++
id = "river-bluffing"
version = 1
title = "Bluffing the river"
prerequisites = ["river-value", "fold-equity-101"]
tags = ["postflop"]
glossary = ["bluff", "blocker", "alpha", "polarized"]
unlock = { quiz = "river-bluffing-quiz" }
minutes = 8
+++

# Bluffing the river

A river [bluff](term:bluff) wins only when the opponent folds. There is no equity left to fall back on.

## How many bluffs

A [polarized](term:polarized) river bet should make the opponent's bluff-catchers indifferent between calling and folding. With a pot-size bet the caller gets 2 to 1, so about one bluff for every two value bets makes the call break even. With a half-pot bet, about one bluff for every three value bets.

The fold the bluff itself needs is [alpha](term:alpha): {{ calc: alpha(pot=1, bet=1) | pct(0) }} for a pot-size bet, {{ calc: alpha(pot=2, bet=1) | pct(0) }} for a half-pot bet.

## Choosing bluffs

Pick bluffs from hands that cannot win at showdown:

- **Missed draws** are natural bluffs: they have no showdown value.
- **[Blockers](term:blocker) matter.** Holding a card that the opponent needs to call removes calling combos. Bluffing with the ace of the flush suit on a three-flush board removes the nut flush from their range.
- **Unblock their folds.** Hands that hold the cards of the opponent's missed draws are worse bluffs: those missed draws would have folded.

## Do not bluff with showdown value

A hand that wins at showdown some of the time (a weak pair, ace-high) should usually check. Turning it into a bluff folds out the worse hands it already beats.

## Population tendencies

At low stakes many players call too much on the river. Against them, bluff less and value bet more. Against players who fold too often, bluff more. Taful's reference strategy is balanced; use it as the baseline and adjust with care.
