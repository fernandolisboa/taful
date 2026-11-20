#!/usr/bin/env python3
"""Builds the Salão style's fonts (D19) into ../fonts/.

GPUI has no letter-spacing and no variable-font axes, so every text style
of the mockup that needs one is a static font of its own:

- Fraunces (the serif): one instance per (optical size, weight) the mockup
  uses, with its letter-spacing added to the advance widths.
- Inter (the sans): the five weights as one family, plus one font per
  tracked label style.

Each single-style font is its own family, declared Regular 400, so asking
for it never makes a platform synthesize bold or italic.

Sources: `src/fonts/fraunces-*.woff2` (the variable fonts the mockups were
drawn with) and Inter 4.0 static OTFs (`--inter-dir`, Debian's `fonts-inter`
by default). Needs `fonttools` and `brotli`.

Usage: python3 apps/desktop/assets/salao/build/fonts.py
"""

import argparse
import pathlib

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE.parent / "fonts"

# (file stem, family, italic, opsz, wght, tracking in em, keep ligatures)
# The mockup's rule for each is in the comment.
SERIF = [
    # .brand .name
    ("brand", "Taful Salao Brand", True, 144, 560, -0.02, False),
    # h1, .verdict .g
    ("h1", "Taful Salao H1", False, 144, 480, -0.02, False),
    # .hero h2
    ("hero", "Taful Salao Hero", False, 144, 460, -0.025, False),
    # .stat .big and its <small>
    ("stat", "Taful Salao Stat", False, 144, 420, -0.03, False),
    ("stat-unit", "Taful Salao Stat Unit", False, 144, 420, 0, True),
    # .overall b
    ("overall", "Taful Salao Overall", False, 144, 440, 0, True),
    # .trk h3
    ("track", "Taful Salao Track", False, 144, 470, -0.015, False),
    # .pot .v
    ("pot", "Taful Salao Pot", False, 144, 460, -0.01, False),
    # .pc b
    ("rank", "Taful Salao Rank", False, 144, 600, 0, True),
    # .leaks h3 (auto optical size: its font size)
    ("title", "Taful Salao Title", False, 23, 480, -0.01, False),
    # .leak .n
    ("ordinal", "Taful Salao Ordinal", True, 26, 400, 0, True),
    # .leak .ev
    ("amount", "Taful Salao Amount", False, 21, 400, 0, True),
    # .med
    ("medal", "Taful Salao Medal", False, 22, 500, 0, True),
    # .trk .cnt b
    ("count", "Taful Salao Count", False, 18, 480, 0, True),
    # .seat .st
    ("stack", "Taful Salao Stack", False, 60, 400, 0, True),
    # .bet span
    ("bet", "Taful Salao Bet", False, 17, 500, 0, True),
    # .verdict .ev b
    ("loss", "Taful Salao Loss", False, 24, 460, 0, True),
    # .opt .e, .hist .st
    ("text", "Taful Salao Text", False, 16, 400, 0, True),
]

SANS_FAMILY = "Taful Salao Sans"
# (Inter file, weight class, style name)
SANS = [
    ("Inter-Regular.otf", 400, "Regular"),
    ("Inter-Medium.otf", 500, "Medium"),
    ("Inter-SemiBold.otf", 600, "SemiBold"),
    ("Inter-Bold.otf", 700, "Bold"),
    ("Inter-ExtraBold.otf", 800, "ExtraBold"),
]
# (file stem, family, Inter file, tracking in em)
TRACKED = [
    # .tagline
    ("sans-r02", "Taful Salao Sans R02", "Inter-Regular.otf", 0.02),
    # .sec em
    ("sans-r04", "Taful Salao Sans R04", "Inter-Regular.otf", 0.04),
    # .verdict .ev span
    ("sans-r14", "Taful Salao Sans R14", "Inter-Regular.otf", 0.14),
    # .nav .grp, .ctl .lab, .sec
    ("sans-r16", "Taful Salao Sans R16", "Inter-Regular.otf", 0.16),
    # .pot .lbl
    ("sans-r18", "Taful Salao Sans R18", "Inter-Regular.otf", 0.18),
    # .tag
    ("sans-s14", "Taful Salao Sans S14", "Inter-SemiBold.otf", 0.14),
    # .eyebrow
    ("sans-s18", "Taful Salao Sans S18", "Inter-SemiBold.otf", 0.18),
    # .seat .av
    ("sans-b02", "Taful Salao Sans B02", "Inter-Bold.otf", 0.02),
]

# What the sans keeps: Latin (both UI locales and names), punctuation,
# currency, arrows, math and the few symbols the UI prints.
SANS_UNICODES = (
    list(range(0x20, 0x7F))
    + list(range(0xA0, 0x250))
    + list(range(0x2000, 0x2070))
    + list(range(0x20A0, 0x20C0))
    + [0x2122, 0x2190, 0x2191, 0x2192, 0x2193, 0x2194]
    + list(range(0x2200, 0x2270))
    + [0x25A0, 0x25B8, 0x25CF, 0x25CB, 0x258F, 0x2713, 0x2715, 0x2717]
    + [0x2660, 0x2663, 0x2665, 0x2666, 0xFEFF, 0xFFFD]
)
# The tracked fonts only set short labels.
LABEL_UNICODES = list(range(0x20, 0x7F)) + list(range(0xA0, 0x180)) + list(range(0x2010, 0x2030))


