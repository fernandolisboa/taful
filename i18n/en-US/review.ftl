# Review session (spec §13) and home screen.

review-due = { $count ->
    [0] Nothing due right now.
    [one] 1 item due.
   *[other] { $count } items due.
}
review-start = Start reviewing
review-next-at = Next review: { $when }
review-empty = All caught up.
review-from-lesson = From the lesson “{ $lesson }”
review-continue = Continue
review-session-done = { $count ->
    [one] 1 review done this session.
   *[other] { $count } reviews done this session.
}

home-due-title = Reviews
home-lesson-test = You finished the open lessons. Take the { $level } test in Lessons to open the next level.
home-lesson-none = Every lesson is done. More arrive with the next milestones.
home-calc-title = Mental math
home-calc-recent = { $correct } of { $total } right in your last answers
home-calc-start = Train
home-progress = { $done } of { $total } lessons done
home-leaks-title = Top leaks
home-leaks-body = No leaks yet. They show up once graded decisions pile up mistakes in a spot.
review-dealing = Dealing a spot…

home-continue-title = Pick up where you left off
home-start-lesson = Start lesson
home-calc-short = Drill pot odds, outs and equity against the clock.
home-due-short = waiting for you now

review-empty-body = Spots you miss at the table, wrong quiz answers and slow calcs land here on their own.

review-go-calc = Practice mental math

# The Home's next step (D27): reviews, then the lesson, then the table.
next-step-label = Next step
next-step-review = Reviews
next-step-table = Table
next-step-review-title = { $count ->
    [one] 1 review for today
   *[other] { $count } reviews for today
}
next-step-review-body = About { $minutes } min. What you missed comes back before you forget it.
next-step-review-go = Review now
next-step-lesson-place = Lesson { $n } of { $total }
next-step-lesson-minutes = { $minutes } min of reading and a quiz
next-step-lesson-go = Start lesson
next-step-lesson-continue = Continue lesson
next-step-table-title = Practice at the table
next-step-table-after = Practice what you saw in “{ $lesson }”: { $practice }.
next-step-table-plain = Practice { $practice }.
next-step-table-test = Whenever you like, the { $level } test opens the next level.
next-step-table-go = Go to the table
next-step-practice-hand = full hands against the bots
next-step-practice-postflop = decisions after the flop
next-step-practice-rfi = opening the pot when everyone folds to you
next-step-practice-vs-open = answering a raise before the flop
next-step-practice-squeeze = acting after a raise and a call
next-step-practice-vs-3bet = answering a 3-bet
next-step-practice-vs-4bet = answering a 4-bet
next-step-practice-vs-allin = answering an all-in before the flop
next-step-practice-bvb = playing small blind against big blind
