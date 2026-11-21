+++
id = "chips-and-blinds"
version = 1
title = "Chips, blinds and the button"
prerequisites = ["hand-rankings"]
tags = ["basics"]
glossary = ["stack", "small-blind", "big-blind", "button", "pot"]
unlock = { quiz = "chips-and-blinds-quiz" }
minutes = 4
guided = "blinds-hand"
+++

# Chips, blinds and the button

## Chips and stacks

Every player has a pile of chips on the table: their [stack](term:stack). Every bet comes out of it, and every pot you win goes back into it. When your stack runs out, you can no longer bet.

## The blinds: the pot starts before the cards

If nobody had to bet, everyone could wait for perfect cards. So that something is always at stake, two players put chips in the [pot](term:pot) **before** seeing their cards. These are the blinds, since they are bets made blind:

- The [small blind](term:small-blind) (SB) puts in half a bet.
- The [big blind](term:big-blind) (BB) puts in a whole bet.

Taful counts everything in big blinds (bb): the small blind puts in {{ calc: div(a=1, b=2) | bb(1) }} and the big blind 1 bb. So every hand starts with {{ calc: add(a=0.5, b=1) | bb(1) }} in the pot. Counting in big blinds keeps the math the same at any table, whether it plays for cents or thousands.

## The button

A disc marked **D** sits in front of one player: the [button](term:button) (BTN), the seat of whoever would deal the cards. The blinds sit right on its left: first the small blind, then the big blind.

```seats
blinds: yes
mark: BTN SB BB
```

After every hand, the button moves one seat to the left, and the blinds move with it. That way each player pays the small blind and the big blind once per trip around the table, and nobody pays more than the others.

## What folding costs

Chips already in the pot do not come back. If you are the big blind and fold when someone raises, you lose the 1 bb you had put in. A player who is not a blind and folds before putting in chips loses nothing.
