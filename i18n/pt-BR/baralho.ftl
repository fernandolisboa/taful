# Textos que só o estilo visual Baralho mostra (D19).

## Datas

baralho-date = { $weekday }, { $day } de { $month }
baralho-weekday-1 = Segunda
baralho-weekday-2 = Terça
baralho-weekday-3 = Quarta
baralho-weekday-4 = Quinta
baralho-weekday-5 = Sexta
baralho-weekday-6 = Sábado
baralho-weekday-7 = Domingo
baralho-month-1 = janeiro
baralho-month-2 = fevereiro
baralho-month-3 = março
baralho-month-4 = abril
baralho-month-5 = maio
baralho-month-6 = junho
baralho-month-7 = julho
baralho-month-8 = agosto
baralho-month-9 = setembro
baralho-month-10 = outubro
baralho-month-11 = novembro
baralho-month-12 = dezembro

## Início

baralho-home-title = Sua mão de hoje
baralho-home-body = Quatro cartas na mesa. Por qual você começa?
baralho-home-continue = Continuar · { $track }
baralho-home-lesson-body = Lição { $n } de { $total }. Uns { $minutes } min de leitura e um quiz.
baralho-home-lesson-body-plain = Lição { $n } de { $total }. Uma leitura e um quiz.
baralho-home-lesson-go = Continuar lição
baralho-home-lessons-done = Lições
baralho-home-lessons-done-title = Tudo lido
baralho-home-all-lessons = Ver as lições
baralho-home-review = Revisar
baralho-home-review-title =
    { $count ->
        [1] 1 revisão
       *[other] { $count } revisões
    }
baralho-home-review-body = Uns { $minutes } min pra não esquecer o que já aprendeu.
baralho-home-review-tomorrow = Amanhã tem { $count }.
baralho-home-review-go = Começar revisão
baralho-home-review-none-title = Nada pra revisar
baralho-home-review-none-body = Sua fila está em dia.
baralho-home-practice = Praticar · seu maior vazamento
baralho-home-practice-nth = Praticar · { $n }º vazamento
baralho-home-leak-body =
    { $where }: { $mistakes ->
        [1] 1 erro
       *[other] { $mistakes } erros
    } em { $decisions ->
        [1] 1 decisão
       *[other] { $decisions } decisões
    } e { $loss } bb perdidos.
baralho-on-street-preflop = No preflop
baralho-on-street-flop = No flop
baralho-on-street-turn = No turn
baralho-on-street-river = No river
baralho-home-practice-none = Praticar
baralho-home-practice-none-title = Mesa de treino
baralho-home-practice-none-go = Ir pra mesa
baralho-home-calc = Calcular
baralho-home-calc-body = Média de { $secs } s por conta, com { $accuracy }% de acerto. O alvo é { $target } s.
baralho-home-tracks = Sua jornada · { $done } de { $total } lições
baralho-home-all-lessons-link = Ver todas as lições →
baralho-track-done = Concluída
baralho-track-open = Em andamento
baralho-track-locked = Libera conforme você avança

## Lições

baralho-lessons-body = Cada nível é um naipe. Lição concluída vira carta aberta; as fechadas liberam conforme você avança.
baralho-lessons-body-themes = Cada tema é um naipe. Lição concluída vira carta aberta; as fechadas liberam conforme você avança.
baralho-lessons-total = de { $total } lições
baralho-lessons-count = { $done } de { $total }
baralho-lessons-track-done = concluída
baralho-lessons-track-open = em andamento
baralho-lessons-track-ready = aberta
baralho-lessons-track-locked = fechada
baralho-lessons-next-meta = { $minutes } min · lição e quiz
baralho-lessons-next-meta-plain = lição e quiz
baralho-lessons-continue = Continuar

## Mesa

baralho-pot = Pote
baralho-you = você
baralho-seat-you = { $action } · você
baralho-repeat-spot = Repetir este spot
baralho-repeat-note = Você está repetindo este spot: a nova resposta não entra no seu histórico.
baralho-turn-title = Sua vez
baralho-turn-body = Escolha uma ação embaixo da mesa.
baralho-replay-title = Replay
baralho-over-title = Fim da mão
baralho-over-body = A mão acabou antes de chegar a sua vez.
baralho-result-with = { $action } com
baralho-ev-loss = Perda de EV
baralho-actions-here = Ações neste ponto
baralho-actions-note = frequência da referência · EV
baralho-actions-note-profile = frequência da referência · EV contra { $profile }
baralho-your-pick = sua escolha
baralho-session = Sessão · { $count ->
        [0] nenhuma decisão
        [1] 1 decisão
       *[other] { $count } decisões
    }
baralho-session-loss = EV perdido { $amount } bb
