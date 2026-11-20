# Salão assets

Files the Salão visual style (D19) embeds. They are built once from the
approved mockup's own rules and committed, so building the app needs none
of the tools below.

## `fonts/`

Static fonts made by `build/fonts.py` (needs `fonttools` and `brotli`):

- Fraunces, one instance per text style of the mockup (optical size,
  weight, letter-spacing), from the variable fonts in `build/src/fonts/`.
- Inter 4.0, the five weights as one family plus one font per tracked
  label style.

GPUI has no letter-spacing and no variable-font axes, which is why each
style is its own family (`Taful Salao …`, declared in `src/salao/kit.rs`).
Both typefaces are under the SIL Open Font License 1.1: `OFL-Fraunces.txt`
and `OFL-Inter.txt`.

## `img/`

Made by `build/render.js` (needs Node with Playwright, Chromium and Inter
installed) from `build/src/sprites.html` and `sprites.css`, which hold the
mockup's CSS for each piece:

- `bg.webp`, `hero.webp`, `table.webp`: what needs radial gradients or
  rotated cards (the lit background, the home's fan of cards, the felt
  with its rail).
- `card_*.png`, `back_*.png`, `chip_*.png`, `dealer.png`, `seal_*.png`,
  `medal_done.png`, `avatar_hero.png`: small pieces with gradients and
  inner shadows, drawn at twice their size.
- `*.svg`: icons the app tints (suits, lock, check, the two parts of the
  Taful mark).

A new file is listed in `ASSETS` or `FONTS` in `src/salao/mod.rs`; the
module's tests check that every embedded file is used and every used file
is embedded.
