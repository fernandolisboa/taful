+++
id = "combos-101"
version = 1
title = "Counting combos"
prerequisites = ["stacks-and-blinds"]
tags = ["basics", "ranges"]
glossary = ["combo", "range", "blocker"]
unlock = { quiz = "combos-101-quiz" }
minutes = 7
+++

# Counting combos

A [range](term:range) is the set of hands a player can hold. To weigh a range you count its [combos](term:combo): the exact two-card combinations.

- A pocket pair such as QQ has {{ calc: combos(range="QQ") | int }} combos (choose 2 of the 4 queens).
- A suited hand such as AKs has {{ calc: combos(range="AKs") | int }} combos, one per suit.
- An offsuit hand such as AKo has {{ calc: combos(range="AKo") | int }} combos.
- So AK in total has {{ calc: combos(range="AK") | int }} combos, three times as many as a single pair.

## Why it matters

Suppose an opponent's range is QQ+ and AK. That is {{ calc: combos(range="QQ+,AK") | int }} combos, and {{ calc: combos(range="AK") | int }} of them are AK, which is not even a pair. Counting combos keeps you from imagining that a range is "all big pairs".

## Blockers

Cards you can see remove combos. If you hold an ace, your opponent can make fewer AK combos: with one ace gone there are 3 aces left, so 3 × 4 = 12 AK combos instead of 16. A card that removes combos from an opponent's range is a [blocker](term:blocker).
