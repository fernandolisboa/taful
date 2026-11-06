+++
id = "hand-reading"
version = 1
title = "Hand reading"
prerequisites = ["turn-play"]
tags = ["strategy", "postflop"]
glossary = ["range", "combo", "blocker"]
unlock = { quiz = "hand-reading-quiz" }
minutes = 7
+++

# Hand reading

Hand reading is narrowing the opponent's [range](term:range) street by street, using every action they take.

## Start before the flop

Their seat and preflop action give the starting range. A UTG open is about 17% of hands. A big blind call against the button is wide but has almost no AA, KK or AK (those 3-bet).

## Remove hands at each action

On every street ask: which hands in their range would act this way, and which would not?

- They called a flop c-bet on Q-8-3 two-tone: pairs, draws and some ace-highs remain. Most complete misses folded.
- They check-raised: their range is polarized now, strong hands and draws.
- They checked back the turn in position: strong hands that want to build the pot are less likely.

## Count combos

When the range is narrow, count [combos](term:combo). On Q-8-3, a set of queens has {{ calc: choose(n=3, k=2) | int }} combos (three queens left, choose two). AQ has {{ calc: mul(a=3, b=4) | int }} combos with a queen on board. Two unpaired cards with neither on the board make 16. Draws are often more numerous than sets.

## Use your [blockers](term:blocker)

Your own cards remove combos from their range. Holding a queen on Q-8-3 cuts their sets of queens to one combo and their AQ to 8.

## Stay humble

Hand reading gives probabilities, not certainties. The point is not to name their exact hand but to know how your hand does against everything they can have.
