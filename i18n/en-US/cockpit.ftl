# Strings only the Cockpit visual style shows (D19).

## Icon rail (short labels) and status bar

cockpit-nav-calc = Math
cockpit-nav-review = Reviews
cockpit-nav-hands = Hands
cockpit-nav-settings = Settings
cockpit-bar-today = Today
cockpit-bar-decisions = { $count ->
    [1] decision
   *[other] decisions
}
cockpit-bar-accuracy = Accuracy
cockpit-bar-ev-lost = EV lost
cockpit-bar-last = Latest
cockpit-bb = { $value } bb
cockpit-percent = { $value }%
cockpit-none = –
cockpit-date = { $weekday }, { $month } { $day }
cockpit-weekday-0 = Monday
cockpit-weekday-1 = Tuesday
cockpit-weekday-2 = Wednesday
cockpit-weekday-3 = Thursday
cockpit-weekday-4 = Friday
cockpit-weekday-5 = Saturday
cockpit-weekday-6 = Sunday
cockpit-month-1 = Jan
cockpit-month-2 = Feb
cockpit-month-3 = Mar
cockpit-month-4 = Apr
cockpit-month-5 = May
cockpit-month-6 = Jun
cockpit-month-7 = Jul
cockpit-month-8 = Aug
cockpit-month-9 = Sep
cockpit-month-10 = Oct
cockpit-month-11 = Nov
cockpit-month-12 = Dec
cockpit-lessons-crumb = { $tracks ->
    [1] 1 track
   *[other] { $tracks } tracks
} · { $lessons ->
    [1] 1 lesson
   *[other] { $lessons } lessons
}

## Home: the four numbers at the top

cockpit-kpi-reviews = Reviews today
cockpit-kpi-reviews-minutes = ~{ $minutes } min
cockpit-kpi-reviews-ahead = tomorrow { $tomorrow } · 7 days { $week }
cockpit-review-go = Review
cockpit-day-today = T
cockpit-day-0 = M
cockpit-day-1 = T
cockpit-day-2 = W
cockpit-day-3 = T
cockpit-day-4 = F
cockpit-day-5 = S
cockpit-day-6 = S
cockpit-kpi-accuracy = Accuracy · 7 days
cockpit-kpi-points = { $points } pts
cockpit-kpi-vs-last-week = vs. the week before
cockpit-kpi-no-trend = no earlier week to compare with
cockpit-kpi-ev = EV lost per decision
cockpit-kpi-calc = Mental math · average time
cockpit-kpi-calc-detail = { $accuracy }% right · target { $target } s
cockpit-kpi-empty = no data yet
cockpit-unit-bb = bb
cockpit-unit-seconds = s
cockpit-unit-percent = %

## Home: decisions chart and leaks

cockpit-chart-title = Decisions by grade · last 14 days
cockpit-chart-empty = Decisions graded at the table show up here, day by day.
cockpit-leaks-title = Top leaks
cockpit-leaks-by = by EV lost
cockpit-leaks-spot = Spot
cockpit-leaks-mistakes = Mistakes
cockpit-leaks-bb = bb lost
cockpit-leaks-train = Drill

## Home: tracks, preflop deviation and shortcuts

cockpit-tracks-title = Tracks
cockpit-next-lesson = Next lesson
cockpit-next-meta = { $track } · { $n } of { $total } · { $minutes } min
cockpit-continue = Continue
cockpit-dev-title = Preflop · where you leave the reference
cockpit-dev-count = { $count ->
    [1] 1 decision
   *[other] { $count } decisions
}
cockpit-dev-top = Most off
cockpit-dev-none = No relevant deviation so far.
cockpit-dev-same = same
cockpit-dev-far = off
cockpit-dev-too-much = too much { $play }
cockpit-dev-too-little = too little { $play }
cockpit-play-fold = fold
cockpit-play-call = call
cockpit-play-open = open
cockpit-play-threebet = 3-bet
cockpit-play-fourbet = 4-bet
cockpit-play-jam = all-in
cockpit-play-raise = raise
cockpit-shortcuts-title = Shortcuts
cockpit-sc-spot = Table · Spot mode
cockpit-sc-spot-body = Preflop decisions from the charts
cockpit-sc-postflop = Table · Postflop
cockpit-sc-postflop-body = Flop and turn from the solved library
cockpit-sc-hand = Table · Full hand
cockpit-sc-hand-body = Preflop to river against the bots
cockpit-sc-calc = Mental math
cockpit-sc-calc-body = Pot odds, outs and equity against the clock
cockpit-sc-hands = Hand review
cockpit-sc-hands-body = Go over the latest hands played
cockpit-sc-leak = Drill the top leak
cockpit-sc-leak-none = No leak yet

