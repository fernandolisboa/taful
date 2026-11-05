+++
id = "combos-101"
version = 1
title = "Contando combos"
prerequisites = ["stacks-and-blinds"]
tags = ["basics", "ranges"]
glossary = ["combo", "range", "blocker"]
unlock = { quiz = "combos-101-quiz" }
minutes = 7
+++

# Contando combos

Um [range](term:range) é o conjunto de mãos que um jogador pode ter. Para pesar um range você conta os seus [combos](term:combo): as combinações exatas de duas cartas.

- Um par na mão como QQ tem {{ calc: combos(range="QQ") | int }} combos (escolher 2 das 4 damas).
- Uma mão suited como AKs tem {{ calc: combos(range="AKs") | int }} combos, um por naipe.
- Uma mão offsuit como AKo tem {{ calc: combos(range="AKo") | int }} combos.
- Então AK no total tem {{ calc: combos(range="AK") | int }} combos, três vezes mais que um par.

## Por que isso importa

Suponha que o range do adversário seja QQ+ e AK. São {{ calc: combos(range="QQ+,AK") | int }} combos, e {{ calc: combos(range="AK") | int }} deles são AK, que nem par é. Contar combos evita imaginar que um range é "só par alto".

## Blockers

As cartas que você vê removem combos. Se você tem um ás, o adversário forma menos combos de AK: com um ás a menos restam 3, então são 3 × 4 = 12 combos de AK em vez de 16. Uma carta que remove combos do range do adversário é um [blocker](term:blocker).
