+++
id = "ranges-101"
version = 1
title = "Thinking in ranges"
prerequisites = ["combos-101", "positions-6max"]
tags = ["preflop", "strategy"]
glossary = ["range", "combo"]
unlock = { quiz = "ranges-101-quiz" }
minutes = 6
+++

# Thinking in ranges

Nobody knows the exact cards an opponent holds. What you can know is their [range](term:range): every hand they would play this way, and how often.

## Writing a range

Ranges are written with a short notation:

- **QQ+** means QQ, KK and AA.
- **ATs+** means ATs, AJs, AQs and AKs (suited).
- **KQo** is king-queen offsuit.

Counting [combos](term:combo) turns a range into numbers. QQ+ is {{ calc: combos(range="QQ+") | int }} combos, AK is {{ calc: combos(range="AK") | int }}, and both together are {{ calc: combos(range="QQ+,AK") | int }}. There are {{ calc: choose(n=52, k=2) | int }} starting hands in total.

## Ranges, not hands

When the button opens, they do not open "a hand". They open about 40% of all hands, from AA down to some suited one-gappers. When they bet the flop, they bet some part of that range. Your job is to ask: which hands in that range act like this, and how do I do against all of them?

## Mixed strategies

Solvers often play one hand two ways: raise it 60% of the time, call 40%. That is not indecision. Mixing keeps a range balanced, so the opponent cannot tell a raise from a call. Taful shows these frequencies in the explanation panel, and grades you by EV: when two actions have almost the same EV, both are fine.

## Your own range

Think about your range too. If you only ever 3-bet AA and KK, a good opponent folds everything else and pays you nothing. Every action you take tells a story about the hands you could have.
