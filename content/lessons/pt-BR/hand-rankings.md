+++
id = "hand-rankings"
version = 2
title = "Ranking das mãos"
prerequisites = ["what-is-holdem"]
tags = ["basics"]
glossary = ["showdown", "kicker", "board", "split-pot"]
unlock = { quiz = "hand-rankings-quiz" }
minutes = 5
guided = "rankings-showdown"
+++

# Ranking das mãos

No [showdown](term:showdown), ganha quem tem a melhor mão de cinco cartas. Mas o que é "melhor"? Existe uma ordem fixa, igual em qualquer mesa do mundo. A regra por trás dela: quanto mais difícil de formar, mais forte a mão.

Da mais forte para a mais fraca:

```cards
Straight flush: 9s 8s 7s 6s 5s
Quadra: Qh Qd Qc Qs | 4d
Full house: Kc Kh Kd | 7s 7c
Flush: Ah Jh 8h 5h 2h
Sequência: Td 9c 8h 7s 6d
Trinca: 5c 5h 5s | Ac Jd
Dois pares: Ad As | 9h 9c | Kh
Um par: Jh Js | Ac 8d 4c
Carta alta: As Qd 9c 6h 3s
```

1. **Straight flush**: cinco cartas em sequência e do mesmo naipe. O mais alto, A K Q J T do mesmo naipe, se chama royal flush.
2. **Quadra**: quatro cartas do mesmo valor.
3. **Full house**: uma trinca e um par juntos.
4. **Flush**: cinco cartas do mesmo naipe, em qualquer ordem.
5. **Sequência** (straight): cinco valores seguidos, com naipes misturados. O ás pode ser a carta mais alta (A K Q J T) ou a mais baixa (5 4 3 2 A).
6. **Trinca**: três cartas do mesmo valor.
7. **Dois pares**: dois pares diferentes.
8. **Um par**: duas cartas do mesmo valor.
9. **Carta alta**: nenhuma das anteriores. Vale a carta mais alta.

O T é o dez: no poker ele é escrito com uma letra só, como as outras cartas.

## Mesma categoria: quem ganha?

Se dois jogadores têm a mesma categoria, compara-se o valor: par de reis ganha de par de damas, e um flush com ás ganha de um flush com rei. Se ainda empatar, comparam-se as outras cartas das cinco, da mais alta para a mais baixa. A carta que desempata se chama [kicker](term:kicker).

```cards
Mesa: Ah 8c 6d 4s 2c
Você: As Kd
Adversário: Ac Qh
```

Os dois têm par de ases, com o ás da [mesa](term:board). Mas o seu rei é maior que a dama dele: você ganha pelo kicker.

> O naipe nunca desempata: copas não vale mais que espadas. Se as cinco cartas de dois jogadores tiverem os mesmos valores, a mão empata e o pote é dividido (um [pote dividido](term:split-pot)).
