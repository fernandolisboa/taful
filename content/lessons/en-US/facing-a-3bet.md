+++
id = "facing-a-3bet"
version = 1
title = "Facing a 3-bet"
prerequisites = ["three-bet-101"]
tags = ["preflop"]
glossary = ["four-bet", "pot-odds", "mdf"]
unlock = { quiz = "facing-a-3bet-quiz" }
minutes = 7
+++

# Facing a 3-bet

You opened and someone re-raised. You can fold, call, or [4-bet](term:four-bet).

## The price

The button opens to 2.5 bb and the big blind 3-bets to 9.5 bb. The small blind's 0.5 bb is in too. Calling costs 7 bb more to win a pot of 12.5 bb, so you need {{ calc: div(a=7, b=19.5) | pct(1) }} equity at showdown, before counting position or how well your hand plays.

## Do not over-fold

The [minimum defense frequency](term:mdf) idea applies loosely before the flop: if you fold too often to 3-bets, opponents can 3-bet any two cards and profit. Taful's chart, with the button facing a big blind 3-bet, folds about 43%, calls about 48% and 4-bets about 9%.

## What to call with

- **In position:** pairs, suited broadways, suited aces, good suited connectors. Position helps you realize your equity.
- **Out of position:** tighten up. Hands that play badly without position (weak offsuit broadways, small pairs) lose value fast.
- **Hands dominated by the 3-bettor's value range** like AJo or KQo are tricky: they often make second-best top pairs.

## 4-bets

4-bet for value with the top (QQ+, AK) and add a few bluffs with blockers, like A5s. Taful's 4-bet is 2.2 times the 3-bet (about 21 bb over a 9.5 bb 3-bet).

## The question to ask

Not "is my hand good?" but "against this 3-bettor's range, from this seat, does calling, 4-betting or folding win the most?". Drill it in spot mode with "Vs 3-bet".
