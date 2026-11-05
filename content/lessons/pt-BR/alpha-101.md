+++
id = "alpha-101"
version = 1
title = "Blefe: alpha"
prerequisites = ["mdf-101"]
tags = ["math", "theory"]
glossary = ["alpha", "mdf"]
unlock = { quiz = "alpha-101-quiz" }
minutes = 5
+++

# Blefe: alpha

Quando você blefa, ganha o pote toda vez que o adversário folda e perde a aposta quando ele paga. O [alpha](term:alpha) é a frequência de fold de que o blefe precisa para empatar.

## A fórmula

Alpha = aposta / (pote + aposta).

- Um blefe de um terço do pote precisa de {{ calc: alpha(pot=3, bet=1) | pct(0) }} de folds.
- Um blefe de meio pote precisa de {{ calc: alpha(pot=2, bet=1) | pct(1) }} de folds.
- Um blefe do tamanho do pote precisa de {{ calc: alpha(pot=1, bet=1) | pct(0) }} de folds.

## Alpha e MDF são dois lados da mesma moeda

Alpha + [MDF](term:mdf) = 100%. Se o seu blefe de meio pote precisa que o adversário folde {{ calc: alpha(pot=2, bet=1) | pct(1) }} das vezes, ele precisa defender {{ calc: mdf(pot=2, bet=1) | pct(1) }} para impedir.

## Só para blefes puros

O alpha supõe que o blefe nunca ganha quando é pago. Um semiblefe com projeto também ganha algumas das vezes em que é pago, então precisa de menos folds do que o alpha diz. Por isso projetos são os melhores blefes.
