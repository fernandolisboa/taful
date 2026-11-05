+++
id = "outs-101"
version = 1
title = "Contando outs"
prerequisites = ["equity-101"]
tags = ["math", "postflop"]
glossary = ["outs", "dirty-outs"]
unlock = { quiz = "outs-101-quiz" }
minutes = 7
+++

# Contando outs

Um [out](term:outs) é uma carta que, se vier a seguir, coloca você na frente. Contar outs é o primeiro passo de toda decisão com projeto.

## Projetos comuns

- **Projeto de flush:** quatro cartas de um naipe, nove restantes no baralho: 9 outs.
- **Sequência aberta:** 9-8 em 7-6-2 completa com qualquer dez ou cinco: 8 outs.
- **Gutshot:** um único valor completa a sequência: 4 outs.
- **Duas overcards:** cada uma pode parear, 3 cartas de cada valor: 6 outs, mas muitas vezes fracos.

## Outs sujos

Nem todo out é limpo. Com A♥ J♥ em K♥ 7♥ 2♦ contra uma trinca de reis, nove copas fazem o seu flush, mas o 2♥ pareia a mesa e dá full house à trinca. Contra essa mão exata você tem {{ calc: outs(hero="AhJh", villain="KsKc", board="Kh7h2d") | int }} outs, não 9. Cartas como o 2♥ são [outs sujos](term:dirty-outs).

O mesmo projeto contra uma mão como K♠ Q♣ é bem mais limpo: agora os ases também ganham, então você tem {{ calc: outs(hero="AhJh", villain="KsQc", board="Kh7h2d") | int }} outs.

## Conte e desconte

Na mesa você não sabe a mão exata. Conte os outs pelo tipo de projeto e depois tire os que provavelmente estão sujos: cartas que pareiam a mesa ou completam um projeto melhor do adversário.
