"""Builds hotshot's icon font from its own drawings: plain lines on a 24
by 24 square, with round ends and joins.

    uv run --no-project --with fonttools --with picosvg \
        tools/icon_font.py tools/icons.txt assets/icons.ttf [STROKE]

tools/icons.txt has a line for each icon hotshot uses: the name of its
constant in `icons`, and the code point the font has it at, in hex. To
add an icon, give it a line there with a code point of its own and draw
it in DRAWINGS below.

The line is 1.8 wide: as heavy as the letters beside it at 13 to 24
pixels, and still open enough in the small marks inside a drawing.
"""

import re
import sys

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.cu2quPen import Cu2QuPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.svgLib.path import parse_path
from picosvg.svg import SVG

LINE = 'fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"'

# Each icon as one path of lines. A dot is a line a hundredth long, which
# its round ends make round.
DRAWINGS = {
    "SELECT": "M6 3l13 7-6 1.6L10 18z",
    "ARROW": "M5 19L19 5M9 5h10v10",
    "LINE": "M5 19L19 5",
    "RECT": "M6 5h12a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z",
    "ELLIPSE": "M3 12a9 7 0 1 0 18 0a9 7 0 1 0-18 0",
    "PEN": "M4 20l4-1L19 8l-3-3L5 16zM14 7l3 3",
    "MARKER": "M15 4l5 5-8 8H7v-5zM7 17l-3 3",
    "TEXT": "M5 7V5h14v2M12 5v14M9 19h6",
    # A mosaic: what a pixelated patch looks like, five of its nine
    # squares dark. Small enough to stay apart at the bar's size.
    "BLUR": "M7 3h10a4 4 0 0 1 4 4v10a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4zM7.5 7.5h1.5v1.5H7.5zM15 7.5h1.5v1.5H15zM11.25 11.25h1.5v1.5h-1.5zM7.5 15h1.5v1.5H7.5zM15 15h1.5v1.5H15z",
    "COUNTER": "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M10.5 9l2-1.5V16",
    "PIPETTE": "M4 20l5-1 9-9-4-4-9 9zM13 7l4 4M15 3l6 6",
    "UNDO": "M9 14L4 9l5-5M4 9h11a5 5 0 0 1 0 10h-3",
    "REDO": "M15 14l5-5-5-5M20 9H9a5 5 0 0 0 0 10h3",
    "COPY": "M8 8h12v12H8zM16 8V4H4v12h4",
    "SAVE": "M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6",
    "CLOSE": "M6 6l12 12M18 6L6 18",
    "CHECK": "M5 12.5l4.5 4.5L19 7",
    "PIN": "M9 3h6l-1 6 3 3v2H7v-2l3-3zM12 14v7",
    # A rectangle hatched across: one drawn filled.
    "FILLED": "M6 5h12a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2zM4 13l9-8M4 19l15-14M10 19l10-9M16 19l4-3.5",
    "SPOTLIGHT": "M12 2v3M12 19v3M2 12h3M19 12h3M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8",
    "ZOOM": "M10 3a7 7 0 1 0 0 14a7 7 0 1 0 0-14M15 15l6 6M10 7v6M7 10h6",
    "BADGE": "M4 7h11l5 5-5 5H4zM8 12h.01",
    "FRAME": "M3 5h18v14H3zM7 9h10v6H7z",
    "STICKER": "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18M8.5 14.5a4.5 4.5 0 0 0 7 0M9 9.5v.5M15 9.5v.5",
    "DOWN": "M6 9l6 6 6-6",
    # A die showing five.
    "DICE": "M7 3h10a4 4 0 0 1 4 4v10a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4zM8.5 8.5h.01M15.5 8.5h.01M12 12h.01M8.5 15.5h.01M15.5 15.5h.01",
    # A mouse.
    "POINTER": "M12 3a6 6 0 0 0-6 6v6a6 6 0 0 0 12 0V9a6 6 0 0 0-6-6M12 7v4",
    "RECORD": "M3 7h12v10H3zM15 10.5l6-3.5v10l-6-3.5",
}

EM = 1000
GRID = 24


def outline(body, stroke):
    """The filled outline of a drawing made of lines `stroke` wide, as a
    glyph: the 24 by 24 square taken to the font's em, the right way up."""
    body = re.sub(r'stroke-width="[0-9.]+"', f'stroke-width="{stroke}"', body).replace("currentColor", "#000")
    svg = SVG.fromstring(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {GRID} {GRID}">{body}</svg>')
    pen = TTGlyphPen(None)
    scale = EM / GRID
    to_em = TransformPen(Cu2QuPen(pen, max_err=1.0, reverse_direction=False), (scale, 0, 0, -scale, 0, EM))
    for shape in svg.topicosvg().shapes():
        parse_path(shape.as_path().d, to_em)
    return pen.glyph()


def main():
    consts, out = sys.argv[1:3]
    stroke = sys.argv[3] if len(sys.argv) > 3 else "1.8"

    glyphs, cmap, missing = {".notdef": TTGlyphPen(None).glyph()}, {}, []
    for line in open(consts):
        if not line.strip():
            continue
        name, point = line.split()
        if name not in DRAWINGS:
            missing.append(name)
            continue
        glyphs[name] = outline(f'<path d="{DRAWINGS[name]}" {LINE}/>', stroke)
        cmap[int(point, 16)] = name
    if missing:
        sys.exit(f"no drawing for: {', '.join(missing)}")

    font = FontBuilder(EM, isTTF=True)
    font.setupGlyphOrder(list(glyphs))
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics({name: (EM, 0) for name in glyphs})
    font.setupHorizontalHeader(ascent=EM, descent=0)
    font.setupNameTable({"familyName": "hotshot icons", "styleName": "Regular"})
    font.setupOS2(sTypoAscender=EM, sTypoDescender=0, usWinAscent=EM, usWinDescent=0)
    font.setupPost()
    font.save(out)
    print(f"{len(glyphs) - 1} icons")


if __name__ == "__main__":
    main()
