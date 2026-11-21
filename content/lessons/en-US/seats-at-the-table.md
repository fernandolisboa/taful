+++
id = "seats-at-the-table"
version = 1
title = "The seats at the table"
prerequisites = ["showdown"]
tags = ["basics"]
glossary = ["button", "small-blind", "big-blind", "position"]
unlock = { quiz = "seats-at-the-table-quiz" }
minutes = 4
guided = "seats-hand"
+++

# The seats at the table

At a six-player table, every seat has a name, counted from the button. Since the button moves every hand, the names move with it: the same player goes through all of them.

```seats
mark: BTN
blinds: yes
```

- **BTN** ([button](term:button)): the seat with the D disc.
- **SB** ([small blind](term:small-blind)) and **BB** ([big blind](term:big-blind)): the two seats on the button's left, which post the blinds.
- **UTG** (under the gun): on the big blind's left, the first to speak preflop.
- **HJ** (hijack) and **CO** (cutoff): the two seats before the button. The CO sits just to the button's right.

Tables with more players add seats between UTG and HJ, but the names at the end of the table never change.

## Speaking first and speaking last

Whoever speaks later has already seen what the others did before deciding. That is an edge, and it has a name: being in [position](term:position). From the flop on, the button speaks last in every round, and the blinds speak first.

If you are in the CO, from the flop on only the button speaks after you:

```seats
you: CO
order: SB BB UTG HJ CO BTN
```

In Fundamentos you will see how your seat changes which hands are worth playing. For now, the names and the order are enough.
