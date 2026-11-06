+++
id = "board-texture"
version = 1
title = "Board texture"
prerequisites = ["ranges-101"]
tags = ["postflop"]
glossary = ["texture", "board"]
unlock = { quiz = "board-texture-quiz" }
minutes = 7
+++

# Board texture

The flop's [texture](term:texture) decides how ranges hit the [board](term:board), and so how both players should bet. Taful groups flops into families by a few features.

## Height

- **High boards** (A-, K- or Q-high): good for the preflop raiser, who has more big cards.
- **Middle boards** (J- to 9-high): more two-pairs, straights and draws for both.
- **Low boards** (8-high or lower): good for the big blind, which defends many small and connected hands.

## Suits

- **Rainbow** (three suits): no flush draw yet.
- **Two-tone** (two of a suit): flush draws exist.
- **Monotone** (one suit): someone may already have a flush; draws to the nut flush matter a lot.

## Connectedness

- **Dry:** K72 rainbow. Few draws, hands rarely change rank on the turn.
- **Wet:** 9-8-6 two-tone. Many straight and flush draws; the best hand on the flop is often not the best on the river.

## Paired boards

On 7-7-2, very few hands have trips. Both players miss most of the time, and the player with more overpairs and high cards (usually the preflop raiser) can bet small and often.

## Why it matters

- On **dry boards** you can bet small: few hands improve, so you need little protection.
- On **wet boards** bets get bigger and less frequent: draws must pay, and your weaker hands lose value fast.

Every postflop spot in Taful is tagged with its texture class. Leaks on the home screen are grouped by texture too, so you can see on which kind of flop you lose the most.
