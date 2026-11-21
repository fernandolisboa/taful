# Skill tree, lesson reader and quiz strings.

## Catalog (D23): level and theme titles, checked by content-check

level-primeiros-passos = First steps
level-primeiros-passos-about = I have never played
level-fundamentos = Fundamentals
level-fundamentos-about = I know the rules
level-iniciante = Beginner
level-iniciante-about = I play for fun
level-intermediario = Intermediate
level-intermediario-about = I play and study
level-avancado = Advanced
level-avancado-about = I study and I win
level-pro = Pro
level-pro-about = I want the top level

theme-basics = The basics
theme-math = Math
theme-preflop = Preflop
theme-postflop = Postflop
theme-reading = Reading the game
theme-mind = Mind and bankroll

## "View by" switch on the lessons screen

tree-view-by = View by
tree-view-journey = Journey
tree-view-themes = Themes
tree-soon = Coming soon

## Skill tree

tree-state-done = Done
tree-state-available = Available
tree-state-locked = Locked
tree-minutes = { $minutes } min
tree-best = Best { $score }

## Lesson reader

lesson-none = Pick a lesson in the lessons list to start reading.
lesson-open-tree = Open the lessons
lesson-locked-note = This lesson is still locked: finish its prerequisites first. You can read it, but the quiz waits.
lesson-glossary = Terms in this lesson
lesson-take-quiz = Take the quiz
lesson-retake-quiz = Take the quiz again
lesson-back-to-tree = ← Back to lessons
lesson-render-error = This lesson failed to render: { $error }

## Quiz

quiz-title = Quiz
quiz-progress = Question { $n } of { $total }
quiz-type-answer = Type your answer and press Enter
quiz-check = Check
quiz-next = Next
quiz-finish = See result
quiz-correct = Correct
quiz-wrong = Not quite. Answer: { $answer }
quiz-passed = Passed with { $score }.
quiz-failed = { $score }. You need { $pass } to pass; the missed questions went to your review queue.
quiz-unlocked = Unlocked: { $lessons }

tree-track-progress = { $done } of { $total }
tree-start = Start
tree-track-locked = { $count ->
    [one] 1 lesson
   *[other] { $count } lessons
} · unlocks as you finish the lessons it builds on. Click to preview.
