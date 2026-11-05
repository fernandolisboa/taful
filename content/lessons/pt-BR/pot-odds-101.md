+++
id = "pot-odds-101"
version = 1
title = "Pot odds"
prerequisites = ["rule-of-2-and-4"]
tags = ["math", "postflop"]
glossary = ["pot-odds", "equity"]
unlock = { quiz = "pot-odds-101-quiz" }
minutes = 7
+++

# Pot odds

Quando você enfrenta uma aposta, o pote oferece um preço. As [pot odds](term:pot-odds) transformam esse preço na [equity](term:equity) de que você precisa para pagar sem perder dinheiro.

## A fórmula

Call / (pote depois do seu call) = aposta / (pote + 2 × aposta), em que o pote é o que havia antes da aposta.

Contra uma aposta de meio pote você precisa de {{ calc: pot_odds(pot=100, bet=50) | pct(1) }} de equity para pagar. Contra uma aposta do tamanho do pote, {{ calc: pot_odds(pot=100, bet=100) | pct(1) }}. Contra um terço do pote, só {{ calc: pot_odds(pot=90, bet=30) | pct(1) }}.

## Juntando tudo

No turn você tem um projeto de flush: 9 outs, cerca de {{ calc: rule_of_2_and_4(outs=9, cards=1) | pct(0) }} de acertar no river. O pote tem 10 bb e o adversário aposta 5 bb. Você precisa de {{ calc: pot_odds(pot=10, bet=5) | pct(1) }}, então pagar direto perde: o seu valor esperado é {{ calc: call_ev(equity=0.196, pot=10, bet=5) | bb(2) }} se você não ganhar nada a mais quando acertar.

É aí que entram as odds implícitas: as fichas que você espera ganhar nas rodadas seguintes quando acerta. Apostas pequenas dão boas pot odds; apostas grandes pedem mais equity ou muitas odds implícitas.

## Referência rápida

- Um terço do pote: {{ calc: pot_odds(pot=3, bet=1) | pct(1) }}
- Meio pote: {{ calc: pot_odds(pot=2, bet=1) | pct(1) }}
- Dois terços do pote: {{ calc: pot_odds(pot=3, bet=2) | pct(1) }}
- Pote: {{ calc: pot_odds(pot=1, bet=1) | pct(1) }}