def track(font, em):
    """Adds `em` of letter-spacing to every advance, as CSS does."""
    if not em:
        return
    add = round(em * font["head"].unitsPerEm)
    hmtx = font["hmtx"]
    for glyph, (advance, lsb) in list(hmtx.metrics.items()):
        if advance > 0:
            hmtx.metrics[glyph] = (max(advance + add, 0), lsb)
    font["hhea"].advanceWidthMax = max(a for a, _ in hmtx.metrics.values())


def name(font, family, style="Regular", typographic=None):
    """Renames the font. `typographic` = (family, style) for a weight that
    is not Regular or Bold of its family."""
    table = font["name"]
    keep = {0, 7, 8, 9, 11, 13, 14}
    table.names = [n for n in table.names if n.nameID in keep and n.platformID == 3]
    full = family if style == "Regular" else f"{family} {style}"
    ps = full.replace(" ", "")
    for name_id, value in [(1, family), (2, style), (3, ps), (4, full), (6, ps)]:
        table.setName(value, name_id, 3, 1, 0x409)
    if typographic:
        table.setName(typographic[0], 16, 3, 1, 0x409)
        table.setName(typographic[1], 17, 3, 1, 0x409)
    for tag in ("STAT", "fvar", "avar", "gvar", "HVAR", "MVAR", "DSIG"):
        if tag in font:
            del font[tag]


def upright_regular(font):
    """Declares the font Regular 400 upright, whatever it draws."""
    os2 = font["OS/2"]
    os2.usWeightClass = 400
    # Clear italic (0), bold (5) and oblique (9); set regular (6).
    os2.fsSelection = (os2.fsSelection & ~0b1000100001) | 0b1000000
    font["head"].macStyle = 0
    font["post"].italicAngle = 0


def drop_features(font, tags):
    if "GSUB" not in font:
        return
    gsub = font["GSUB"].table
    records = gsub.FeatureList.FeatureRecord
    dropped = {i for i, r in enumerate(records) if r.FeatureTag in tags}
    if not dropped:
        return
    remap = {}
    kept = []
    for i, r in enumerate(records):
        if i not in dropped:
            remap[i] = len(kept)
            kept.append(r)
    gsub.FeatureList.FeatureRecord = kept
    gsub.FeatureList.FeatureCount = len(kept)
    for script in gsub.ScriptList.ScriptRecord:
        systems = [ls.LangSys for ls in script.Script.LangSysRecord]
        if script.Script.DefaultLangSys:
            systems.append(script.Script.DefaultLangSys)
        for ls in systems:
            ls.FeatureIndex = [remap[i] for i in ls.FeatureIndex if i in remap]
            ls.FeatureCount = len(ls.FeatureIndex)


def one_line_box(font):
    """Gives the font one ascent and descent in all three places a platform
    may read them (`hhea`, OS/2 typographic, OS/2 Windows), so its baseline
    sits at the same height everywhere."""
    hhea, os2 = font["hhea"], font["OS/2"]
    hhea.lineGap = os2.sTypoLineGap = 0
    os2.sTypoAscender, os2.sTypoDescender = hhea.ascent, hhea.descent
    os2.usWinAscent, os2.usWinDescent = hhea.ascent, -hhea.descent
    os2.fsSelection |= 1 << 7  # USE_TYPO_METRICS


def save(font, stem):
    font.flavor = None
    one_line_box(font)
    path = OUT / f"{stem}.{'otf' if 'CFF ' in font else 'ttf'}"
    font.save(path)
    return path


def serif(stem, family, italic, opsz, wght, tracking, ligatures):
    src = HERE / "src/fonts" / ("fraunces-italic.woff2" if italic else "fraunces-roman.woff2")
    font = instancer.instantiateVariableFont(TTFont(src), {"opsz": opsz, "wght": wght})
    if not ligatures:
        drop_features(font, {"liga"})
    track(font, tracking)
    name(font, family)
    upright_regular(font)
    return save(font, stem)


def sans_subset(path, unicodes=SANS_UNICODES):
    font = TTFont(path)
    options = subset.Options()
    # What browsers apply by default, plus tabular figures.
    options.layout_features = ["ccmp", "locl", "calt", "kern", "mark", "mkmk", "tnum"]
    options.name_IDs = ["*"]
    options.notdef_outline = True
    options.hinting = False
    options.desubroutinize = True
    s = subset.Subsetter(options)
    s.populate(unicodes=unicodes)
    s.subset(font)
    return font


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--inter-dir", default="/usr/share/fonts/opentype/inter")
    args = ap.parse_args()
    inter = pathlib.Path(args.inter_dir)
    OUT.mkdir(parents=True, exist_ok=True)
    for old in list(OUT.glob("*.ttf")) + list(OUT.glob("*.otf")):
        old.unlink()
    built = [serif(*row) for row in SERIF]
    for file, weight, style in SANS:
        font = sans_subset(inter / file)
        ribbi = style in ("Regular", "Bold")
        name(
            font,
            SANS_FAMILY if ribbi else f"{SANS_FAMILY} {style}",
            style if ribbi else "Regular",
            None if ribbi else (SANS_FAMILY, style),
        )
        font["OS/2"].usWeightClass = weight
        built.append(save(font, f"sans-{weight}"))
    for stem, family, file, tracking in TRACKED:
        font = sans_subset(inter / file, LABEL_UNICODES)
        track(font, tracking)
        name(font, family)
        upright_regular(font)
        built.append(save(font, stem))
    total = 0
    for p in built:
        total += p.stat().st_size
        print(f"{p.stat().st_size:>8}  {p.name}")
    print(f"{total:>8}  total")


if __name__ == "__main__":
    main()
