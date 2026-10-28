"""Builds hotshot's icon font from Solar's linear icons, drawn at a
heavier line than Solar's own, with a few drawings of its own for the
shapes Solar has no icon of.

    uv run --no-project --with fonttools --with picosvg \
        tools/icon_font.py ICONS_JSON tools/icons.txt assets/icons.ttf [STROKE]

ICONS_JSON is the set as Iconify publishes it:
https://cdn.jsdelivr.net/npm/@iconify-json/solar/icons.json

tools/icons.txt has a line for each icon hotshot uses: the name of its
constant in `icons`, and the code point the font has it at, in hex. To
add an icon, give it a line there with a code point of its own and say
in PICKS below which of Solar's drawings it is, or draw it in OWN.

The line is 2.25 wide where Solar draws it at 1.5: beside text at 13 to
24 pixels the thinner line is lighter than the letters. This is the same
weight jog's icons are drawn at.
"""

import json
import re
import sys

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.cu2quPen import Cu2QuPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.svgLib.path import parse_path
from picosvg.svg import SVG

# hotshot's name for an icon, and the Solar drawing that stands for it.
PICKS = {
    "SELECT": "cursor", "ARROW": "arrow-right-up", "PEN": "pen", "TEXT": "text-bold",
    "PIPETTE": "pipette", "UNDO": "undo-left", "REDO": "undo-right", "COPY": "copy",
    "SAVE": "diskette", "CLOSE": "close", "CHECK": "check", "TRASH": "trash-bin-minimalistic",
    "PIN": "pin", "MARKER": "paint-roller", "MONITOR": "monitor", "FOLDER": "folder",
    "SPOTLIGHT": "flashlight-on", "ZOOM": "magnifier-zoom-in", "BADGE": "tag-horizontal",
    "FRAME": "gallery-wide", "STICKER": "smile-circle", "DOWN": "alt-arrow-down",
    "POINTER": "cursor-square", "RECORD": "videocamera-record", "DUPLICATE": "copy-add",
    "CAMERA": "camera", "EYE": "eye", "EYE_OFF": "eye-closed", "LOCK": "lock-keyhole-minimalistic",
    "FILE": "file", "PICTURE": "gallery", "FOLDER_OPEN": "folder-open", "HOME": "home",
    "CLOCK": "clock-circle", "DRAFTS": "documents", "STAR": "star", "EXPORT": "export",
    "SEARCH": "magnifier", "LINK": "link-minimalistic-2", "RIGHT": "alt-arrow-right", "CROP": "crop",
    "LAYERS": "layers-minimalistic", "ADD_PICTURE": "gallery-add", "MENU": "menu-dots",
    "ALIGN_LEFT": "align-left", "ALIGN_HCENTER": "align-horizontal-center", "ALIGN_RIGHT": "align-right",
    "ALIGN_TOP": "align-top", "ALIGN_VCENTER": "align-vertical-center", "ALIGN_BOTTOM": "align-bottom",
    "SWAP": "square-transfer-horizontal", "SETTINGS": "settings", "DOWNLOAD": "download-minimalistic",
}

LINE = 'fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"'

