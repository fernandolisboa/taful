+++
id = "four-bet-pots"
version = 1
title = "4-bets and low-SPR pots"
prerequisites = ["facing-a-3bet", "spr-101"]
tags = ["preflop", "postflop"]
glossary = ["four-bet", "spr", "blocker"]
unlock = { quiz = "four-bet-pots-quiz" }
minutes = 6
+++

# 4-bets and low-SPR pots

At 100 bb a [4-bet](term:four-bet) commits a big part of the stack. Plan the whole hand before you make it.

## The numbers

The big blind 3-bets to 9.5 bb, the button 4-bets to 21 bb and the big blind calls. The pot is about 42.5 bb with 79 bb behind: an [SPR](term:spr) of {{ calc: spr(stack=79, pot=42.5) | ratio }}. With an SPR under 2, top pair is usually happy to get all the money in on the flop.

Compare a single-raised pot (SPR {{ calc: spr(stack=97.5, pot=5.5) | ratio }}) or a called 3-bet out of position (about {{ calc: spr(stack=90.5, pot=19.5) | ratio }}).

## Facing a 4-bet

Against a 4-bet you can fold, call, or move all-in (a 5-bet). Ranges are narrow now:

- **All-in:** AA and KK, some QQ and AK, and a few hands with an ace [blocker](term:blocker) that cannot call profitably.
- **Call:** in position, hands that flop well and are not dominated: QQ, JJ, AK sometimes, AQs.
- **Fold:** everything else. A 3-bet bluff that gets 4-bet usually folds.

## 4-bet bluffs

4-bet bluffs need blockers more than playability: A5s blocks AA and AK and still has a little equity when called. Keep them few; most players do not 3-bet enough to make many 4-bet bluffs profitable.

## After the flop

With an SPR below 2, the main question on the flop is simple: is this hand good enough to stack off? Overpairs and top pair with a good kicker usually are. Small c-bets are common, because one bet and a shove already put the stacks in.
