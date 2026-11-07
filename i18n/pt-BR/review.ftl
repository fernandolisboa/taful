# Sessão de revisão (spec §13) e tela inicial.

review-intro = Sua fila de repetição espaçada: spots que você errou, perguntas de quiz erradas e cálculos lentos ou errados, numa sessão só.
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
home-lesson-none = Todas as lições concluídas. Mais lições chegam nos próximos marcos.
home-calc-title = Cálculo mental
home-calc-recent = { $correct } de { $total } certas nas suas últimas respostas
home-calc-none = Nenhuma resposta ainda.
home-calc-start = Treinar
home-progress = { $done } de { $total } lições concluídas
home-leaks-title = Maiores vazamentos
home-leaks-body = Nenhum vazamento ainda. Eles aparecem quando as decisões avaliadas acumulam erros num tipo de spot.
review-dealing = Distribuindo um spot…

home-continue-title = Continue de onde parou
home-start-lesson = Começar lição
home-calc-short = Treine pot odds, outs e equity contra o relógio.
home-due-short = { $count } para revisar
