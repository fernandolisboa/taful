+++
id = "a-whole-hand"
version = 1
title = "Uma mão do começo ao fim"
prerequisites = ["seats-at-the-table"]
tags = ["basics"]
glossary = ["small-blind", "big-blind", "hole-cards", "preflop", "flop", "turn", "river", "showdown"]
unlock = { quiz = "a-whole-hand-quiz" }
minutes = 3
guided = "whole-hand"
+++

# Uma mão do começo ao fim

Hora de juntar tudo. Este é o caminho de uma mão completa:

1. **Blinds**: o [small blind](term:small-blind) põe {{ calc: div(a=1, b=2) | bb(1) }} e o [big blind](term:big-blind) põe 1 bb. O botão marca quem fala por último.
2. **Cartas**: cada jogador recebe as suas duas [cartas da mão](term:hole-cards), fechadas.
3. **[Pré-flop](term:preflop)**: começando à esquerda do big blind, cada um desiste, paga ou aumenta.
4. **[Flop](term:flop)**: três cartas na mesa e uma nova rodada, começando à esquerda do botão.
5. **[Turn](term:turn)**: a quarta carta, e mais uma rodada.
6. **[River](term:river)**: a quinta carta, e a última rodada.
7. **[Showdown](term:showdown)**: quem sobrou mostra as cartas, e a melhor mão de cinco leva o pote.

Em qualquer ponto, se todos menos um desistem, a mão acaba e quem sobrou leva o pote.

## A mão que você vai jogar

Na mão guiada desta lição você está no botão com J♥ T♥, e o HJ aumenta antes de você:

```seats
you: BTN
mark: HJ
blinds: yes
```

```cards
Suas cartas: Jh Th
```

Durante a mão, repare em três coisas:

- quem fala primeiro em cada rodada;
- quando dá para dar check e quando há uma aposta para igualar;
- quanto há no pote a cada rodada.
