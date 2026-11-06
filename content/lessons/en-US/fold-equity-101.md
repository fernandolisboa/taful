+++
id = "fold-equity-101"
version = 1
title = "Fold equity and semi-bluffs"
prerequisites = ["alpha-101", "ev-101"]
tags = ["math", "postflop"]
glossary = ["fold-equity", "semi-bluff", "alpha", "range"]
unlock = { quiz = "fold-equity-101-quiz" }
minutes = 7
+++

# Fold equity and semi-bluffs

When you bet, you can win in two ways: the opponent folds now, or you win at showdown. [Fold equity](term:fold-equity) is the value of the first one.

## Pure bluffs

A bluff with no chance to win at showdown needs the opponent to fold at least [alpha](term:alpha) of the time. A half-pot bluff needs {{ calc: alpha(pot=2, bet=1) | pct(1) }} folds; a pot-size bluff needs {{ calc: alpha(pot=1, bet=1) | pct(1) }}.

## Semi-bluffs

A [semi-bluff](term:semi-bluff) is a bet with a hand that is behind now but can improve: a flush draw, an open-ended straight draw, overcards. It wins when the opponent folds, and it still wins some of the time when called.

That second way to win means a semi-bluff needs fewer folds than a pure bluff. A flush draw on the flop has about {{ calc: hit_probability(outs=9, cards=2) | pct(0) }} to get there by the river, so even when called it is far from dead.

## Why betting a draw can beat checking

- Checking gives up the fold equity.
- Betting builds a pot you will win big when you hit.
- Your strong hands and your draws look the same, so your value bets get paid more.

## When fold equity is low

Fold equity shrinks when the opponent's [range](term:range) is strong or sticky, when the pot is multiway, and when the bet is too small to fold out anything. In those spots, draws prefer to check and realize their equity cheaply.
