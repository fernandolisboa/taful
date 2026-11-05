+++
id = "hand-rankings"
version = 1
title = "Ranking das mãos"
prerequisites = []
tags = ["basics"]
glossary = ["kicker", "board"]
unlock = { quiz = "hand-rankings-quiz" }
minutes = 6
+++

# Ranking das mãos

No Hold'em cada jogador forma a melhor mão de **cinco cartas** com as suas duas cartas fechadas e as cinco cartas da [mesa](term:board). Você pode usar as duas cartas da mão, uma ou nenhuma.

Da mais forte para a mais fraca:

1. **Straight flush**: cinco cartas em sequência do mesmo naipe (A♠ K♠ Q♠ J♠ T♠ é o royal flush).
2. **Quadra**: quatro cartas do mesmo valor.
3. **Full house**: uma trinca mais um par.
4. **Flush**: cinco cartas do mesmo naipe.
5. **Sequência (straight)**: cinco cartas seguidas. O ás vale alto (A-K-Q-J-T) ou baixo (5-4-3-2-A).
6. **Trinca**: um set (par na mão mais uma carta na mesa) ou trips (uma carta da mão mais um par na mesa).
7. **Dois pares**.
8. **Um par**.
9. **Carta alta**.

## Empates e kickers

Mãos da mesma categoria são comparadas carta a carta. Com A♠ K♦ contra A♣ Q♥ numa mesa A♥ 8♣ 6♦ 4♠ 2♣, os dois têm par de ases; o rei é um [kicker](term:kicker) melhor que a dama, então A-K ganha. Só cinco cartas contam: se a própria mesa for a melhor mão para os dois, o pote é dividido.

## Quantas mãos iniciais existem?

Duas cartas de um baralho de 52 dão {{ calc: choose(n=52, k=2) | int }} mãos iniciais possíveis. Boa parte do que você vai aprender é reduzir esse número às mãos que o adversário realmente pode ter.

> Naipe nunca desempata. Um flush é comparado pela carta mais alta, depois pela seguinte, e assim por diante.
