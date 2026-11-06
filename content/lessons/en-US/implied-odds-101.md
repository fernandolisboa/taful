+++
id = "implied-odds-101"
version = 1
title = "Implied odds"
prerequisites = ["ev-101"]
tags = ["math", "postflop"]
glossary = ["implied-odds", "pot-odds", "dirty-outs"]
unlock = { quiz = "implied-odds-101-quiz" }
minutes = 7
+++

# Implied odds

[Pot odds](term:pot-odds) only count the chips in the middle now. [Implied odds](term:implied-odds) add the chips you expect to win on later streets when you hit.

## How much more do you need?

On the turn you hold a flush draw: 9 outs, {{ calc: hit_probability(outs=9, cards=1) | pct(1) }} to hit on the river. The pot is 10 bb and the opponent bets 5 bb. The call on its own is worth {{ calc: call_ev(equity=0.196, pot=10, bet=5) | bb(2) }}.

To break even you must win the missing amount back when you hit: about {{ calc: div(a=1.08, b=0.196) | bb(1) }} more on average on the river. If the opponent will often pay a river bet that size, the call is fine.

## When implied odds are good

- **Stacks are deep.** There must be money left to win.
- **Your draw is hidden.** A gutshot to a straight on a rainbow board is harder to see than a third suited card.
- **You draw to the nuts.** Hitting a weak flush that loses to a better flush costs money instead of winning it.
- **The opponent has a strong hand** and will not let go of it.

## Reverse implied odds

Some hands win a small pot when they are ahead and lose a big one when they are behind: weak top pairs, low flushes, straights on the low end. Their [dirty outs](term:dirty-outs) and second-best hands pay off on later streets. Count these costs as well, and prefer folding or controlling the pot with them.

## Rule of thumb

Implied odds justify calls with draws that are a little short on price, not calls that are far off. A small bet makes a draw easy to call; a pot-size bet usually does not.
