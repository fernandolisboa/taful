+++
id = "alpha-101"
version = 1
title = "Bluffing: alpha"
prerequisites = ["mdf-101"]
tags = ["math", "theory"]
glossary = ["alpha", "mdf"]
unlock = { quiz = "alpha-101-quiz" }
minutes = 5
+++

# Bluffing: alpha

When you bluff, you win the pot every time your opponent folds and lose the bet when they call. [Alpha](term:alpha) is the fold frequency your bluff needs to break even.

## The formula

Alpha = bet / (pot + bet).

- A third-pot bluff needs {{ calc: alpha(pot=3, bet=1) | pct(0) }} folds.
- A half-pot bluff needs {{ calc: alpha(pot=2, bet=1) | pct(1) }} folds.
- A pot-size bluff needs {{ calc: alpha(pot=1, bet=1) | pct(0) }} folds.

## Alpha and MDF are two sides of one coin

Alpha + [MDF](term:mdf) = 100%. If your half-pot bluff needs your opponent to fold {{ calc: alpha(pot=2, bet=1) | pct(1) }} of the time, then they must defend {{ calc: mdf(pot=2, bet=1) | pct(1) }} to stop it.

## Pure bluffs only

Alpha assumes your bluff never wins when called. A semi-bluff with a draw also wins some of the times it gets called, so it needs fewer folds than alpha says. That is why draws make the best bluffs.
