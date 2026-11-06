+++
id = "bet-sizing"
version = 1
title = "Tamanho de aposta"
prerequisites = ["cbet-101", "alpha-101"]
tags = ["postflop", "math"]
glossary = ["polarized", "linear", "alpha", "pot-odds"]
unlock = { quiz = "bet-sizing-quiz" }
minutes = 8
+++

# Tamanho de aposta

Cada tamanho de aposta faz uma troca entre o que você arrisca, o que pode ganhar e o que o adversário precisa para continuar.

## O que cada tamanho pede

Uma aposta pequena dá boas [pot odds](term:pot-odds) e precisa de poucos folds:

- **Um terço do pote:** quem paga precisa de {{ calc: pot_odds(pot=3, bet=1) | pct(1) }} de equity; um blefe precisa de {{ calc: alpha(pot=3, bet=1) | pct(1) }} de folds.
- **Meio pote:** quem paga precisa de {{ calc: pot_odds(pot=2, bet=1) | pct(1) }}; um blefe precisa de {{ calc: alpha(pot=2, bet=1) | pct(1) }}.
- **Pote:** quem paga precisa de {{ calc: pot_odds(pot=1, bet=1) | pct(1) }}; um blefe precisa de {{ calc: alpha(pot=1, bet=1) | pct(1) }}.

## Apostas pequenas: ranges lineares

Apostas pequenas combinam com um range [linear](term:linear): muitas mãos de valor médio e forte, mais alguns blefes. Funcionam quando você tem vantagem de range mas poucas mãos que querem jogar pelo stack, e quando o range do adversário tem muitas mãos fracas que precisam foldar.

## Apostas grandes: ranges polarizados

Apostas grandes combinam com um range [polarizado](term:polarized): mãos muito fortes e blefes, sem nada no meio. Mãos médias não podem apostar grande: só mãos melhores pagam. Apostas grandes funcionam quando você tem a vantagem de nuts.

## Tamanhos ao longo das rodadas

Planeje os tamanhos para os stacks entrarem até o river quando você quiser. Um plano comum num pote de raise simples é um terço ou meio pote no flop, cerca de três quartos no turn e pote ou mais no river.

## No Taful

A biblioteca pós-flop oferece um conjunto fixo de tamanhos por spot, os tamanhos que o solver usou. Um tamanho fora do menu não é opção no treino. Observe quais tamanhos a estratégia de referência mais usa em cada textura.
