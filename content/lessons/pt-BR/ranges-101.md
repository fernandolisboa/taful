+++
id = "ranges-101"
version = 1
title = "Pensando em ranges"
prerequisites = ["combos-101", "positions-6max"]
tags = ["preflop", "strategy"]
glossary = ["range", "combo"]
unlock = { quiz = "ranges-101-quiz" }
minutes = 6
+++

# Pensando em ranges

Ninguém sabe as cartas exatas do adversário. O que dá para saber é o [range](term:range) dele: todas as mãos que ele jogaria desse jeito, e com que frequência.

## Escrevendo um range

Ranges são escritos com uma notação curta:

- **QQ+** quer dizer QQ, KK e AA.
- **ATs+** quer dizer ATs, AJs, AQs e AKs (suited).
- **KQo** é rei-dama offsuit.

Contar [combos](term:combo) transforma um range em números. QQ+ são {{ calc: combos(range="QQ+") | int }} combos, AK são {{ calc: combos(range="AK") | int }}, e os dois juntos são {{ calc: combos(range="QQ+,AK") | int }}. Existem {{ calc: choose(n=52, k=2) | int }} mãos iniciais no total.

## Ranges, não mãos

Quando o botão abre, ele não abre "uma mão". Ele abre cerca de 40% de todas as mãos, de AA até alguns suited com um buraco. Quando ele aposta no flop, aposta uma parte desse range. O seu trabalho é perguntar: quais mãos desse range agem assim, e como eu me saio contra todas elas?

## Estratégias mistas

Os solvers muitas vezes jogam uma mão de dois jeitos: aumentam 60% das vezes e pagam 40%. Isso não é indecisão. Misturar mantém um range equilibrado, para o adversário não distinguir um raise de um call. O Taful mostra essas frequências no painel de explicação e avalia você pelo EV: quando duas ações têm quase o mesmo EV, as duas estão boas.

## O seu próprio range

Pense no seu range também. Se você só dá 3-bet com AA e KK, um bom adversário folda todo o resto e não paga nada. Cada ação que você toma conta uma história sobre as mãos que você poderia ter.
