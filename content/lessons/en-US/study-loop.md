+++
id = "study-loop"
version = 1
title = "Studying with Taful"
prerequisites = ["gto-vs-exploit"]
tags = ["strategy"]
glossary = ["ev", "leak"]
unlock = { quiz = "study-loop-quiz" }
minutes = 6
+++

# Studying with Taful

Taful runs one loop: play, grade, review, find leaks, drill them, and come back later through spaced repetition.

## Play and grade

Spot mode drills one decision at a time. Full hands play from preflop to the river against the reference bots. Every decision you make is graded by [EV](term:ev) loss: how much worse your action was than the best one, in big blinds.

## Review the hand

After a full hand, open the hand review. Step through the hand, look at each decision and its grade, and read the explanation: frequencies and EV of every action.

## Find your [leaks](term:leak)

The home screen shows your biggest leaks: groups of spots (configuration, street, node type, texture) where you lost the most EV recently. Use "Drill this" to practice that group directly.

## Spaced repetition

Every mistake, every wrong quiz answer and every slow mental-math answer becomes a review item. Taful schedules each item to come back just before you would forget it. Do the reviews when they are due: a few minutes a day beats a long session once a week.

## A daily routine

1. Clear the due reviews.
2. Read one lesson or drill the top leak.
3. Play a set of full hands and review the biggest mistakes.

## How to judge progress

Look at your average EV loss per decision over weeks, not your results over a session. Results swing with the cards; EV loss measures your decisions.
