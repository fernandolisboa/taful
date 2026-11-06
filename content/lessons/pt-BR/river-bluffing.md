+++
id = "river-bluffing"
version = 1
title = "Blefando no river"
prerequisites = ["river-value", "fold-equity-101"]
tags = ["postflop"]
glossary = ["bluff", "blocker", "alpha", "polarized"]
unlock = { quiz = "river-bluffing-quiz" }
minutes = 8
+++

# Blefando no river

Um [blefe](term:bluff) no river só ganha quando o adversário folda. Não sobra equity para salvar.

## Quantos blefes

Uma aposta [polarizada](term:polarized) no river deve deixar os bluff-catchers do adversário indiferentes entre pagar e foldar. Com uma aposta do tamanho do pote quem paga recebe 2 para 1, então mais ou menos um blefe para cada duas apostas de valor deixa o call no zero a zero. Com meio pote, mais ou menos um blefe para cada três apostas de valor.

O fold de que o próprio blefe precisa é o [alpha](term:alpha): {{ calc: alpha(pot=1, bet=1) | pct(0) }} para uma aposta do tamanho do pote, {{ calc: alpha(pot=2, bet=1) | pct(0) }} para meio pote.

## Escolhendo os blefes

Escolha os blefes entre as mãos que não ganham no showdown:

- **Projetos que não fecharam** são blefes naturais: não têm valor de showdown.
- **[Blockers](term:blocker) importam.** Ter uma carta de que o adversário precisa para pagar tira combos de call. Blefar com o ás do naipe numa mesa com três cartas do mesmo naipe tira o flush nuts do range dele.
- **Não bloqueie os folds dele.** Mãos com as cartas dos projetos que o adversário não fechou são piores blefes: esses projetos já iam foldar.

## Não blefe com valor de showdown

Uma mão que às vezes ganha no showdown (um par fraco, ás alto) normalmente deve dar check. Transformá-la em blefe faz foldar as mãos piores que ela já ganha.

## Tendências da população

Nos limites baixos muitos jogadores pagam demais no river. Contra eles, blefe menos e aposte mais por valor. Contra jogadores que foldam demais, blefe mais. A estratégia de referência do Taful é equilibrada; use-a como base e ajuste com cuidado.
