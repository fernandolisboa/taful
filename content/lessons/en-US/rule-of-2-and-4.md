+++
id = "rule-of-2-and-4"
version = 1
title = "The rule of 2 and 4"
prerequisites = ["outs-101"]
tags = ["math", "postflop"]
glossary = ["outs"]
unlock = { quiz = "rule-of-2-and-4-quiz" }
minutes = 5
+++

# The rule of 2 and 4

Once you know your [outs](term:outs), turn them into a probability:

- With **one card** to come (turn to river), multiply outs by **2**.
- With **two cards** to come (flop to river, all-in), multiply outs by **4**.

## How close is it?

A flush draw on the flop has 9 outs. The rule says {{ calc: rule_of_2_and_4(outs=9, cards=2) | pct(0) }} to hit by the river; the exact chance is {{ calc: hit_probability(outs=9, cards=2) | pct(1) }}. On the turn the rule says {{ calc: rule_of_2_and_4(outs=9, cards=1) | pct(0) }}; the exact chance is {{ calc: hit_probability(outs=9, cards=1) | pct(1) }}.

## The correction above 8 outs

With many outs, times 4 overestimates. Subtract one point for every out above eight. A flush draw plus an open-ender has 15 outs: 15 × 4 = 60, minus 7, gives {{ calc: rule_of_2_and_4(outs=15, cards=2) | pct(0) }}. The exact value is {{ calc: hit_probability(outs=15, cards=2) | pct(1) }}.

> The ×4 rule only applies when you will see both cards, usually because someone is all-in. If you face another bet on the turn, use ×2 for the next card only.
