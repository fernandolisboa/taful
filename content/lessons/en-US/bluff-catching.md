+++
id = "bluff-catching"
version = 1
title = "Bluff-catching"
prerequisites = ["river-bluffing", "mdf-101"]
tags = ["postflop"]
glossary = ["bluff-catcher", "mdf", "pot-odds", "blocker"]
unlock = { quiz = "bluff-catching-quiz" }
minutes = 7
+++

# Bluff-catching

A [bluff-catcher](term:bluff-catcher) is a hand that beats only the opponent's bluffs. It loses to every value bet and wins against every bluff.

## The price

Facing a pot-size river bet, you get 2 to 1: you need to win {{ calc: pot_odds(pot=1, bet=1) | pct(1) }} of the time. If the bettor is bluffing more often than that, calling makes money. Facing half pot, you need only {{ calc: pot_odds(pot=2, bet=1) | pct(1) }}.

## How many to call

The [minimum defense frequency](term:mdf) against a pot-size bet is {{ calc: mdf(pot=1, bet=1) | pct(0) }} of your range. Below that, the bettor profits from bluffing any two cards.

But MDF is about your whole range, and on the river your range is what it is. If your range is weak because of how you played, folding more than MDF can be correct, and a good opponent will bet more often against you.

## Picking the right bluff-catchers

Among hands of similar strength, call with the ones that:

- **[block](term:blocker) value bets:** holding a card from the opponent's best hands means fewer value combos;
- **do not block bluffs:** holding a card from the opponent's missed draws means fewer bluffs, so call with other hands.

## Exploits

Against players who rarely bluff the river, fold bluff-catchers more. Against players who bluff too much, call more. The baseline comes from the math; the adjustment comes from what you know about the player.
