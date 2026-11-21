+++
id = "betting-rounds"
version = 1
title = "The four betting rounds"
prerequisites = ["chips-and-blinds"]
tags = ["basics"]
glossary = ["preflop", "flop", "turn", "river", "showdown", "button"]
unlock = { quiz = "betting-rounds-quiz" }
minutes = 4
guided = "first-hand"
+++

# The four betting rounds

A hand has up to four betting rounds. Between one round and the next, more cards are turned face up on the board.

1. **[Preflop](term:preflop)**: everyone has their two cards, and the board is still empty. The blinds are already in the pot.
2. **[Flop](term:flop)**: three cards come out at once.
3. **[Turn](term:turn)**: the fourth card comes out.
4. **[River](term:river)**: the fifth and last card comes out.

```cards
Flop: Kd 7c 2h
Turn: ~Kd ~7c ~2h | 4s
River: ~Kd ~7c ~2h | ~4s | 9d
```

If two or more players are still in after the river round, the [showdown](term:showdown) comes next.

## Who speaks first

The turn to act always passes to the left, clockwise, one player at a time. Only who starts changes.

**Preflop**, the player on the big blind's left starts, and the blinds speak last, since they already put chips in:

```seats
order: UTG HJ CO BTN SB BB
blinds: yes
```

**From the flop on**, the first player still in the hand on the [button](term:button)'s left starts, and the button speaks last:

```seats
order: SB BB UTG HJ CO BTN
```

## When a round ends

A round ends when everyone still in the hand has put the same amount of chips into it and each of them has had a chance to speak. If everyone checks, it ends too. And if everyone but one folds, the hand ends right there: no more cards come out.
