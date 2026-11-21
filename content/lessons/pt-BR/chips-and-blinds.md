+++
id = "chips-and-blinds"
version = 1
title = "Fichas, blinds e o botão"
prerequisites = ["hand-rankings"]
tags = ["basics"]
glossary = ["stack", "small-blind", "big-blind", "button", "pot"]
unlock = { quiz = "chips-and-blinds-quiz" }
minutes = 4
guided = "blinds-hand"
+++

# Fichas, blinds e o botão

## Fichas e stack

Cada jogador tem uma pilha de fichas na mesa: o seu [stack](term:stack). Toda aposta sai dele, e todo pote que você ganha volta para ele. Quando o stack acaba, você não pode mais apostar.

## Os blinds: o pote começa antes das cartas

Se ninguém fosse obrigado a apostar, todo mundo podia esperar só as cartas perfeitas. Para sempre haver algo em jogo, dois jogadores põem fichas no [pote](term:pot) **antes** de ver as cartas. São os blinds (do inglês, "cegas": apostas feitas às cegas):

- O [small blind](term:small-blind) (SB) põe meia aposta.
- O [big blind](term:big-blind) (BB) põe uma aposta inteira.

O Taful conta tudo em big blinds (bb): o small blind põe {{ calc: div(a=1, b=2) | bb(1) }} e o big blind põe 1 bb. Assim, toda mão começa com {{ calc: add(a=0.5, b=1) | bb(1) }} no pote. Contar em big blinds deixa a conta igual em qualquer mesa, seja de centavos ou de milhares.

## O botão

Um disco marcado com **D** fica na frente de um jogador: é o [botão](term:button) (BTN), o lugar de quem daria as cartas. Os blinds sentam logo à esquerda dele: primeiro o small blind, depois o big blind.

```seats
blinds: yes
mark: BTN SB BB
```

Depois de cada mão, o botão anda um lugar para a esquerda, e os blinds andam junto. Assim cada jogador paga o small blind e o big blind uma vez a cada volta da mesa, e ninguém paga mais que os outros.

## O que custa desistir

Fichas que já estão no pote não voltam. Se você é o big blind e desiste quando alguém aumenta, perde o 1 bb que já tinha posto. Quem não é blind e desiste antes de pôr fichas não perde nada.
