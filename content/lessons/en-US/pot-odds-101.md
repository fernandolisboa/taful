+++
id = "pot-odds-101"
version = 1
title = "Pot odds"
prerequisites = ["rule-of-2-and-4"]
tags = ["math", "postflop"]
glossary = ["pot-odds", "equity"]
unlock = { quiz = "pot-odds-101-quiz" }
minutes = 7
+++

# Pot odds

When you face a bet, the pot offers you a price. [Pot odds](term:pot-odds) turn that price into the [equity](term:equity) you need to call without losing money.

## The formula

Call / (pot after you call) = bet / (pot + 2 × bet), where the pot is what was there before the bet.

Facing a half-pot bet you need {{ calc: pot_odds(pot=100, bet=50) | pct(1) }} equity to call. Facing a pot-size bet you need {{ calc: pot_odds(pot=100, bet=100) | pct(1) }}. Facing a third of the pot, only {{ calc: pot_odds(pot=90, bet=30) | pct(1) }}.

## Putting it together

On the turn you hold a flush draw: 9 outs, about {{ calc: rule_of_2_and_4(outs=9, cards=1) | pct(0) }} to hit on the river. The pot is 10 bb and your opponent bets 5 bb. You need {{ calc: pot_odds(pot=10, bet=5) | pct(1) }}, so calling straight away loses: your expected value is {{ calc: call_ev(equity=0.196, pot=10, bet=5) | bb(2) }} if you never win anything more when you hit.

That is where implied odds come in: the chips you expect to win on later streets when you hit. Small bets give good pot odds; big bets need either more equity or a lot of implied odds.

## Quick reference

- A third of the pot: {{ calc: pot_odds(pot=3, bet=1) | pct(1) }}
- Half the pot: {{ calc: pot_odds(pot=2, bet=1) | pct(1) }}
- Two thirds of the pot: {{ calc: pot_odds(pot=3, bet=2) | pct(1) }}
- Pot: {{ calc: pot_odds(pot=1, bet=1) | pct(1) }}
