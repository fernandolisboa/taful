# Sessão de revisão (spec §13) e tela inicial.

review-due = { $count ->
    [0] Nada para revisar agora.
    [one] 1 item para revisar.
   *[other] { $count } itens para revisar.
}
review-start = Começar a revisão
review-next-at = Próxima revisão: { $when }
review-empty = Tudo em dia.
review-from-lesson = Da lição “{ $lesson }”
review-continue = Continuar
review-session-done = { $count ->
    [one] 1 revisão feita nesta sessão.
   *[other] { $count } revisões feitas nesta sessão.
}

home-due-title = Revisões
home-lesson-test = Você terminou as lições abertas. Faça a prova de { $level } em Lições para abrir o próximo nível.
home-lesson-none = Todas as lições concluídas. Mais lições chegam nos próximos marcos.
home-calc-title = Cálculo mental
home-calc-recent = { $correct } de { $total } certas nas suas últimas respostas
home-calc-start = Treinar
home-progress = { $done } de { $total } lições concluídas
home-leaks-title = Maiores vazamentos
home-leaks-body = Nenhum vazamento ainda. Eles aparecem quando as decisões avaliadas acumulam erros num tipo de spot.
review-dealing = Distribuindo um spot…

home-calc-short = Treine pot odds, outs e equity contra o relógio.
home-due-short = esperando por você agora

review-empty-body = Spots que você errar na mesa, quizzes errados e cálculos lentos entram aqui sozinhos.

review-go-calc = Treinar cálculo mental

# O próximo passo no Início (D27): revisões, depois a lição, depois a mesa.
next-step-label = Próximo passo
next-step-review = Revisões
next-step-table = Mesa
next-step-review-title = { $count ->
    [one] 1 revisão para hoje
   *[other] { $count } revisões para hoje
}
next-step-review-body = Cerca de { $minutes } min. O que você errou volta antes de esquecer.
next-step-review-go = Revisar agora
next-step-lesson-place = Lição { $n } de { $total }
next-step-lesson-minutes = { $minutes } min de leitura e um quiz
next-step-lesson-go = Começar lição
next-step-lesson-continue = Continuar lição
next-step-table-title = Treine na mesa
next-step-table-after = Pratique o que viu em “{ $lesson }”: { $practice }.
next-step-table-plain = Pratique { $practice }.
next-step-table-test = Quando quiser, a prova de { $level } abre o próximo nível.
next-step-table-go = Ir para a mesa
next-step-practice-hand = mãos completas contra os bots
next-step-practice-postflop = decisões depois do flop
next-step-practice-rfi = abrir o pote quando todos foldam até você
next-step-practice-vs-open = responder a um aumento antes do flop
next-step-practice-squeeze = agir depois de um aumento e um call
next-step-practice-vs-3bet = responder a um 3-bet
next-step-practice-vs-4bet = responder a um 4-bet
next-step-practice-vs-allin = responder a um all-in antes do flop
next-step-practice-bvb = jogar small blind contra big blind
