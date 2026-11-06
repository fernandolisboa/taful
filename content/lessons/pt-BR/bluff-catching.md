+++
id = "bluff-catching"
version = 1
title = "Pagando blefes (bluff-catching)"
prerequisites = ["river-bluffing", "mdf-101"]
tags = ["postflop"]
glossary = ["bluff-catcher", "mdf", "pot-odds", "blocker"]
unlock = { quiz = "bluff-catching-quiz" }
minutes = 7
+++

# Pagando blefes (bluff-catching)

Um [bluff-catcher](term:bluff-catcher) é uma mão que só ganha dos blefes do adversário. Perde para toda aposta de valor e ganha de todo blefe.

## O preço

Contra uma aposta do tamanho do pote no river, você recebe 2 para 1: precisa ganhar {{ calc: pot_odds(pot=1, bet=1) | pct(1) }} das vezes. Se quem aposta blefa mais do que isso, pagar dá lucro. Contra meio pote, você só precisa de {{ calc: pot_odds(pot=2, bet=1) | pct(1) }}.

## Quantos pagar

A [frequência mínima de defesa](term:mdf) contra uma aposta do tamanho do pote é {{ calc: mdf(pot=1, bet=1) | pct(0) }} do seu range. Abaixo disso, quem aposta lucra blefando com quaisquer duas cartas.

Mas o MDF é sobre o range inteiro, e no river o seu range é o que é. Se o seu range é fraco por causa de como você jogou, foldar mais que o MDF pode ser certo, e um bom adversário vai apostar mais contra você.

## Escolhendo os bluff-catchers certos

Entre mãos de força parecida, pague com as que:

- **[bloqueiam](term:blocker) apostas de valor:** ter uma carta das melhores mãos do adversário significa menos combos de valor;
- **não bloqueiam blefes:** ter uma carta dos projetos que o adversário não fechou significa menos blefes, então pague com outras mãos.

## Exploits

Contra jogadores que raramente blefam no river, folde mais bluff-catchers. Contra jogadores que blefam demais, pague mais. A base vem da matemática; o ajuste vem do que você sabe sobre o jogador.
