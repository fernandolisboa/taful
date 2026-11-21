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
cockpit-lessons-crumb = { $view ->
    [themes] { $groups ->
        [1] 1 tema
       *[other] { $groups } temas
    }
   *[journey] { $groups ->
        [1] 1 nível
       *[other] { $groups } níveis
    }
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

cockpit-tracks-title = Jornada
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

## Table

cockpit-act-fold = fold
cockpit-act-check = check
cockpit-act-call = call { $amount }
cockpit-act-bet = bet { $amount }
cockpit-act-open = open { $amount }
cockpit-act-nbet = { $n }-bet { $amount }
cockpit-act-raise = raise { $amount }
cockpit-act-allin = all-in { $amount }
cockpit-act-pot-share = { $percent }% pote
cockpit-you = você
cockpit-pod-turn = sua vez
cockpit-pot = Pote
cockpit-table-crumb-spot = spot { $n }
cockpit-table-crumb-hand = mão { $n }
cockpit-sel-stack = Stack
cockpit-key-space = Espaço
cockpit-repeat = Repetir
cockpit-table-turn = { $street } · sua vez no { $position }
cockpit-table-replay = { $street } · replay
cockpit-table-wait = { $street } · aguardando
cockpit-table-over = Fim da mão
cockpit-table-pot = POTE { $amount } bb
cockpit-table-spr = SPR { $value }
cockpit-table-odds = ODDS { $ratio } · { $percent }%
cockpit-line-title = Linha da mão
cockpit-eval-title = Avaliação
cockpit-eval-vs-reference = contra a estratégia de referência
cockpit-eval-loss = Perda de EV
cockpit-eval-ev = EV da ação
cockpit-col-freq = Frequência com { $hand }
cockpit-col-ev = EV
cockpit-col-delta = Δ EV
cockpit-range-title = Range do { $position } neste ponto
cockpit-range-title-none = Range neste ponto
cockpit-range-note = estratégia de referência por mão
cockpit-range-all = Range inteiro
cockpit-range-yours = sua mão
cockpit-equity = Equity
cockpit-equity-vs = vs. range do { $position }
cockpit-equity-needs = precisa (pot odds)
cockpit-kind-raise = Raise
cockpit-kind-bet = Bet
cockpit-kind-call = Call
cockpit-kind-check = Check
cockpit-kind-fold = Fold
cockpit-hand-straight-flush = straight flush
cockpit-hand-quads = quadra
cockpit-hand-full-house = full house
cockpit-hand-flush = flush
cockpit-hand-straight = sequência
cockpit-hand-set = trinca (set)
cockpit-hand-trips = trinca
cockpit-hand-two-pair = dois pares
cockpit-hand-overpair = overpair
cockpit-hand-top-pair = top pair
cockpit-hand-middle-pair = par do meio
cockpit-hand-bottom-pair = par baixo
cockpit-hand-pocket-pair = par na mão
cockpit-hand-underpair = par abaixo do board
cockpit-hand-ace-high = ás alto
cockpit-hand-overcards = duas overcards
cockpit-hand-high-card = carta alta
cockpit-hand-combo-draw = draw combinado
cockpit-hand-flush-draw = draw de flush
cockpit-hand-open-ended = draw aberto de sequência
cockpit-hand-gutshot = gutshot
cockpit-hand-suited-ace = ás suited
cockpit-hand-suited-broadways = broadways suited
cockpit-hand-suited-connectors = conectores suited
cockpit-hand-suited-gapper = suited com um gap
cockpit-hand-suited = suited
cockpit-hand-offsuit-ace = ás offsuit
cockpit-hand-offsuit-broadways = broadways offsuit
cockpit-hand-offsuit-connectors = conectores offsuit
cockpit-hand-offsuit = offsuit
