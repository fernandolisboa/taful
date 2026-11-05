# Treino de cálculo mental (spec §11).

calc-intro = Exercícios cronometrados. Responda rápido: certo dentro do tempo-alvo vale Bom, dentro da metade vale Fácil. Respostas erradas ou lentas vão para a sua fila de revisão.
calc-mixed = Misto
calc-kind-outs = Outs
calc-kind-rule_of_2_and_4 = Regra do 2 e do 4
calc-kind-pot_odds = Pot odds
calc-kind-mdf = MDF
calc-kind-alpha = Alpha
calc-kind-spr = SPR
calc-kind-equity_vs_range = Equity contra range

calc-prompt-outs = Você tem { $hero } contra { $villain } em { $board }. Quantos outs você tem?
calc-prompt-rule_of_2_and_4 = { $outs } outs, { $cards ->
    [one] uma carta
   *[other] duas cartas
} por vir. Chance de acertar, em %?
calc-prompt-pot_odds = Pote { $pot } bb, o adversário aposta { $bet } bb. Equity necessária para pagar, em %?
calc-prompt-mdf = O adversário aposta { $pct }% do pote. Frequência mínima de defesa, em %?
calc-prompt-alpha = Você blefa { $pct }% do pote. Folds necessários para empatar, em %?
calc-prompt-spr = Stack efetivo { $stack } bb, pote { $pot } bb. SPR?
calc-prompt-equity_vs_range = Você tem { $hero } em { $board }. Range do adversário: { $range }. Sua equity, em %?

calc-shortcut-outs = Atalho: conte pelo tipo de projeto (flush 9, sequência aberta 8, gutshot 4, overcards 3 cada) e tire as cartas que pareiam a mesa.
calc-shortcut-rule_of_2_and_4 = Atalho: outs × 2 com uma carta por vir, outs × 4 com duas (menos um por out acima de 8).
calc-shortcut-pot_odds = Fórmula: aposta / (pote + 2 × aposta).
calc-shortcut-mdf = Fórmula: pote / (pote + aposta).
calc-shortcut-alpha = Fórmula: aposta / (pote + aposta).
calc-shortcut-spr = Fórmula: stack / pote.
calc-shortcut-equity_vs_range = Atalho: faixas por mão feita (mão forte ~85%, top pair ~65%, par fraco ~45%, projetos pela regra do 2 e do 4).

calc-start = Começar
calc-submit = Responder
calc-skip = Pular
calc-next = Próximo exercício
calc-type = Digite um número e aperte Enter. Esc pula.
calc-elapsed = { $secs } s (alvo { $target } s)
calc-result-right = Certo
calc-result-wrong = Errado
calc-result-skipped = Pulado
calc-your-answer = Sua resposta: { $value }
calc-exact = Exato: { $value }
calc-shortcut-value = O atalho dá: { $value }
calc-tolerance =
    { $unit ->
        [percent] Aceito dentro de ± { $value } p.p.
       *[other] Aceito dentro de ± { $value }
    }
calc-rating = Avaliado como { $rating }
calc-queued = Esta habilidade está na sua fila de revisão.
calc-stats = { $correct } de { $attempts } certas · média { $secs } s

rating-again = De novo
rating-hard = Difícil
rating-good = Bom
rating-easy = Fácil
