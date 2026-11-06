+++
id = "blind-defense"
version = 1
title = "Defendendo o big blind"
prerequisites = ["facing-an-open"]
tags = ["preflop"]
glossary = ["pot-odds", "big-blind", "three-bet"]
unlock = { quiz = "blind-defense-quiz" }
minutes = 7
+++

# Defendendo o big blind

O [big blind](term:big-blind) já pagou 1 bb, fecha a ação antes do flop e tem o melhor preço da mesa. Por isso ele defende mais mãos do que qualquer outra posição.

## O preço

O botão abre para 2,5 bb e o small blind folda. O big blind paga 1,5 bb para ganhar um pote de 4 bb, então precisa de {{ calc: div(a=1.5, b=5.5) | pct(1) }} de equity no showdown. Quase quaisquer duas cartas têm isso contra um range de 39% do botão, mas jogar fora de posição custa parte disso.

## Quanto defender

O chart de referência do Taful no big blind:

- **Contra um open do botão:** folda cerca de 40%, paga cerca de 47%, dá [3-bet](term:three-bet) em cerca de 13%.
- **Contra um open do cutoff:** folda cerca de 52%.
- **Contra um open do UTG:** folda cerca de 68%. Um range fechado merece respeito.
- **Contra um open do small blind (3 bb):** folda só cerca de 20%.

## Com o que defender

- **Mãos suited de quase todo tipo:** fazem flushes, projetos e backdoors.
- **Mãos offsuit conectadas:** T9o, 98o, 87o contra opens tardios.
- **Mãos offsuit fracas como K4o ou Q3o** são fold contra tudo menos os opens mais largos: fazem top pairs dominados.

## 3-bet ou call?

No big blind, dê 3-bet para cerca de 9,5 bb com mãos de valor e um conjunto polarizado de blefes (ases suited, alguns reis suited). Pague com o resto do range de defesa. Como o small blind joga raise ou fold, o big blind muitas vezes enfrenta o small blind sozinho: defenda largo aí.

## Depois do flop

Você está fora de posição, então não vai ganhar todo pote que poderia. Defenda com mãos que conseguem realizar equity, e não se sinta obrigado a ganhar todo flop: dê check e folde quando errar.
