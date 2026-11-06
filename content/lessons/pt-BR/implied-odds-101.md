+++
id = "implied-odds-101"
version = 1
title = "Odds implícitas"
prerequisites = ["ev-101"]
tags = ["math", "postflop"]
glossary = ["implied-odds", "pot-odds", "dirty-outs"]
unlock = { quiz = "implied-odds-101-quiz" }
minutes = 7
+++

# Odds implícitas

As [pot odds](term:pot-odds) só contam as fichas que estão no meio agora. As [odds implícitas](term:implied-odds) somam as fichas que você espera ganhar nas próximas rodadas quando acerta.

## Quanto mais você precisa?

No turn você tem um projeto de flush: 9 outs, {{ calc: hit_probability(outs=9, cards=1) | pct(1) }} de acertar no river. O pote tem 10 bb e o adversário aposta 5 bb. O call sozinho vale {{ calc: call_ev(equity=0.196, pot=10, bet=5) | bb(2) }}.

Para empatar você precisa recuperar o que falta quando acertar: cerca de {{ calc: div(a=1.08, b=0.196) | bb(1) }} a mais, em média, no river. Se o adversário costuma pagar uma aposta desse tamanho no river, o call está bom.

## Quando as odds implícitas são boas

- **Os stacks são fundos.** Precisa sobrar dinheiro para ganhar.
- **O seu projeto é escondido.** Um gutshot para sequência numa mesa rainbow é mais difícil de ver do que uma terceira carta do mesmo naipe.
- **Você busca o nuts.** Acertar um flush fraco que perde para um flush melhor custa dinheiro em vez de ganhar.
- **O adversário tem uma mão forte** e não vai largá-la.

## Odds implícitas reversas

Algumas mãos ganham um pote pequeno quando estão à frente e perdem um grande quando estão atrás: top pairs fracos, flushes baixos, sequências pela ponta de baixo. Os [outs sujos](term:dirty-outs) e as segundas melhores mãos pagam nas próximas rodadas. Conte esses custos também, e prefira foldar ou controlar o pote com elas.

## Regra prática

Odds implícitas justificam calls com projetos que estão um pouco abaixo do preço, não calls muito longe dele. Uma aposta pequena deixa um projeto fácil de pagar; uma aposta do tamanho do pote normalmente não.
