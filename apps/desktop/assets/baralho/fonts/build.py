#!/usr/bin/env python3
"""Builds the static font files of the Baralho visual style.

GPUI has no letter-spacing and cannot set variable-font axes, so every text
style of the Baralho mockups that needs an optical size, a weight or a
tracking value gets its own static instance:

- display-<opsz>-<wght>-<tracking>.ttf: Bricolage Grotesque at one optical
  size and weight, with the tracking (thousandths of an em, negative) baked
  into the advance widths. Family "Baralho D<opsz> W<wght> T<tracking>".
- caps-<wght>-<tracking>.ttf: Figtree with positive tracking for the small
  uppercase labels. Family "Baralho Caps W<wght> T<tracking>".
- figtree-<wght>[-italic].ttf: plain Figtree instances, family "Figtree".

Common ligatures are removed (the mockups turn them off).

Usage (needs fonttools; the sources are the variable fonts from
https://github.com/google/fonts, ofl/bricolagegrotesque and ofl/figtree):

    python3 build.py <BricolageGrotesque[opsz,wdth,wght].ttf> <Figtree[wght].ttf> <Figtree-Italic[wght].ttf>

The list of display faces must match `DISPLAY_FACES` in
`apps/desktop/src/baralho/type_.rs` (a test there checks the files exist).
"""
import os
import sys

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

OUT = os.path.dirname(os.path.abspath(__file__))

# (optical size, weight, tracking in -1/1000 em)
DISPLAY = [
    (14, 700, 10), (14, 700, 70), (14, 800, 0), (14, 800, 20), (14, 800, 40),
    (17, 700, 10), (17, 700, 20), (17, 800, 0),
    (21, 800, 20), (21, 800, 25), (21, 800, 30), (21, 800, 40),
    (26, 800, 30), (26, 800, 40),
    (32, 800, 30), (32, 800, 40),
    (46, 800, 40),
    (96, 800, 35), (96, 800, 40), (96, 800, 50),
]
# (weight, tracking in +1/1000 em)
CAPS = [(700, 120), (800, 60), (800, 120), (800, 160)]
# (weight, italic)
TEXT = [(400, False), (500, False), (600, False), (700, False), (800, False), (400, True), (700, True)]


# Display faces keep Latin (Basic, Latin-1, Extended-A) and common
# punctuation and symbols; the system font covers anything else.
LATIN = (
    set(range(0x20, 0x7F)) | set(range(0xA0, 0x180)) | set(range(0x2000, 0x2070))
    | set(range(0x20A0, 0x20C0)) | set(range(0x2190, 0x2200)) | set(range(0x2200, 0x2270))
    | {0x2122, 0x25CF, 0x2026}
)


def drop_ligatures(font, keep=None):
    opts = subset.Options()
    opts.layout_features = [f for f in subset.Options().layout_features if f not in ("liga", "clig")]
    opts.glyph_names = True
    opts.notdef_outline = True
    opts.name_IDs = ["*"]
    opts.name_languages = ["*"]
    opts.hinting = True
    opts.legacy_kern = True
    s = subset.Subsetter(opts)
    s.populate(unicodes=sorted(u for u in font.getBestCmap() if keep is None or u in keep))
    s.subset(font)


def track(font, thousandths):
    upem = font["head"].unitsPerEm
    delta = round(thousandths * upem / 1000)
    hmtx = font["hmtx"]
    for name, (adv, lsb) in list(hmtx.metrics.items()):
        if adv > 0:
            hmtx.metrics[name] = (max(adv + delta, 0), lsb)


def rename(font, family, weight, style="Regular"):
    name = font["name"]
    ps = family.replace(" ", "") + "-" + style.replace(" ", "")
    for rec in list(name.names):
        if rec.nameID in (16, 17, 21, 22, 25):
            name.removeNames(nameID=rec.nameID)
    for nid, value in ((1, family), (2, style), (3, ps), (4, f"{family} {style}".strip()), (6, ps)):
        name.setName(value, nid, 3, 1, 0x409)
        name.setName(value, nid, 1, 0, 0)
    os2 = font["OS/2"]
    os2.usWeightClass = weight
    italic = "Italic" in style
    os2.fsSelection = (os2.fsSelection & ~0b1100001) | (0b1 if italic else 0b1000000)
    font["head"].macStyle = 0b10 if italic else 0
    if "STAT" in font:
        del font["STAT"]


def save(font, file):
    path = os.path.join(OUT, file)
    font.save(path)
    print(f"{file}: {os.path.getsize(path) // 1024} KB")


def main():
    bricolage, figtree, figtree_italic = sys.argv[1:4]
    for opsz, wght, t in DISPLAY:
        f = instancer.instantiateVariableFont(TTFont(bricolage), {"opsz": opsz, "wght": wght, "wdth": 100})
        drop_ligatures(f, LATIN)
        track(f, -t)
        rename(f, f"Baralho D{opsz} W{wght} T{t}", wght)
        save(f, f"display-{opsz}-{wght}-{t}.ttf")
    for wght, t in CAPS:
        f = instancer.instantiateVariableFont(TTFont(figtree), {"wght": wght})
        drop_ligatures(f)
        track(f, t)
        rename(f, f"Baralho Caps W{wght} T{t}", wght)
        save(f, f"caps-{wght}-{t}.ttf")
    for wght, italic in TEXT:
        f = instancer.instantiateVariableFont(TTFont(figtree_italic if italic else figtree), {"wght": wght})
        drop_ligatures(f)
        # One family, real weights: the shared screens ask for it by weight.
        name = f["name"]
        for rec in list(name.names):
            if rec.nameID in (16, 17, 21, 22, 25):
                name.removeNames(nameID=rec.nameID)
        style = {400: "Regular", 500: "Medium", 600: "SemiBold", 700: "Bold", 800: "ExtraBold"}[wght]
        full = style + (" Italic" if italic else "")
        if italic and wght == 400:
            full = "Italic"
        ribbi = (wght, italic) in ((400, False), (700, False), (400, True), (700, True))
        ps = "Figtree-" + full.replace(" ", "")
        if ribbi:
            values = ((1, "Figtree"), (2, full), (3, ps), (4, f"Figtree {full}"), (6, ps))
        else:
            values = ((1, f"Figtree {style}"), (2, "Italic" if italic else "Regular"), (3, ps),
                      (4, f"Figtree {full}"), (6, ps), (16, "Figtree"), (17, full))
        for nid, value in values:
            name.setName(value, nid, 3, 1, 0x409)
            name.setName(value, nid, 1, 0, 0)
        os2 = f["OS/2"]
        os2.usWeightClass = wght
        sel = os2.fsSelection & ~0b1100001
        if italic:
            sel |= 0b1
        if wght == 700 and ribbi:
            sel |= 0b100000
        if not italic and wght == 400:
            sel |= 0b1000000
        os2.fsSelection = sel
        f["head"].macStyle = (0b1 if wght == 700 and ribbi else 0) | (0b10 if italic else 0)
        if "STAT" in f:
            del f["STAT"]
        save(f, f"figtree-{wght}{'-italic' if italic else ''}.ttf")


if __name__ == "__main__":
    main()
