# Strings only the Baralho visual style shows (D19).

## Dates

baralho-date = { $weekday }, { $month } { $day }
baralho-weekday-1 = Monday
baralho-weekday-2 = Tuesday
baralho-weekday-3 = Wednesday
baralho-weekday-4 = Thursday
baralho-weekday-5 = Friday
baralho-weekday-6 = Saturday
baralho-weekday-7 = Sunday
baralho-month-1 = January
baralho-month-2 = February
baralho-month-3 = March
baralho-month-4 = April
baralho-month-5 = May
baralho-month-6 = June
baralho-month-7 = July
baralho-month-8 = August
baralho-month-9 = September
baralho-month-10 = October
baralho-month-11 = November
baralho-month-12 = December

## Home

baralho-home-title = Today's hand
baralho-home-body = Four cards on the table. Which one do you start with?
baralho-home-continue = Continue · { $track }
baralho-home-lesson-body = Lesson { $n } of { $total }. About { $minutes } min of reading and a quiz.
baralho-home-lesson-body-plain = Lesson { $n } of { $total }. A read and a quiz.
baralho-home-lesson-go = Continue lesson
baralho-home-lessons-done = Lessons
baralho-home-lessons-done-title = All read
baralho-home-all-lessons = See the lessons
baralho-home-review = Review
baralho-home-review-title =
    { $count ->
        [1] 1 review
       *[other] { $count } reviews
    }
baralho-home-review-body = About { $minutes } min to keep what you have learned.
baralho-home-review-tomorrow = Tomorrow brings { $count }.
baralho-home-review-go = Start review
baralho-home-review-none-title = Nothing to review
baralho-home-review-none-body = Your queue is up to date.
baralho-home-practice = Practice · your biggest leak
baralho-home-practice-nth = Practice · leak no. { $n }
baralho-home-leak-body =
    { $where }: { $mistakes ->
        [1] 1 mistake
       *[other] { $mistakes } mistakes
    } in { $decisions ->
        [1] 1 decision
       *[other] { $decisions } decisions
    } and { $loss } bb lost.
baralho-on-street-preflop = Preflop
baralho-on-street-flop = On the flop
baralho-on-street-turn = On the turn
baralho-on-street-river = On the river
baralho-home-practice-none = Practice
baralho-home-practice-none-title = Training table
baralho-home-practice-none-go = Go to the table
baralho-home-calc = Calculate
baralho-home-calc-body = { $secs } s per exercise on average, { $accuracy }% right. The target is { $target } s.
baralho-home-tracks = Your tracks · { $done } of { $total } lessons
baralho-home-all-lessons-link = See all lessons →
baralho-track-done = Completed
baralho-track-open = In progress
baralho-track-locked = Opens after the previous track

## Lessons

baralho-lessons-body = Each track is a suit. A finished lesson turns face up; the closed ones open in order.
baralho-lessons-total = of { $total } lessons
baralho-lessons-count = { $done } of { $total }
baralho-lessons-track-done = completed
baralho-lessons-track-open = in progress
baralho-lessons-track-ready = open
baralho-lessons-track-locked = closed
baralho-lessons-next-meta = { $minutes } min · lesson and quiz
baralho-lessons-next-meta-plain = lesson and quiz
baralho-lessons-continue = Continue

## Table

baralho-pot = Pot
baralho-you = you
baralho-seat-you = { $action } · you
baralho-repeat-spot = Repeat this spot
baralho-repeat-note = You are repeating this spot: the new answer does not go into your history.
baralho-turn-title = Your turn
baralho-turn-body = Pick an action under the table.
baralho-replay-title = Replay
baralho-over-title = Hand over
baralho-over-body = The hand ended before your turn came.
baralho-result-with = { $action } with
baralho-ev-loss = EV loss
baralho-actions-here = Actions at this point
baralho-actions-note = reference frequency · EV
baralho-actions-note-profile = reference frequency · EV against { $profile }
baralho-your-pick = your pick
baralho-session = Session · { $count ->
        [0] no decisions
        [1] 1 decision
       *[other] { $count } decisions
    }
baralho-session-loss = EV lost { $amount } bb
