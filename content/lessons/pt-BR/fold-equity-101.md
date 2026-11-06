+++
id = "fold-equity-101"
version = 1
title = "Fold equity e semiblefes"
prerequisites = ["alpha-101", "ev-101"]
tags = ["math", "postflop"]
glossary = ["fold-equity", "semi-bluff", "alpha", "range"]
unlock = { quiz = "fold-equity-101-quiz" }
minutes = 7
+++

# Fold equity e semiblefes

Quando você aposta, pode ganhar de dois jeitos: o adversário folda agora, ou você ganha no showdown. A [fold equity](term:fold-equity) é o valor do primeiro.

## Blefes puros

Um blefe sem chance de ganhar no showdown precisa que o adversário folde pelo menos [alpha](term:alpha) das vezes. Um blefe de meio pote precisa de {{ calc: alpha(pot=2, bet=1) | pct(1) }} de folds; um blefe do tamanho do pote precisa de {{ calc: alpha(pot=1, bet=1) | pct(1) }}.

## Semiblefes

Um [semiblefe](term:semi-bluff) é uma aposta com uma mão que está atrás agora mas pode melhorar: um projeto de flush, um projeto de sequência aberto, cartas altas. Ele ganha quando o adversário folda, e ainda ganha parte das vezes quando é pago.

Esse segundo jeito de ganhar faz um semiblefe precisar de menos folds que um blefe puro. Um projeto de flush no flop tem cerca de {{ calc: hit_probability(outs=9, cards=2) | pct(0) }} de fechar até o river, então mesmo pago está longe de morto.

## Por que apostar um projeto pode ser melhor que dar check

- Dar check abre mão da fold equity.
- Apostar cresce um pote que você ganha grande quando acerta.
- As suas mãos fortes e os seus projetos ficam parecidos, então as suas apostas de valor são mais pagas.

## Quando a fold equity é baixa

A fold equity encolhe quando o [range](term:range) do adversário é forte ou grudento, quando o pote é multiway e quando a aposta é pequena demais para fazer algo foldar. Nesses spots, os projetos preferem dar check e realizar a equity barato.
