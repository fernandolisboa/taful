+++
id = "four-bet-pots"
version = 1
title = "4-bets e potes de SPR baixo"
prerequisites = ["facing-a-3bet", "spr-101"]
tags = ["preflop", "postflop"]
glossary = ["four-bet", "spr", "blocker"]
unlock = { quiz = "four-bet-pots-quiz" }
minutes = 6
+++

# 4-bets e potes de SPR baixo

Com 100 bb, um [4-bet](term:four-bet) compromete boa parte do stack. Planeje a mão inteira antes de fazê-lo.

## Os números

O big blind dá 3-bet para 9,5 bb, o botão dá 4-bet para 21 bb e o big blind paga. O pote fica em cerca de 42,5 bb com 79 bb atrás: um [SPR](term:spr) de {{ calc: spr(stack=79, pot=42.5) | ratio }}. Com SPR abaixo de 2, top pair normalmente topa colocar todo o dinheiro no flop.

Compare com um pote de raise simples (SPR {{ calc: spr(stack=97.5, pot=5.5) | ratio }}) ou um 3-bet pago fora de posição (cerca de {{ calc: spr(stack=90.5, pot=19.5) | ratio }}).

## Enfrentando um 4-bet

Contra um 4-bet você pode foldar, pagar ou ir all-in (um 5-bet). Os ranges agora são estreitos:

- **All-in:** AA e KK, parte de QQ e AK, e algumas mãos com [blocker](term:blocker) de ás que não podem pagar com lucro.
- **Call:** em posição, mãos que acertam bem o flop e não estão dominadas: QQ, JJ, às vezes AK, AQs.
- **Fold:** todo o resto. Um 3-bet de blefe que leva 4-bet normalmente folda.

## 4-bets de blefe

4-bets de blefe precisam mais de blockers do que de jogabilidade: A5s bloqueia AA e AK e ainda tem um pouco de equity quando pago. Use poucos; a maioria dos jogadores não dá 3-bet o bastante para muitos 4-bets de blefe darem lucro.

## Depois do flop

Com SPR abaixo de 2, a principal pergunta no flop é simples: esta mão é boa o bastante para ir all-in? Overpairs e top pair com bom kicker normalmente são. C-bets pequenos são comuns, porque uma aposta e um shove já colocam os stacks.