## Lessons

cockpit-lessons-progress = Overall progress
cockpit-lessons-done = Done
cockpit-lessons-of = / { $total }
cockpit-lessons-time-left = Time left
cockpit-lessons-quiz = Quiz average
cockpit-unit-hours = h
cockpit-unit-minutes = min
cockpit-lesson-minutes = { $minutes } min
cockpit-lesson-minutes-next = { $minutes } min · next
cockpit-lesson-open = Open

## Table

cockpit-act-fold = fold
cockpit-act-check = check
cockpit-act-call = call { $amount }
cockpit-act-bet = bet { $amount }
cockpit-act-open = open { $amount }
cockpit-act-nbet = { $n }-bet { $amount }
cockpit-act-raise = raise { $amount }
cockpit-act-allin = all-in { $amount }
cockpit-act-pot-share = { $percent }% pot
cockpit-you = you
cockpit-pod-turn = your turn
cockpit-pot = Pot
cockpit-table-crumb-spot = spot { $n }
cockpit-table-crumb-hand = hand { $n }
cockpit-sel-stack = Stack
cockpit-key-space = Space
cockpit-repeat = Replay
cockpit-table-turn = { $street } · your turn on the { $position }
cockpit-table-replay = { $street } · replay
cockpit-table-wait = { $street } · waiting
cockpit-table-over = Hand over
cockpit-table-pot = POT { $amount } bb
cockpit-table-spr = SPR { $value }
cockpit-table-odds = ODDS { $ratio } · { $percent }%
cockpit-line-title = Hand line
cockpit-eval-title = Evaluation
cockpit-eval-vs-reference = against the reference strategy
cockpit-eval-loss = EV loss
cockpit-eval-ev = EV of the action
cockpit-col-freq = Frequency with { $hand }
cockpit-col-ev = EV
cockpit-col-delta = Δ EV
cockpit-range-title = { $position } range at this point
cockpit-range-title-none = Range at this point
cockpit-range-note = reference strategy by hand
cockpit-range-all = Whole range
cockpit-range-yours = your hand
cockpit-equity = Equity
cockpit-equity-vs = vs. { $position } range
cockpit-equity-needs = needs (pot odds)
cockpit-kind-raise = Raise
cockpit-kind-bet = Bet
cockpit-kind-call = Call
cockpit-kind-check = Check
cockpit-kind-fold = Fold
cockpit-hand-straight-flush = straight flush
cockpit-hand-quads = quads
cockpit-hand-full-house = full house
cockpit-hand-flush = flush
cockpit-hand-straight = straight
cockpit-hand-set = set
cockpit-hand-trips = trips
cockpit-hand-two-pair = two pair
cockpit-hand-overpair = overpair
cockpit-hand-top-pair = top pair
cockpit-hand-middle-pair = middle pair
cockpit-hand-bottom-pair = bottom pair
cockpit-hand-pocket-pair = pocket pair
cockpit-hand-underpair = underpair
cockpit-hand-ace-high = ace high
cockpit-hand-overcards = two overcards
cockpit-hand-high-card = high card
cockpit-hand-combo-draw = combo draw
cockpit-hand-flush-draw = flush draw
cockpit-hand-open-ended = open-ended straight draw
cockpit-hand-gutshot = gutshot
cockpit-hand-suited-ace = suited ace
cockpit-hand-suited-broadways = suited broadways
cockpit-hand-suited-connectors = suited connectors
cockpit-hand-suited-gapper = suited one-gapper
cockpit-hand-suited = suited
cockpit-hand-offsuit-ace = offsuit ace
cockpit-hand-offsuit-broadways = offsuit broadways
cockpit-hand-offsuit-connectors = offsuit connectors
cockpit-hand-offsuit = offsuit
