# Skill tree, lesson reader and quiz strings.

## Track titles (`track-<id>`, checked by content-check)

track-foundations = Foundations
track-poker-math = Poker math

## Skill tree

tree-intro = Work through the tracks in order. Passing a lesson's quiz unlocks the next ones.
tree-state-done = Done
tree-state-available = Available
tree-state-locked = Locked
tree-minutes = { $minutes } min
tree-best = Best { $score }

## Lesson reader

lesson-none = Pick a lesson in the skill tree to start reading.
lesson-open-tree = Open the skill tree
lesson-locked-note = This lesson is still locked: finish its prerequisites first. You can read it, but the quiz waits.
lesson-glossary = Terms in this lesson
lesson-take-quiz = Take the quiz
lesson-retake-quiz = Take the quiz again
lesson-back-to-tree = Back to the skill tree
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
