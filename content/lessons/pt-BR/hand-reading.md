+++
id = "hand-reading"
version = 1
title = "Leitura de mãos"
prerequisites = ["turn-play"]
tags = ["strategy", "postflop"]
glossary = ["range", "combo", "blocker"]
unlock = { quiz = "hand-reading-quiz" }
minutes = 7
+++

# Leitura de mãos

Ler mãos é estreitar o [range](term:range) do adversário rodada a rodada, usando cada ação que ele toma.

## Comece antes do flop

A posição e a ação antes do flop dão o range inicial. Um open do UTG é cerca de 17% das mãos. Um call do big blind contra o botão é largo, mas quase não tem AA, KK ou AK (essas dão 3-bet).

## Tire mãos a cada ação

Em cada rodada pergunte: quais mãos do range dele agiriam assim, e quais não?

- Ele pagou um c-bet no flop em Q-8-3 two-tone: sobram pares, projetos e alguns ases altos. A maioria das mãos que erraram tudo foldou.
- Ele deu check-raise: o range agora é polarizado, mãos fortes e projetos.
- Ele deu check no turn em posição: mãos fortes que querem crescer o pote ficam menos prováveis.

## Conte combos

Quando o range é estreito, conte [combos](term:combo). Em Q-8-3, uma trinca de damas tem {{ calc: choose(n=3, k=2) | int }} combos (sobram três damas, escolha duas). AQ tem {{ calc: mul(a=3, b=4) | int }} combos com uma dama na mesa. Duas cartas diferentes que não estão na mesa fazem 16. Os projetos muitas vezes são mais numerosos que as trincas.

## Use os seus [blockers](term:blocker)

As suas cartas tiram combos do range dele. Ter uma dama em Q-8-3 reduz as trincas de dama dele a um combo e o AQ a 8.

## Tenha humildade

Ler mãos dá probabilidades, não certezas. O objetivo não é acertar a mão exata, mas saber como a sua mão se sai contra tudo o que ele pode ter.
