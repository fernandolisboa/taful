# Strings da tela de lições, do leitor de lições e do quiz.

## Catálogo (D23): títulos de níveis e temas, conferidos pelo content-check

level-primeiros-passos = Primeiros passos
level-primeiros-passos-about = Nunca joguei
level-fundamentos = Fundamentos
level-fundamentos-about = Sei as regras
level-iniciante = Iniciante
level-iniciante-about = Jogo por diversão
level-intermediario = Intermediário
level-intermediario-about = Jogo e estudo
level-avancado = Avançado
level-avancado-about = Estudo e ganho
level-pro = Pró
level-pro-about = Quero o nível mais alto

theme-basics = O básico
theme-math = Matemática
theme-preflop = Preflop
theme-postflop = Pós-flop
theme-reading = Leitura do jogo
theme-mind = Mente e banca

## Seletor "Ver por" da tela de lições

tree-view-by = Ver por
tree-view-journey = Jornada
tree-view-themes = Temas
tree-soon = Em breve

## Skill tree

tree-state-done = Concluída
tree-state-available = Disponível
tree-state-locked = Bloqueada
tree-minutes = { $minutes } min
tree-best = Melhor { $score }

## Lesson reader

lesson-none = Escolha uma lição na lista para começar a ler.
lesson-open-tree = Ver as lições
lesson-locked-note = Esta lição ainda está bloqueada: conclua os pré-requisitos primeiro. Dá para ler, mas o quiz espera.
lesson-glossary = Termos desta lição
lesson-take-quiz = Fazer o quiz
lesson-retake-quiz = Fazer o quiz de novo
lesson-picture-you = Você
lesson-back-to-tree = ← Voltar às lições
lesson-render-error = Esta lição não pôde ser exibida: { $error }

## Quiz

quiz-title = Quiz
quiz-progress = Pergunta { $n } de { $total }
quiz-type-answer = Digite a resposta e aperte Enter
quiz-check = Conferir
quiz-next = Próxima
quiz-finish = Ver resultado
quiz-correct = Certo
quiz-wrong = Quase. Resposta: { $answer }
quiz-passed = Aprovado com { $score }.
quiz-failed = { $score }. Para passar você precisa de { $pass }; as perguntas erradas foram para a sua fila de revisão.
quiz-unlocked = Liberadas: { $lessons }

tree-track-progress = { $done } de { $total }
tree-start = Começar
tree-lessons = { $count ->
    [one] 1 lição
   *[other] { $count } lições
}
tree-track-locked = { $count ->
    [one] 1 lição
   *[other] { $count } lições
} · fechadas por enquanto. Clique para dar uma olhada.

## Modo de estudo (D24): a engrenagem da tela de Lições e Configurações › Estudo

study-mode-title = Modo de estudo
study-mode-gear = Modo de estudo
study-mode-prova = Prova
study-mode-prova-line = O seu nível fica aberto; uma prova curta abre o próximo.
study-mode-prova-about = Todas as lições do nível em que você está ficam abertas, na ordem que quiser. Quando se sentir pronto, faça a prova do nível para abrir o próximo. Dá para fazer a prova antes, para pular um nível que você já conhece.
study-mode-guiado = Guiado
study-mode-guiado-line = Uma lição de cada vez, na ordem da Jornada.
study-mode-guiado-about = A próxima lição abre quando você passa no quiz da anterior. Bom para quem está começando e prefere não escolher. A prova de nível continua valendo para pular um nível.
study-mode-livre = Livre
study-mode-livre-line = Tudo aberto; a lição recomendada fica marcada.
study-mode-livre-about = Nada fica bloqueado. A próxima lição recomendada fica destacada, e uma lição que depende de outras que você ainda não fez abre com um aviso curto. As provas de nível mostram onde você está.
study-mode-note = Trocar de modo não apaga nada: lições concluídas e provas aprovadas continuam valendo.

## Provas de nível (D24)

tree-test = Prova do nível
tree-test-take = Fazer a prova
tree-test-retake = Refazer
tree-test-passed = Prova aprovada
tree-test-meta = { $questions } perguntas · { $pass } para passar
tree-opens-with = Abre com a prova de { $level }
tree-next-test = Você terminou as lições abertas. A prova de { $level } abre o próximo nível.
level-test-title = Prova: { $level }
level-test-intro = { $questions } perguntas das lições deste nível. Acerte { $need } para passar. Passar abre o próximo nível; as lições não ficam marcadas como concluídas, e as perguntas erradas não vão para as revisões.
level-test-start = Começar a prova
level-test-error = Algo deu errado: { $error }. Tente de novo daqui a pouco.
level-test-passed = Aprovado com { $score }.
level-test-opened = Aberto agora: { $levels }.
level-test-failed = { $score }. Para passar você precisa de { $pass }. Tente de novo quando quiser: as perguntas mudam a cada tentativa.
lesson-locked-test = Esta lição é de um nível ainda fechado. Ele abre quando você passa na prova de { $level }. Dá para ler, mas o quiz espera.
lesson-locked-step = No modo Guiado as lições abrem uma de cada vez. Dá para ler esta, mas o quiz espera.
lesson-locked-step-next = No modo Guiado as lições abrem uma de cada vez; a próxima é { $lesson }. Dá para ler esta, mas o quiz espera.
lesson-missing = Esta lição usa o que foi visto em { $lessons }. Dá para ler mesmo assim.


## Mão guiada (D26)
lesson-guided-title = Mão guiada: { $name }
lesson-guided-body = Jogue uma mão na mesa de verdade, passo a passo. A lição fica concluída quando você passa no quiz e termina a mão.
lesson-guided-play = Jogar a mão guiada
lesson-guided-again = Jogar a mão guiada de novo
lesson-guided-finished = Mão concluída.
quiz-guided-pending = Falta um passo: jogue a mão guiada para concluir a lição.
