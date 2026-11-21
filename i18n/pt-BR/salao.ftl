# Strings only the Salão visual style shows (D19).

## Dates (the home screen's "Sexta-feira, 20 de novembro")

salao-date = { $weekday }, { $day } de { $month }
salao-weekday-1 = Segunda-feira
salao-weekday-2 = Terça-feira
salao-weekday-3 = Quarta-feira
salao-weekday-4 = Quinta-feira
salao-weekday-5 = Sexta-feira
salao-weekday-6 = Sábado
salao-weekday-7 = Domingo
salao-month-1 = janeiro
salao-month-2 = fevereiro
salao-month-3 = março
salao-month-4 = abril
salao-month-5 = maio
salao-month-6 = junho
salao-month-7 = julho
salao-month-8 = agosto
salao-month-9 = setembro
salao-month-10 = outubro
salao-month-11 = novembro
salao-month-12 = dezembro

## Home

salao-home-lessons-total = { $done } de { $total } lições no total
salao-home-due-today = para hoje · cerca de { $minutes } min
salao-home-due-none = nada para hoje
salao-home-review-start = Começar revisão
salao-seconds = s
salao-home-calc-pace = tempo médio · { $percent }% de acerto
salao-home-calc-none = nenhuma resposta ainda
salao-home-table-week = Mesa · esta semana
salao-home-decisions = { $count ->
    [1] decisão
   *[other] decisões
}
salao-home-table-good = { $percent }% melhor ou correta
salao-home-table-none = nenhuma decisão esta semana
salao-home-play = Jogar
salao-home-leaks-sub = onde suas decisões avaliadas mais perdem EV
salao-leak-stats = { $mistakes ->
    [1] { $mistakes } erro
   *[other] { $mistakes } erros
} em { $decisions ->
    [1] { $decisions } decisão
   *[other] { $decisions } decisões
}
salao-bb = bb

## Lessons

salao-lessons-of = de { $total ->
    [1] 1 lição
   *[other] { $total } lições
}
salao-of = de { $total }
salao-track-current = Em andamento
salao-track-locked = { $count ->
    [1] 1 lição
   *[other] { $count } lições
} · fechadas por enquanto.
salao-lesson-next = próxima
salao-lesson-continue = Continuar
salao-lesson-quiz = quiz { $score }
salao-track-open = { $count ->
    [1] 1 lição
   *[other] { $count } lições
}

## Table

salao-pot = Pote
salao-you = você
salao-seat-you = { $action } · você
salao-ev-loss = Perda de EV
salao-options = Ações neste ponto
salao-options-note = frequência da referência · EV
salao-options-note-profile = frequência · EV contra o bot
salao-option-yours = sua escolha
salao-option-best = melhor EV
salao-verdict-with = { $action } com
salao-repeat-spot = Repetir este spot
salao-session = Sessão · { $count ->
    [1] 1 decisão
   *[other] { $count } decisões
}
salao-session-ev = EV perdido { $amount } bb
