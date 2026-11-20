# Strings only the Cockpit visual style shows (D19).

## Icon rail (short labels) and status bar

cockpit-nav-calc = Cálculo
cockpit-nav-review = Revisões
cockpit-nav-hands = Mãos
cockpit-nav-settings = Config.
cockpit-bar-today = Hoje
cockpit-bar-decisions = { $count ->
    [1] decisão
   *[other] decisões
}
cockpit-bar-accuracy = Precisão
cockpit-bar-ev-lost = EV perdido
cockpit-bar-last = Últimas
cockpit-bb = { $value } bb
cockpit-percent = { $value }%
cockpit-none = –
cockpit-date = { $weekday }, { $day } { $month }
cockpit-weekday-0 = segunda
cockpit-weekday-1 = terça
cockpit-weekday-2 = quarta
cockpit-weekday-3 = quinta
cockpit-weekday-4 = sexta
cockpit-weekday-5 = sábado
cockpit-weekday-6 = domingo
cockpit-month-1 = jan
cockpit-month-2 = fev
cockpit-month-3 = mar
cockpit-month-4 = abr
cockpit-month-5 = mai
cockpit-month-6 = jun
cockpit-month-7 = jul
cockpit-month-8 = ago
cockpit-month-9 = set
cockpit-month-10 = out
cockpit-month-11 = nov
cockpit-month-12 = dez
cockpit-lessons-crumb = { $tracks ->
    [1] 1 trilha
   *[other] { $tracks } trilhas
} · { $lessons ->
    [1] 1 lição
   *[other] { $lessons } lições
}

## Home: the four numbers at the top

cockpit-kpi-reviews = Revisões hoje
cockpit-kpi-reviews-minutes = ~{ $minutes } min
cockpit-kpi-reviews-ahead = amanhã { $tomorrow } · 7 dias { $week }
cockpit-review-go = Revisar
cockpit-day-today = H
cockpit-day-0 = S
cockpit-day-1 = T
cockpit-day-2 = Q
cockpit-day-3 = Q
cockpit-day-4 = S
cockpit-day-5 = S
cockpit-day-6 = D
cockpit-kpi-accuracy = Precisão · 7 dias
cockpit-kpi-points = { $points } pts
cockpit-kpi-vs-last-week = vs. semana anterior
cockpit-kpi-no-trend = sem semana anterior para comparar
cockpit-kpi-ev = EV perdido por decisão
cockpit-kpi-calc = Cálculo mental · tempo médio
cockpit-kpi-calc-detail = { $accuracy }% de acerto · alvo { $target } s
cockpit-kpi-empty = sem dados ainda
cockpit-unit-bb = bb
cockpit-unit-seconds = s
cockpit-unit-percent = %

## Home: decisions chart and leaks

cockpit-chart-title = Decisões por nota · últimos 14 dias
cockpit-chart-empty = As decisões avaliadas na mesa aparecem aqui, dia a dia.
cockpit-leaks-title = Maiores vazamentos
cockpit-leaks-by = por EV perdido
cockpit-leaks-spot = Spot
cockpit-leaks-mistakes = Erros
cockpit-leaks-bb = bb perdidos
cockpit-leaks-train = Treinar

## Home: tracks, preflop deviation and shortcuts

cockpit-tracks-title = Trilhas
cockpit-next-lesson = Próxima lição
cockpit-next-meta = { $track } · { $n } de { $total } · { $minutes } min
cockpit-continue = Continuar
cockpit-dev-title = Preflop · onde você desvia da referência
cockpit-dev-count = { $count ->
    [1] 1 decisão
   *[other] { $count } decisões
}
cockpit-dev-top = Mais desviadas
cockpit-dev-none = Nenhum desvio relevante até agora.
cockpit-dev-same = igual
cockpit-dev-far = desvia
cockpit-dev-too-much = { $play } demais
cockpit-dev-too-little = { $play } de menos
cockpit-play-fold = fold
cockpit-play-call = call
cockpit-play-open = open
cockpit-play-threebet = 3-bet
cockpit-play-fourbet = 4-bet
cockpit-play-jam = all-in
cockpit-play-raise = raise
cockpit-shortcuts-title = Atalhos
cockpit-sc-spot = Mesa · Modo spot
cockpit-sc-spot-body = Decisões de preflop pelos charts
cockpit-sc-postflop = Mesa · Pós-flop
cockpit-sc-postflop-body = Flop e turn pela biblioteca resolvida
cockpit-sc-hand = Mesa · Mão completa
cockpit-sc-hand-body = Do preflop ao river contra os bots
cockpit-sc-calc = Cálculo mental
cockpit-sc-calc-body = Pot odds, outs e equity contra o relógio
cockpit-sc-hands = Revisão de mãos
cockpit-sc-hands-body = Repasse as últimas mãos jogadas
cockpit-sc-leak = Treinar o maior vazamento
cockpit-sc-leak-none = Nenhum vazamento ainda

## Lessons

cockpit-lessons-progress = Progresso geral
cockpit-lessons-done = Concluídas
cockpit-lessons-of = / { $total }
cockpit-lessons-time-left = Tempo restante
cockpit-lessons-quiz = Quiz médio
cockpit-unit-hours = h
cockpit-unit-minutes = min
cockpit-lesson-minutes = { $minutes } min
cockpit-lesson-minutes-next = { $minutes } min · próxima
cockpit-lesson-open = Abrir
