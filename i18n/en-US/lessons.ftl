# Skill tree, lesson reader and quiz strings.

## Track titles (`track-<id>`, checked by content-check)

track-foundations = Foundations
track-poker-math = Poker math
track-preflop = Preflop
track-postflop = Postflop
track-game-plan = Game plan

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
} · unlocks when you finish the previous track. Click to preview.

## Guided hand (D26)
lesson-guided-title = Guided hand: { $name }
lesson-guided-body = Play a hand on the real table, step by step. The lesson is done once you pass the quiz and finish the hand.
lesson-guided-play = Play the guided hand
lesson-guided-again = Play the guided hand again
lesson-guided-finished = Hand finished.
quiz-guided-pending = One step left: play the guided hand to finish the lesson.
