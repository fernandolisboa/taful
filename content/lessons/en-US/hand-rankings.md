+++
id = "hand-rankings"
version = 1
title = "Hand rankings"
prerequisites = []
tags = ["basics"]
glossary = ["kicker", "board"]
unlock = { quiz = "hand-rankings-quiz" }
minutes = 6
+++

# Hand rankings

In Hold'em every player makes the best **five-card** hand from their two hole cards and the five cards on the [board](term:board). You may use both hole cards, one, or none.

From strongest to weakest:

1. **Straight flush**: five in a row, same suit (A♠ K♠ Q♠ J♠ T♠ is a royal flush).
2. **Four of a kind**: four cards of one rank.
3. **Full house**: three of a kind plus a pair.
4. **Flush**: five cards of one suit.
5. **Straight**: five in a row. The ace plays high (A-K-Q-J-T) or low (5-4-3-2-A).
6. **Three of a kind**: a set (pocket pair plus one on the board) or trips (one hole card plus a board pair).
7. **Two pair**.
8. **One pair**.
9. **High card**.

## Ties and kickers

Hands of the same category are compared card by card. With A♠ K♦ against A♣ Q♥ on a board of A♥ 8♣ 6♦ 4♠ 2♣, both players have a pair of aces; the king is a better [kicker](term:kicker) than the queen, so A-K wins. Only five cards count: if the board itself is the best hand for both players, the pot is split.

## How many starting hands?

Two cards from a 52-card deck give {{ calc: choose(n=52, k=2) | int }} possible starting hands. Most of what you will learn is about narrowing that number down to the hands an opponent can actually hold.

> Suits never break ties. A flush is compared by its highest card, then the next one, and so on.
