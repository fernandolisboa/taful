+++
id = "equity-101"
version = 1
title = "Equity"
prerequisites = ["combos-101"]
tags = ["math"]
glossary = ["equity", "range"]
unlock = { quiz = "equity-101-quiz" }
minutes = 7
+++

# Equity

Your [equity](term:equity) is your share of the pot if all the cards were dealt out with no more betting: how often you win, plus half of how often you tie.

## Hand against hand

On a flop of 7♠ 6♠ 2♦, 9♠ 8♠ (a flush draw plus an open-ended straight draw) against A♥ A♣ has {{ calc: equity_vs_hand(hero="9s8s", villain="AhAc", board="7s6s2d") | pct(1) }} equity. It has no pair yet, but so many turns and rivers help it (any spade, any five or ten) that it is the favorite.

The same aces against a lone gutshot are a different story: J♣ 9♣ on Q♦ 8♠ 2♥ has only {{ calc: equity_vs_hand(hero="Jc9c", villain="AhAc", board="Qd8s2h") | pct(1) }}.

## Hand against a range

You rarely know your opponent's exact hand. Equity against a [range](term:range) averages over every combo they can hold. A♠ K♠ on J♥ 7♦ 2♣ against a range of QQ, JJ and AQ has {{ calc: equity_vs_range(hero="AsKs", villain="QQ,JJ,AQ", board="Jh7d2c") | pct(1) }} equity: it beats AQ and needs help against the pairs.

## Equity is not the whole story

Equity assumes the hand is checked down. Position, stack depth and future bets change how much of that equity you actually realize. Still, every later decision starts from this number: you will compare it with the price you are being offered in the pot odds lesson.
