# Review session (spec §13) and home screen.

review-intro = Your spaced-repetition queue: spots you misplayed, missed quiz questions and slow or wrong calc answers, in one session.
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
home-lesson-none = Every lesson is done. More arrive with the next milestones.
home-calc-title = Mental math
home-calc-recent = { $correct } of { $total } right in your last answers
home-calc-none = No answers yet.
home-calc-start = Train
home-progress = { $done } of { $total } lessons done
home-leaks-title = Top leaks
home-leaks-body = No leaks yet. They show up once graded decisions pile up mistakes in a spot.
review-dealing = Dealing a spot…

home-continue-title = Pick up where you left off
home-start-lesson = Start lesson
home-calc-short = Drill pot odds, outs and equity against the clock.
home-due-short = { $count } to review
