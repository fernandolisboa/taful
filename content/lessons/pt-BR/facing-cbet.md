+++
id = "facing-cbet"
version = 1
title = "Enfrentando um c-bet"
prerequisites = ["cbet-101", "mdf-101"]
tags = ["postflop"]
glossary = ["c-bet", "mdf", "pot-odds", "range"]
unlock = { quiz = "facing-cbet-quiz" }
minutes = 7
+++

# Enfrentando um c-bet

Você pagou antes do flop e quem deu raise dá [c-bet](term:c-bet). Foldar, pagar ou aumentar?

## Quanto defender

A [frequência mínima de defesa](term:mdf) dá uma primeira resposta. Contra uma aposta de um terço do pote você precisa continuar com cerca de {{ calc: mdf(pot=3, bet=1) | pct(0) }} do seu [range](term:range); contra meio pote, {{ calc: mdf(pot=2, bet=1) | pct(0) }}; contra o pote, {{ calc: mdf(pot=1, bet=1) | pct(0) }}.

O MDF é um guia, não uma lei: se o range de quem aposta é muito forte, foldar mais está certo; se ele aposta demais, defenda mais.

## O preço

Contra uma aposta de um terço do pote as suas [pot odds](term:pot-odds) são {{ calc: pot_odds(pot=3, bet=1) | pct(1) }}. Mãos como projetos backdoor, gutshots e pares fracos recebem um preço que apostas pequenas não conseguem negar. Por isso c-bets pequenos são muito pagos.

## Com o que pagar

- Pares de quase todo tipo contra apostas pequenas.
- Projetos: de flush, de sequência aberta, e gutshots com cartas altas.
- Ás alto com projetos backdoor contra apostas pequenas em mesas secas.

## Com o que aumentar

Aumente com um conjunto polarizado de mãos: mãos feitas fortes (dois pares, trincas) e bons projetos que podem melhorar. Não aumente pares médios: mãos melhores continuam, mãos piores foldam.

## O que foldar

Mãos sem par, sem projeto e sem backdoors, principalmente contra apostas maiores.

## Em posição e fora de posição

Em posição, pague mais: você vê a próxima ação do adversário antes de decidir. Fora de posição, dê um pouco mais de check-raise e pague um pouco menos, porque as mãos que só pagam têm dificuldade para realizar a equity.
