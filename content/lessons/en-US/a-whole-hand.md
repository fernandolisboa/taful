+++
id = "a-whole-hand"
version = 1
title = "A hand from start to finish"
prerequisites = ["seats-at-the-table"]
tags = ["basics"]
glossary = ["small-blind", "big-blind", "hole-cards", "preflop", "flop", "turn", "river", "showdown"]
unlock = { quiz = "a-whole-hand-quiz" }
minutes = 3
guided = "whole-hand"
+++

# A hand from start to finish

Time to put it all together. This is the path of a complete hand:

1. **Blinds**: the [small blind](term:small-blind) puts in {{ calc: div(a=1, b=2) | bb(1) }} and the [big blind](term:big-blind) 1 bb. The button marks who speaks last from the flop on.
2. **Cards**: every player gets their two [hole cards](term:hole-cards), face down.
3. **[Preflop](term:preflop)**: starting on the big blind's left, each player folds, calls or raises (the big blind can check if nobody raised).
4. **[Flop](term:flop)**: three cards on the board and a new round, starting on the button's left.
5. **[Turn](term:turn)**: the fourth card, and one more round.
6. **[River](term:river)**: the fifth card, and the last round.
7. **[Showdown](term:showdown)**: the players left show their cards, and the best five-card hand takes the pot.

At any point, if everyone but one folds, the hand is over and whoever is left takes the pot.

## The hand you will play

In this lesson's guided hand you are on the button with J♥ T♥, and the HJ raises before you:

```seats
you: BTN
mark: HJ
blinds: yes
```

```cards
Your cards: Jh Th
```

During the hand, watch for three things:

- who speaks first in each round;
- when you can check and when there is a bet to match;
- how much is in the pot in each round.
