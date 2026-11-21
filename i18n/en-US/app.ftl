# App shell strings. en-US is the source locale; every key here must exist
# in every other locale (checked by a test in taful-app).

-product = Taful

app-name = { -product }
app-tagline = No-Limit Hold'em coach
window-title = { -product } · No-Limit Hold'em coach

## Navigation (spec §15.1)

nav-home = Home
nav-skill-tree = Lessons
nav-lesson = Lesson
nav-group-learn = Learn
nav-group-practice = Practice
nav-group-analyze = Analyze
nav-table = Table
nav-hand-review = Hand review
nav-calc = Mental math
nav-review = Review session
nav-settings = Settings

## Screen descriptions shown until each screen is built

screen-home-body = Your next step, due reviews, your top leaks and quick actions.
screen-skill-tree-body = From zero to pro, by level or by theme. The gear sets how lessons open.
screen-lesson-body = Lessons with glossary tooltips and a quiz at the end.
screen-table-body = Spot mode and full hands against the reference bot, with every decision graded by EV loss.
screen-hand-review-body = Every decision of a hand with its grade, and a step-by-step replay.
screen-calc-body = Timed pot-odds, equity and combo exercises, with the exact calculation next to the shortcut.
screen-review-body = Your spaced-repetition queue: spots you misplayed, missed quiz questions and slow or wrong calc answers, in one session.
screen-settings-body = Language, appearance, study, your data and updates, each in its own tab.
screen-coming-in = Coming in { $milestone }.

## Footer

footer-version = Version { $version }
footer-offline = Local-first: your data stays on this computer.

# System tray icon (D18).
tray-open = Open { -product }
tray-check-updates = Check for updates
tray-install-update = Restart and update to { $version }
tray-quit = Quit
