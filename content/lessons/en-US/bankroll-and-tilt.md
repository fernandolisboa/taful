+++
id = "bankroll-and-tilt"
version = 1
title = "Variance, bankroll and tilt"
prerequisites = ["study-loop"]
tags = ["mental-game"]
glossary = ["variance", "tilt", "ev"]
unlock = { quiz = "bankroll-and-tilt-quiz" }
minutes = 6
+++

# Variance, bankroll and tilt

Good decisions win in the long run. In the short run, cards decide. Handling the gap is part of the game.

## Variance

[Variance](term:variance) is how far results swing around their expected value. In No-Limit Hold'em the swings are large: a winning player can lose for tens of thousands of hands. Ten bad sessions in a row prove very little.

You hold 7♥7♣ on 7♠4♠2♦ and get all-in against A♠K♠. You are a big favorite, {{ calc: equity_vs_hand(hero="7h7c", villain="AsKs", board="7s4s2d") | pct(0) }}, and you still lose about one time in four. Losing that pot is not a mistake.

## Bankroll

A bankroll is money set aside for poker only. Keeping enough buy-ins for your stake protects you from going broke during a downswing. Common advice for cash games is at least 30 to 50 buy-ins, more if you are not yet sure you beat the game.

## Tilt

[Tilt](term:tilt) is playing worse because of emotions: after a bad beat, a long losing run, or a needling opponent. Typical signs: calling to "see if they had it", playing hands you usually fold, raising to win back what you lost.

## What helps

- **Judge the decision, not the outcome.** Ask whether your [EV](term:ev) was positive, not whether you won.
- **Set stop rules:** a number of buy-ins lost or a time limit that ends the session.
- **Take a break** after a big pot, won or lost.
- **Review later,** calmly. Taful keeps every graded decision so you can check whether a painful hand was a mistake at all.
