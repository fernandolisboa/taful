+++
id = "bet-sizing"
version = 1
title = "Bet sizing"
prerequisites = ["cbet-101", "alpha-101"]
tags = ["postflop", "math"]
glossary = ["polarized", "linear", "alpha", "pot-odds"]
unlock = { quiz = "bet-sizing-quiz" }
minutes = 8
+++

# Bet sizing

Every bet size makes a trade between what you risk, what you can win, and what your opponent needs to continue.

## What each size asks

A small bet gives good [pot odds](term:pot-odds) and needs few folds:

- **A third of the pot:** the caller needs {{ calc: pot_odds(pot=3, bet=1) | pct(1) }} equity; a bluff needs {{ calc: alpha(pot=3, bet=1) | pct(1) }} folds.
- **Half the pot:** the caller needs {{ calc: pot_odds(pot=2, bet=1) | pct(1) }}; a bluff needs {{ calc: alpha(pot=2, bet=1) | pct(1) }}.
- **Pot:** the caller needs {{ calc: pot_odds(pot=1, bet=1) | pct(1) }}; a bluff needs {{ calc: alpha(pot=1, bet=1) | pct(1) }}.

## Small bets: linear ranges

Small bets fit a [linear](term:linear) range: many hands of medium and strong value, plus some bluffs. They work when you have a range advantage but few hands that want to play for stacks, and when the opponent's range has many weak hands that must fold.

## Big bets: polarized ranges

Big bets fit a [polarized](term:polarized) range: very strong hands and bluffs, with nothing in between. Medium hands cannot bet big: only better hands call. Big bets work when you have the nut advantage.

## Sizing across streets

Plan the sizes so the stacks go in by the river when you want them to. A common plan in a single-raised pot is a third or half pot on the flop, around three quarters on the turn, and pot or more on the river.

## In Taful

The postflop library offers a fixed set of sizes per spot, the sizes the solver used. A size that is not on the menu is not an option in the trainer. Watch which sizes the reference strategy uses most on each texture.
