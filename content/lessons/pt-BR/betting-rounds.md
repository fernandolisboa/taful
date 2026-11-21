+++
id = "betting-rounds"
version = 1
title = "As quatro rodadas"
prerequisites = ["chips-and-blinds"]
tags = ["basics"]
glossary = ["preflop", "flop", "turn", "river", "showdown", "button"]
unlock = { quiz = "betting-rounds-quiz" }
minutes = 4
guided = "first-hand"
+++

# As quatro rodadas

Uma mão tem até quatro rodadas de apostas. Entre uma rodada e outra, mais cartas são abertas na mesa.

1. **[Pré-flop](term:preflop)**: cada um recebeu suas duas cartas, e a mesa ainda está vazia. Os blinds já estão no pote.
2. **[Flop](term:flop)**: três cartas são abertas de uma vez.
3. **[Turn](term:turn)**: sai a quarta carta.
4. **[River](term:river)**: sai a quinta e última carta.

```cards
Flop: Kd 7c 2h
Turn: ~Kd ~7c ~2h | 4s
River: ~Kd ~7c ~2h | ~4s | 9d
```

Se depois da rodada do river ainda houver dois ou mais jogadores, vem o [showdown](term:showdown).

## Quem fala primeiro

A vez sempre passa para a esquerda, no sentido horário, um jogador de cada vez. Só muda quem começa.

**No pré-flop**, começa quem está à esquerda do big blind, e os blinds falam por último, porque já puseram fichas:

```seats
order: UTG HJ CO BTN SB BB
blinds: yes
```

**Do flop em diante**, começa o primeiro jogador ainda na mão à esquerda do [botão](term:button), e o botão fala por último:

```seats
order: SB BB UTG HJ CO BTN
```

## Quando uma rodada termina

A rodada termina quando todos que continuam na mão puseram a mesma quantidade de fichas nela e cada um já teve a chance de falar. Se todo mundo passa a vez, ela também termina. E se todos menos um desistem, a mão acaba ali mesmo: não sai mais nenhuma carta.
