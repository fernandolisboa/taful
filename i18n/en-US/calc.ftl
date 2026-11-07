# Mental-math trainer (spec §11).

calc-intro = Answer fast: right within the target time rates Good, within half of it Easy. Wrong or slow answers go to your review queue.
calc-mixed = Mixed
calc-kind-outs = Outs
calc-kind-rule_of_2_and_4 = Rule of 2 and 4
calc-kind-pot_odds = Pot odds
calc-kind-mdf = MDF
calc-kind-alpha = Alpha
calc-kind-spr = SPR
calc-kind-equity_vs_range = Equity vs range

calc-prompt-outs = You hold { $hero } against { $villain } on { $board }. How many outs do you have?
calc-prompt-rule_of_2_and_4 = { $outs } outs, { $cards ->
    [one] one card
   *[other] two cards
} to come. Chance to hit, in %?
calc-prompt-pot_odds = Pot { $pot } bb, villain bets { $bet } bb. Equity needed to call, in %?
calc-prompt-mdf = Villain bets { $pct }% of the pot. Minimum defense frequency, in %?
calc-prompt-alpha = You bluff { $pct }% of the pot. Folds needed to break even, in %?
calc-prompt-spr = Effective stack { $stack } bb, pot { $pot } bb. SPR?
calc-prompt-equity_vs_range = You hold { $hero } on { $board }. Villain's range: { $range }. Your equity, in %?

calc-shortcut-outs = Shortcut: count by draw type (flush 9, open-ender 8, gutshot 4, overcards 3 each), drop cards that pair the board.
calc-shortcut-rule_of_2_and_4 = Shortcut: outs × 2 with one card to come, outs × 4 with two (minus one per out above 8).
calc-shortcut-pot_odds = Formula: bet / (pot + 2 × bet).
calc-shortcut-mdf = Formula: pot / (pot + bet).
calc-shortcut-alpha = Formula: bet / (pot + bet).
calc-shortcut-spr = Formula: stack / pot.
calc-shortcut-equity_vs_range = Shortcut: bands by made hand (strong made hand ~85%, top pair ~65%, weak pair ~45%, draws by the rule of 2 and 4).

calc-start = Start
calc-submit = Answer
calc-skip = Skip
calc-next = Next exercise
calc-type = Type a number and press Enter. Esc skips.
calc-elapsed = { $secs } s (target { $target } s)
calc-result-right = Right
calc-result-wrong = Wrong
calc-result-skipped = Skipped
calc-your-answer = Your answer: { $value }
calc-exact = Exact: { $value }
calc-shortcut-value = Shortcut gives: { $value }
calc-tolerance =
    { $unit ->
        [percent] Accepted within ± { $value } pp
       *[other] Accepted within ± { $value }
    }
calc-rating = Rated { $rating }
calc-queued = This skill is in your review queue.
calc-stats = { $correct } of { $attempts } right · average { $secs } s

rating-again = Again
rating-hard = Hard
rating-good = Good
rating-easy = Easy

## Exercise picker cards (#57)

calc-desc-mixed = A bit of everything, drawn each round.
calc-desc-outs = How many cards improve your hand.
calc-desc-rule_of_2_and_4 = Turn outs into rough equity.
calc-desc-pot_odds = How much equity you need to call.
calc-desc-mdf = How often to defend against a bet.
calc-desc-alpha = How many folds a bluff needs.
calc-desc-spr = Effective stack divided by the pot.
calc-desc-equity_vs_range = Your equity against a range on the board.
calc-how-title = Pick an exercise and start
