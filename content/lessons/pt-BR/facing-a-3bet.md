+++
id = "facing-a-3bet"
version = 1
title = "Enfrentando um 3-bet"
prerequisites = ["three-bet-101"]
tags = ["preflop"]
glossary = ["four-bet", "pot-odds", "mdf"]
unlock = { quiz = "facing-a-3bet-quiz" }
minutes = 7
+++

# Enfrentando um 3-bet

Você abriu e alguém aumentou de novo. Você pode foldar, pagar ou dar [4-bet](term:four-bet).

## O preço

O botão abre para 2,5 bb e o big blind dá 3-bet para 9,5 bb. O 0,5 bb do small blind também está no pote. Pagar custa mais 7 bb para ganhar um pote de 12,5 bb, então você precisa de {{ calc: div(a=7, b=19.5) | pct(1) }} de equity no showdown, antes de contar a posição ou o quanto a sua mão joga bem.

## Não folde demais

A ideia da [frequência mínima de defesa](term:mdf) vale mais ou menos antes do flop: se você folda demais contra 3-bets, os adversários podem dar 3-bet com quaisquer duas cartas e lucrar. O chart do Taful, com o botão contra um 3-bet do big blind, folda cerca de 43%, paga cerca de 48% e dá 4-bet em cerca de 9%.

## Com o que pagar

- **Em posição:** pares, broadways suited, ases suited, bons conectores suited. A posição ajuda a realizar a equity.
- **Fora de posição:** feche. Mãos que jogam mal sem posição (broadways offsuit fracos, pares baixos) perdem valor rápido.
- **Mãos dominadas pelo range de valor de quem deu 3-bet**, como AJo ou KQo, são complicadas: muitas vezes fazem top pair que perde.

## 4-bets

Dê 4-bet por valor com o topo (QQ+, AK) e acrescente alguns blefes com blockers, como A5s. O 4-bet do Taful é 2,2 vezes o 3-bet (cerca de 21 bb contra um 3-bet de 9,5 bb).

## A pergunta certa

Não é "minha mão é boa?", mas "contra o range de quem deu 3-bet, desta posição, pagar, dar 4-bet ou foldar ganha mais?". Treine no modo spot com "Contra 3-bet".