# Drawings of hotshot's own, on Solar's 24 by 24 square and in its manner:
# the plain shapes the tools draw, which Solar has no icons of.
OWN = {
    "LINE": f'<path d="M5 19L19 5" {LINE}/>',
    # A frame, as Figma draws its tool: two lines each way.
    "HASH": f'<path d="M8 3.5v17M16 3.5v17M3.5 8h17M3.5 16h17" {LINE}/>',
    "PLUS": f'<path d="M12 5v14M5 12h14" {LINE}/>',
    "MINUS": f'<path d="M5 12h14" {LINE}/>',
    # Spaced evenly across, and down: three bars between two walls.
    "DIST_H": f'<path d="M3.5 3.5v17M20.5 3.5v17" {LINE}/><rect x="9.5" y="7" width="5" height="10" rx="1.5" {LINE}/>',
    "DIST_V": f'<path d="M3.5 3.5h17M3.5 20.5h17" {LINE}/><rect x="7" y="9.5" width="10" height="5" rx="1.5" {LINE}/>',
    # Turned, round, kept to its shape, and flipped: for the fields
    # that set each.
    "ROTATE": f'<path d="M4.5 12a7.5 7.5 0 1 0 2.2-5.3M4.5 4v3.5H8" {LINE}/>',
    "RADIUS": f'<path d="M5 19v-6a8 8 0 0 1 8-8h6" {LINE}/>',
    "RATIO": f'<path d="M9 7H7a5 5 0 0 0 0 10h2M15 7h2a5 5 0 0 1 0 10h-2M8.5 12h7" {LINE}/>',
    "FLIP": f'<path d="M12 3v18M9 7L4 17h5zM15 7l5 10h-5z" {LINE}/>',
    "RECT": f'<rect x="3" y="5" width="18" height="14" rx="4" {LINE}/>',
    "ELLIPSE": f'<ellipse cx="12" cy="12" rx="9.25" ry="7.25" {LINE}/>',
    # The board's tools Solar has no icon of: an open hand, to move the
    # page by (after Lucide's, ISC); a note, its corner turned up; and a
    # diamond, for decisions.
    "HAND": f'<path d="M18 11V6a2 2 0 0 0-4 0M14 10V4a2 2 0 0 0-4 0v2M10 10.5V6a2 2 0 0 0-4 0v8M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-6-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15" {LINE}/>',
    "STICKY": f'<path d="M20 13.5V7a3 3 0 0 0-3-3H7a3 3 0 0 0-3 3v10a3 3 0 0 0 3 3h6.5zM13.5 20v-3.5a3 3 0 0 1 3-3H20" {LINE}/>',
    "DIAMOND": f'<path d="M12 3.2L20.8 12L12 20.8L3.2 12Z" {LINE}/>',
    "COUNTER": f'<circle cx="12" cy="12" r="9.25" {LINE}/><path d="M10.25 9.75L12.75 8V16" {LINE}/>',
    # A mosaic: what a pixelated patch looks like.
    "BLUR": (
        f'<rect x="3" y="3" width="18" height="18" rx="4.5" {LINE}/>'
        '<rect x="7" y="7" width="3.4" height="3.4" rx="0.6" fill="currentColor"/>'
        '<rect x="13.6" y="7" width="3.4" height="3.4" rx="0.6" fill="currentColor"/>'
        '<rect x="10.3" y="10.3" width="3.4" height="3.4" rx="0.6" fill="currentColor"/>'
        '<rect x="7" y="13.6" width="3.4" height="3.4" rx="0.6" fill="currentColor"/>'
        '<rect x="13.6" y="13.6" width="3.4" height="3.4" rx="0.6" fill="currentColor"/>'
    ),
    # Two squares, one over the other: the solid one is the one picked
    # out, in front of the other, or behind it, where only as much of
    # it shows as the one in front leaves.
    "FORWARD": f'<rect x="9" y="9" width="12" height="12" rx="3" {LINE}/><rect x="3" y="3" width="12" height="12" rx="3" fill="currentColor"/>',
    "BACKWARD": f'<rect x="3" y="3" width="12" height="12" rx="3" {LINE}/><path d="M17 9h1a3 3 0 0 1 3 3v6a3 3 0 0 1-3 3h-6a3 3 0 0 1-3-3v-1h5a3 3 0 0 0 3-3z" fill="currentColor"/>',
    # A die showing five.
    "DICE": (
        f'<rect x="3" y="3" width="18" height="18" rx="5" {LINE}/>'
        '<circle cx="8.2" cy="8.2" r="1.5" fill="currentColor"/><circle cx="15.8" cy="8.2" r="1.5" fill="currentColor"/>'
        '<circle cx="12" cy="12" r="1.5" fill="currentColor"/>'
        '<circle cx="8.2" cy="15.8" r="1.5" fill="currentColor"/><circle cx="15.8" cy="15.8" r="1.5" fill="currentColor"/>'
    ),
}

EM = 1000
GRID = 24


def outline(body, stroke):
    """The filled outline of a drawing made of lines `stroke` wide, as a
    glyph: Solar's 24 by 24 square taken to the font's em, the right way up."""
    body = re.sub(r'stroke-width="[0-9.]+"', f'stroke-width="{stroke}"', body).replace("currentColor", "#000")
    svg = SVG.fromstring(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {GRID} {GRID}">{body}</svg>')
    pen = TTGlyphPen(None)
    scale = EM / GRID
    to_em = TransformPen(Cu2QuPen(pen, max_err=1.0, reverse_direction=False), (scale, 0, 0, -scale, 0, EM))
    for shape in svg.topicosvg().shapes():
        parse_path(shape.as_path().d, to_em)
    return pen.glyph()


def main():
    icons_json, consts, out = sys.argv[1:4]
    stroke = sys.argv[4] if len(sys.argv) > 4 else "2.25"
    solar = json.load(open(icons_json))["icons"]

    glyphs, cmap, missing = {".notdef": TTGlyphPen(None).glyph()}, {}, []
    for line in open(consts):
        if not line.strip():
            continue
        name, point = line.split()
        pick = PICKS.get(name, "")
        drawing = OWN.get(name) or (solar.get(pick if pick.endswith("-bold") else pick + "-linear") or {}).get("body")
        if not drawing:
            missing.append(f"{name} ({PICKS.get(name, 'no pick')})")
            continue
        glyphs[name] = outline(drawing, stroke)
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
