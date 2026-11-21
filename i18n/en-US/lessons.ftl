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
tree-lessons = { $count ->
    [one] 1 lesson
   *[other] { $count } lessons
}
tree-track-locked = { $count ->
    [one] 1 lesson
   *[other] { $count } lessons
} · locked for now. Click to preview.

## Study mode (D24): the gear on the lessons screen and Settings › Study

study-mode-title = Study mode
study-mode-gear = Study mode
study-mode-prova = Test
study-mode-prova-line = Your level is open; a short test opens the next.
study-mode-prova-about = Every lesson of the level you are in is open, in any order. When you feel ready, take the level test to open the next level. You can take it early to skip a level you already know.
study-mode-guiado = Guided
study-mode-guiado-line = One lesson at a time, in Journey order.
study-mode-guiado-about = The next lesson opens when you pass the quiz of the one before. Good if you are starting out and would rather not choose. The level test still lets you skip a level.
study-mode-livre = Free
study-mode-livre-line = Everything open; the recommended lesson is marked.
study-mode-livre-about = Nothing is locked. The recommended next lesson stays highlighted, and a lesson that builds on others you have not done opens with a short note. Level tests show you where you stand.
study-mode-note = Switching modes keeps everything: finished lessons and passed tests stay.

## Level tests (D24)

tree-test = Level test
tree-test-take = Take the test
tree-test-retake = Take it again
tree-test-passed = Test passed
tree-test-meta = { $questions } questions · { $pass } to pass
tree-opens-with = Opens with the { $level } test
tree-next-test = You finished the open lessons. The { $level } test opens the next level.
level-test-title = Level test: { $level }
level-test-intro = { $questions } questions from this level's lessons. Get { $need } right to pass. Passing opens the next level; it does not mark lessons as done, and wrong answers do not go to your reviews.
level-test-start = Start the test
level-test-passed = Passed with { $score }.
level-test-opened = Open now: { $levels }.
level-test-failed = { $score }. You need { $pass } to pass. Try again whenever you like: the questions change every time.
lesson-locked-test = This lesson is in a level that is still closed. It opens when you pass the { $level } test. You can read it, but the quiz waits.
lesson-locked-step = In Guided mode lessons open one at a time. You can read this one, but the quiz waits.
lesson-locked-step-next = In Guided mode lessons open one at a time; the next one is { $lesson }. You can read this one, but the quiz waits.
lesson-missing = This lesson builds on { $lessons }. You can read it anyway.


## Guided hand (D26)
lesson-guided-title = Guided hand: { $name }
lesson-guided-body = Play a hand on the real table, step by step. The lesson is done once you pass the quiz and finish the hand.
lesson-guided-play = Play the guided hand
lesson-guided-again = Play the guided hand again
lesson-guided-finished = Hand finished.
quiz-guided-pending = One step left: play the guided hand to finish the lesson.
