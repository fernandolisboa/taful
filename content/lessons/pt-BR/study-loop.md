+++
id = "study-loop"
version = 1
title = "Estudando com o Taful"
prerequisites = ["gto-vs-exploit"]
tags = ["strategy"]
glossary = ["ev", "leak"]
unlock = { quiz = "study-loop-quiz" }
minutes = 6
+++

# Estudando com o Taful

O Taful roda um ciclo só: jogar, avaliar, revisar, achar leaks, treiná-los e voltar a eles mais tarde pela repetição espaçada.

## Jogue e seja avaliado

O modo spot treina uma decisão por vez. As mãos completas vão do preflop ao river contra os bots de referência. Toda decisão sua é avaliada pela perda de [EV](term:ev): o quanto a sua ação foi pior que a melhor, em big blinds.

## Revise a mão

Depois de uma mão completa, abra a revisão da mão. Avance pela mão, olhe cada decisão e a sua nota, e leia a explicação: frequências e EV de cada ação.

## Ache os seus [leaks](term:leak)

A tela inicial mostra os seus maiores leaks: grupos de spots (configuração, rodada, tipo de nó, textura) em que você perdeu mais EV recentemente. Use "Treinar isso" para praticar esse grupo direto.

## Repetição espaçada

Todo erro, toda resposta errada de quiz e toda resposta lenta de cálculo mental vira um item de revisão. O Taful agenda cada item para voltar pouco antes de você esquecer. Faça as revisões quando vencerem: alguns minutos por dia valem mais que uma sessão longa por semana.

## Uma rotina diária

1. Faça as revisões do dia.
2. Leia uma lição ou treine o maior leak.
3. Jogue algumas mãos completas e revise os maiores erros.

## Como medir o progresso

Olhe a sua perda média de EV por decisão ao longo de semanas, não o resultado de uma sessão. Os resultados oscilam com as cartas; a perda de EV mede as suas decisões.
