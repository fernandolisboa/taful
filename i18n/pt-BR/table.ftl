# Tela da mesa (spec §10, §15.1): modo spot e mãos completas no preflop (M3).

table-subtitle = Preflop, cash 6-max 100bb. Cada decisão é avaliada pela perda de EV contra os charts de referência.
table-mode-spot = Modo spot
table-mode-hand = Mão completa
table-mode-postflop = Pós-flop
table-spot-kind-rfi = Abrir o pote
table-spot-kind-vs-open = Contra open
table-spot-kind-squeeze = Squeeze
table-spot-kind-vs-3bet = Contra 3-bet
table-spot-kind-vs-4bet = Contra 4-bet
table-spot-kind-vs-allin = Contra all-in
table-spot-kind-bvb = Blind contra blind
table-pot = Pote { $amount } bb
table-stack = { $amount } bb
table-spot-prompt = Sua vez. Escolha uma ação abaixo.
table-no-spot = Nenhum spot disponível.
table-chart-error = Não deu para carregar o chart de preflop: { $error }

## Ações (os botões mostram exatamente os tamanhos da árvore, D4)

table-action-fold = Fold
table-action-check = Check
table-action-call = Call { $amount }
table-action-raise = Raise para { $amount }
table-action-bet = Bet { $amount }
table-action-allin = All-in { $amount }
table-log-call = Call
table-log-allin = All-in

## Avaliação (spec §8)

table-grade-best = Melhor
table-grade-correct = Correta
table-grade-inaccuracy = Imprecisão
table-grade-mistake = Erro
table-grade-blunder = Erro grave
table-ev-loss = Perda de EV: { $amount } bb
table-decision-context = { $position } com { $hand }
table-col-action = Ação
table-col-hand-freq = Sua mão
table-col-ev = EV (bb)
table-col-range-freq = Range
table-next-spot = Próximo spot

## Mão completa

table-decisions = Suas decisões
table-new-hand = Nova mão
table-hand-over-settled = Fim da mão. Seu resultado: { $amount } bb.
table-hand-over-flop = O preflop acabou. O jogo pós-flop chega com a biblioteca pós-flop.
table-hand-over-left = Esta linha está fora dos charts, então a mão termina aqui: { $line }
table-hand-ev-loss = EV perdido nesta mão: { $amount } bb

## Postflop spot mode (M4, spec §7)

table-subtitle-postflop = Pós-flop, cash 6-max 100bb. Decisões no flop e no turn, avaliadas contra a biblioteca resolvida.
table-postflop-any = Todos os spots
table-postflop-btn-bb-srp = BTN contra BB
table-postflop-co-bb-srp = CO contra BB
table-postflop-sb-bb-srp = SB contra BB
table-postflop-btn-sb-3bp = BTN contra SB, 3-bet
table-postflop-co-btn-3bp = CO contra BTN, 3-bet
table-postflop-missing = A biblioteca pós-flop ainda não está neste computador. Ela é gerada pelo tools/library-build e lida de { $dir }.
table-postflop-error = Não deu para carregar a biblioteca pós-flop: { $error }
