+++
id = "bankroll-and-tilt"
version = 1
title = "Variância, bankroll e tilt"
prerequisites = ["study-loop"]
tags = ["mental-game"]
glossary = ["variance", "tilt", "ev"]
unlock = { quiz = "bankroll-and-tilt-quiz" }
minutes = 6
+++

# Variância, bankroll e tilt

Boas decisões ganham no longo prazo. No curto prazo, as cartas decidem. Lidar com essa diferença faz parte do jogo.

## Variância

A [variância](term:variance) é o quanto os resultados oscilam em torno do valor esperado. No No-Limit Hold'em as oscilações são grandes: um jogador vencedor pode perder por dezenas de milhares de mãos. Dez sessões ruins seguidas provam muito pouco.

Você tem 7♥7♣ em 7♠4♠2♦ e vai all-in contra A♠K♠. Você é bem favorito, {{ calc: equity_vs_hand(hero="7h7c", villain="AsKs", board="7s4s2d") | pct(0) }}, e ainda perde cerca de uma vez em quatro. Perder esse pote não é um erro.

## Bankroll

Bankroll é o dinheiro separado só para o poker. Ter buy-ins suficientes para o seu limite protege você de quebrar durante uma fase ruim. O conselho comum para cash game é ter pelo menos 30 a 50 buy-ins, mais se você ainda não tem certeza de que ganha no jogo.

## Tilt

[Tilt](term:tilt) é jogar pior por causa das emoções: depois de uma bad beat, de uma sequência longa de derrotas ou de um adversário provocador. Sinais típicos: pagar "para ver se ele tinha", jogar mãos que você normalmente folda, aumentar para recuperar o que perdeu.

## O que ajuda

- **Julgue a decisão, não o resultado.** Pergunte se o seu [EV](term:ev) era positivo, não se você ganhou.
- **Defina regras de parada:** um número de buy-ins perdidos ou um limite de tempo que encerra a sessão.
- **Faça uma pausa** depois de um pote grande, ganho ou perdido.
- **Revise depois,** com calma. O Taful guarda toda decisão avaliada para você conferir se uma mão dolorida foi mesmo um erro.
