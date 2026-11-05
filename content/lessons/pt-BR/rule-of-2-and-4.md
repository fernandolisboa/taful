+++
id = "rule-of-2-and-4"
version = 1
title = "A regra do 2 e do 4"
prerequisites = ["outs-101"]
tags = ["math", "postflop"]
glossary = ["outs"]
unlock = { quiz = "rule-of-2-and-4-quiz" }
minutes = 5
+++

# A regra do 2 e do 4

Depois de contar os [outs](term:outs), transforme-os em probabilidade:

- Com **uma carta** por vir (do turn para o river), multiplique os outs por **2**.
- Com **duas cartas** por vir (do flop ao river, all-in), multiplique os outs por **4**.

## Quão perto chega?

Um projeto de flush no flop tem 9 outs. A regra diz {{ calc: rule_of_2_and_4(outs=9, cards=2) | pct(0) }} de acertar até o river; a chance exata é {{ calc: hit_probability(outs=9, cards=2) | pct(1) }}. No turn a regra diz {{ calc: rule_of_2_and_4(outs=9, cards=1) | pct(0) }}; a chance exata é {{ calc: hit_probability(outs=9, cards=1) | pct(1) }}.

## A correção acima de 8 outs

Com muitos outs, o ×4 superestima. Tire um ponto para cada out acima de oito. Projeto de flush mais sequência aberta tem 15 outs: 15 × 4 = 60, menos 7, dá {{ calc: rule_of_2_and_4(outs=15, cards=2) | pct(0) }}. O valor exato é {{ calc: hit_probability(outs=15, cards=2) | pct(1) }}.

> O ×4 só vale quando você vai ver as duas cartas, normalmente porque alguém está all-in. Se você vai enfrentar outra aposta no turn, use ×2 só para a próxima carta.
