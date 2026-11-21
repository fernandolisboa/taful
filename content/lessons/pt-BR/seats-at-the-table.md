+++
id = "seats-at-the-table"
version = 1
title = "Os lugares da mesa"
prerequisites = ["showdown"]
tags = ["basics"]
glossary = ["button", "small-blind", "big-blind", "position"]
unlock = { quiz = "seats-at-the-table-quiz" }
minutes = 4
guided = "seats-hand"
+++

# Os lugares da mesa

Numa mesa de seis jogadores, cada lugar tem um nome, contado a partir do botão. Como o botão anda a cada mão, os nomes andam junto: o mesmo jogador passa por todos eles.

```seats
mark: BTN
blinds: yes
```

- **BTN** ([botão](term:button)): o lugar do disco D.
- **SB** ([small blind](term:small-blind)) e **BB** ([big blind](term:big-blind)): os dois à esquerda do botão, que põem os blinds.
- **UTG** (under the gun, "na mira"): à esquerda do big blind, o primeiro a falar no pré-flop.
- **HJ** (hijack) e **CO** (cutoff): os dois lugares antes do botão. O CO fica logo à direita dele.

Em mesas com mais jogadores aparecem lugares extras entre o UTG e o HJ, mas os nomes do fim da mesa são sempre os mesmos.

## Falar antes e falar depois

Quem fala depois já viu o que os outros fizeram antes de decidir. Isso é uma vantagem, e tem nome: estar em [posição](term:position). Do flop em diante, o botão fala por último em todas as rodadas, e os blinds falam primeiro.

Se você está no CO, do flop em diante só o botão fala depois de você:

```seats
you: CO
order: SB BB UTG HJ CO BTN
```

Em Fundamentos você vai ver como o lugar muda as mãos que vale a pena jogar. Por enquanto, basta saber os nomes e a ordem.
