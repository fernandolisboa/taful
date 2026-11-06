+++
id = "facing-cbet"
version = 1
title = "Facing a c-bet"
prerequisites = ["cbet-101", "mdf-101"]
tags = ["postflop"]
glossary = ["c-bet", "mdf", "pot-odds", "range"]
unlock = { quiz = "facing-cbet-quiz" }
minutes = 7
+++

# Facing a c-bet

You called before the flop and the raiser [c-bets](term:c-bet). Fold, call or raise?

## How much to defend

The [minimum defense frequency](term:mdf) gives a first answer. Against a third-pot bet you must continue with about {{ calc: mdf(pot=3, bet=1) | pct(0) }} of your [range](term:range); against half pot, {{ calc: mdf(pot=2, bet=1) | pct(0) }}; against pot, {{ calc: mdf(pot=1, bet=1) | pct(0) }}.

MDF is a guide, not a law: if the bettor's range is very strong, folding more is right; if they bet too often, defend more.

## The price

Against a third-pot bet your [pot odds](term:pot-odds) are {{ calc: pot_odds(pot=3, bet=1) | pct(1) }}. Hands like backdoor draws, gutshots and weak pairs get a price that small bets cannot deny. That is why small c-bets get called a lot.

## What to call

- Pairs of almost any kind against small bets.
- Draws: flush draws, open-enders, and gutshots with overcards.
- Ace-high with backdoor draws against small bets on dry boards.

## What to raise

Raise with a polarized set of hands: strong made hands (two-pair, sets) and good draws that can improve. Do not raise medium pairs: better hands continue, worse hands fold.

## What to fold

Hands with no pair, no draw and no backdoors, especially against bigger bets.

## In position vs out of position

In position, call more: you see the opponent's next action before deciding. Out of position, check-raise a little more and call a little less, because hands that just call have a hard time realizing their equity.
