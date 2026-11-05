+++
id = "outs-101"
version = 1
title = "Counting outs"
prerequisites = ["equity-101"]
tags = ["math", "postflop"]
glossary = ["outs", "dirty-outs"]
unlock = { quiz = "outs-101-quiz" }
minutes = 7
+++

# Counting outs

An [out](term:outs) is a card that, if it comes next, puts you ahead. Counting outs is the first step of every drawing decision.

## Common draws

- **Flush draw:** four cards of a suit, nine more in the deck: 9 outs.
- **Open-ended straight draw:** 9-8 on 7-6-2 makes a straight with any ten or five: 8 outs.
- **Gutshot:** one rank fills the straight: 4 outs.
- **Two overcards:** each can pair, 3 cards of each rank: 6 outs, but often weak ones.

## Dirty outs

Not every out is clean. Holding A♥ J♥ on K♥ 7♥ 2♦ against a set of kings, nine hearts make your flush, but the 2♥ pairs the board and gives the set a full house. Against that exact hand you have {{ calc: outs(hero="AhJh", villain="KsKc", board="Kh7h2d") | int }} outs, not 9. Cards like the 2♥ are [dirty outs](term:dirty-outs).

The same draw against a hand like K♠ Q♣ is much cleaner: aces now win too, so you have {{ calc: outs(hero="AhJh", villain="KsQc", board="Kh7h2d") | int }} outs.

## Count, then discount

At the table you do not know the exact hand. Count your outs by draw type, then subtract the ones that are likely to be dirty: cards that pair the board, or complete a better draw for your opponent.
