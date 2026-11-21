# Strings only the Salão visual style shows (D19).

## Dates (the home screen's "Friday, November 20")

salao-date = { $weekday }, { $month } { $day }
salao-weekday-1 = Monday
salao-weekday-2 = Tuesday
salao-weekday-3 = Wednesday
salao-weekday-4 = Thursday
salao-weekday-5 = Friday
salao-weekday-6 = Saturday
salao-weekday-7 = Sunday
salao-month-1 = January
salao-month-2 = February
salao-month-3 = March
salao-month-4 = April
salao-month-5 = May
salao-month-6 = June
salao-month-7 = July
salao-month-8 = August
salao-month-9 = September
salao-month-10 = October
salao-month-11 = November
salao-month-12 = December

## Home

salao-home-lessons-total = { $done } of { $total } lessons in all
salao-home-due-today = for today · about { $minutes } min
salao-home-due-none = nothing for today
salao-home-review-start = Start review
salao-seconds = s
salao-home-calc-pace = average time · { $percent }% right
salao-home-calc-none = no answers yet
salao-home-table-week = Table · this week
salao-home-decisions = { $count ->
    [one] decision
   *[other] decisions
}
salao-home-table-good = { $percent }% best or correct
salao-home-table-none = no decisions this week
salao-home-play = Play
salao-home-leaks-sub = where your graded decisions lose the most EV
salao-leak-stats = { $mistakes ->
    [one] { $mistakes } mistake
   *[other] { $mistakes } mistakes
} in { $decisions ->
    [one] { $decisions } decision
   *[other] { $decisions } decisions
}
salao-bb = bb

## Lessons

salao-lessons-of = of { $total ->
    [one] 1 lesson
   *[other] { $total } lessons
}
salao-of = of { $total }
salao-track-current = In progress
salao-track-locked = { $count ->
    [one] 1 lesson
   *[other] { $count } lessons
} · locked for now.
salao-lesson-next = next
salao-lesson-continue = Continue
salao-lesson-quiz = quiz { $score }
salao-track-open = { $count ->
    [one] 1 lesson
   *[other] { $count } lessons
}

## Table

salao-pot = Pot
salao-you = you
salao-seat-you = { $action } · you
salao-ev-loss = EV loss
salao-options = Actions at this point
salao-options-note = reference frequency · EV
salao-options-note-profile = frequency · EV against the bot
salao-option-yours = your choice
salao-option-best = best EV
salao-verdict-with = { $action } with
salao-repeat-spot = Repeat this spot
salao-session = Session · { $count ->
    [one] 1 decision
   *[other] { $count } decisions
}
salao-session-ev = EV lost { $amount } bb
