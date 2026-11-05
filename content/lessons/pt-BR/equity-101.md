+++
id = "equity-101"
version = 1
title = "Equity"
prerequisites = ["combos-101"]
tags = ["math"]
glossary = ["equity", "range"]
unlock = { quiz = "equity-101-quiz" }
minutes = 7
+++

# Equity

A sua [equity](term:equity) é a sua parte do pote se todas as cartas fossem distribuídas sem mais apostas: quantas vezes você ganha, mais metade das vezes que empata.

## Mão contra mão

Num flop 7♠ 6♠ 2♦, 9♠ 8♠ (projeto de flush mais sequência aberta) contra A♥ A♣ tem {{ calc: equity_vs_hand(hero="9s8s", villain="AhAc", board="7s6s2d") | pct(1) }} de equity. Ainda não tem nem par, mas tantos turns e rivers ajudam (qualquer espadas, qualquer cinco ou dez) que ele é o favorito.

Os mesmos ases contra um gutshot sozinho são outra história: J♣ 9♣ em Q♦ 8♠ 2♥ tem só {{ calc: equity_vs_hand(hero="Jc9c", villain="AhAc", board="Qd8s2h") | pct(1) }}.

## Mão contra range

Você raramente sabe a mão exata do adversário. A equity contra um [range](term:range) é a média sobre todos os combos que ele pode ter. A♠ K♠ em J♥ 7♦ 2♣ contra um range de QQ, JJ e AQ tem {{ calc: equity_vs_range(hero="AsKs", villain="QQ,JJ,AQ", board="Jh7d2c") | pct(1) }} de equity: ganha de AQ e precisa melhorar contra os pares.

## Equity não é tudo

A equity supõe que a mão vai até o showdown sem apostas. Posição, profundidade dos stacks e apostas futuras mudam quanto dessa equity você realiza de fato. Mesmo assim, toda decisão começa por esse número: na lição de pot odds você vai compará-lo com o preço que o pote oferece.
