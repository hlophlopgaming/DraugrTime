#!/usr/bin/env python3
"""Render Draugr Time branding from one geometry source.

Writes vector sources (logo/*.svg) and the raster files the app and packaging
use (PNG, ICO, ICNS). Needs only Pillow. Run from anywhere:

    python3 scripts/render_logos.py

Shapes are defined in a 128x128 box (the tray glyph in 32x32) so the SVGs and
PNGs always match. Edit the geometry here, never the generated files.
"""
from __future__ import annotations

import io
import math
import struct
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
LOGO = ROOT / "logo"
SS = 8  # supersampling factor

# Brand colours (see docs/design.md)
TILE = (16, 20, 26)  # side, #10141A
BONE = (236, 231, 220)  # text, #ECE7DC
ACCENT = (95, 212, 184)  # accent, #5FD4B8
TRAY_ON = "#2BB596"  # recording: brand mint, readable on dark and light panels
TRAY_OFF = "#D55A46"  # not recording / paused / error

# ---------------------------------------------------------------- geometry
# Paths use absolute M/L/C/Q/Z commands as (op, *points).

CROWN = [("M", (26, 36)), ("L", (26, 14)), ("L", (44, 26)), ("L", (64, 6)),
         ("L", (84, 26)), ("L", (102, 14)), ("L", (102, 36)), ("Z",)]
SKULL = [("M", (40, 104)),
         ("C", (30, 96), (26, 86), (26, 76)),
         ("C", (26, 54), (43, 42), (64, 42)),
         ("C", (85, 42), (102, 54), (102, 76)),
         ("C", (102, 86), (98, 96), (88, 104)),
         ("L", (88, 116)),
         ("Q", (88, 124), (81, 124)), ("Q", (75, 124), (74, 117)),
         ("Q", (70, 125), (64, 125)), ("Q", (58, 125), (54, 117)),
         ("Q", (53, 124), (47, 124)), ("Q", (40, 124), (40, 116)),
         ("Z",)]
EYES = [((49, 76), (11, 12)), ((79, 76), (11, 12))]  # centre, radii
NOSE = [("M", (64, 91)), ("C", (68, 91), (70, 97), (70, 100)),
        ("C", (70, 103), (67, 105), (64, 105)), ("C", (61, 105), (58, 103), (58, 100)),
        ("C", (58, 97), (60, 91), (64, 91)), ("Z",)]

# Tray glyph, 32x32: crown fused onto a clock ring, two hands. One colour.
T_CROWN = [("M", (7, 12)), ("L", (7, 2.5)), ("L", (11.5, 6.5)), ("L", (16, 1)),
           ("L", (20.5, 6.5)), ("L", (25, 2.5)), ("L", (25, 12)), ("Z",)]
T_RING = ((16, 19.5), 11.5, 8.6)  # centre, outer r, inner r
T_HOUR = ((16, 19.5), (16, 13.2))
T_MINUTE = ((16, 19.5), (20.6, 23.6))
T_HAND_W = 2.6


# ---------------------------------------------------------------- helpers
def flatten(path, steps=24):
    pts, polys = [], []
    for cmd in path:
        op = cmd[0]
        if op == "M":
            if pts:
                polys.append(pts)
            pts = [cmd[1]]
        elif op == "L":
            pts.append(cmd[1])
        elif op == "C":
            p0, (p1, p2, p3) = pts[-1], cmd[1:]
            for i in range(1, steps + 1):
                t = i / steps
                pts.append(tuple((1 - t) ** 3 * a + 3 * (1 - t) ** 2 * t * b + 3 * (1 - t) * t * t * c + t ** 3 * d
                                 for a, b, c, d in zip(p0, p1, p2, p3)))
        elif op == "Q":
            p0, (p1, p2) = pts[-1], cmd[1:]
            for i in range(1, steps + 1):
                t = i / steps
                pts.append(tuple((1 - t) ** 2 * a + 2 * (1 - t) * t * b + t * t * c for a, b, c in zip(p0, p1, p2)))
        elif op == "Z":
            polys.append(pts)
            pts = []
    if pts:
        polys.append(pts)
    return polys


def d_attr(path):
    out = []
    for cmd in path:
        op = cmd[0]
        out.append(op + (" " + " ".join(f"{x:g} {y:g}" for x, y in cmd[1:]) if len(cmd) > 1 else ""))
    return " ".join(out)


def ellipse_d(c, r):
    (cx, cy), (rx, ry) = c, r
    return f"M {cx - rx:g} {cy:g} a {rx:g} {ry:g} 0 1 0 {2 * rx:g} 0 a {rx:g} {ry:g} 0 1 0 {-2 * rx:g} 0 Z"


class Canvas:
    """Supersampled single-channel coverage mask."""

    def __init__(self, size, unit):
        self.size, self.k = size, size * SS / unit
        self.img = Image.new("L", (size * SS, size * SS), 0)
        self.draw = ImageDraw.Draw(self.img)

    def _p(self, pts, dx=0, dy=0, s=1.0):
        return [((x * s + dx) * self.k, (y * s + dy) * self.k) for x, y in pts]

    def path(self, path, value=255, **t):
        for poly in flatten(path):
            self.draw.polygon(self._p(poly, **t), fill=value)

    def ellipse(self, c, r, value=255, dx=0, dy=0, s=1.0):
        (cx, cy), (rx, ry) = c, r
        box = self._p([(cx - rx, cy - ry), (cx + rx, cy + ry)], dx, dy, s)
        self.draw.ellipse(box, fill=value)

    def line(self, a, b, w, value=255):
        pa, pb = self._p([a, b])
        self.draw.line([pa, pb], fill=value, width=round(w * self.k))
        for (x, y) in (pa, pb):
            r = w * self.k / 2
            self.draw.ellipse((x - r, y - r, x + r, y + r), fill=value)

    def mask(self):
        return self.img.resize((self.size, self.size), Image.LANCZOS)


def solid(mask, rgb):
    out = Image.new("RGBA", mask.size, rgb + (0,))
    out.putalpha(mask)
    return out


# ---------------------------------------------------------------- artwork
def mark_mask(size=128, s=1.0, dx=0.0, dy=0.0, part="all"):
    c = Canvas(size, 128)
    t = dict(dx=dx, dy=dy, s=s)
    if part in ("all", "crown"):
        c.path(CROWN, **t)
    if part in ("all", "skull"):
        c.path(SKULL, **t)
        for e in EYES:
            c.ellipse(*e, value=0, **t)
        c.path(NOSE, value=0, **t)
    return c.mask()


def mono_mark(rgb):
    return solid(mark_mask(), rgb)


def app_icon(size=128):
    c = Canvas(size, 128)
    c.draw.rounded_rectangle((0, 0, size * SS - 1, size * SS - 1), radius=28 * c.k, fill=255)
    tile = solid(c.mask(), TILE)
    s, dx, dy = 0.74, 16.6, 15.0
    img = tile.copy()
    img.alpha_composite(solid(mark_mask(size, s, dx, dy, "skull"), BONE))
    img.alpha_composite(solid(mark_mask(size, s, dx, dy, "crown"), ACCENT))
    return img


def tray_mask(size=32):
    c = Canvas(size, 32)
    (cx, cy), ro, ri = T_RING
    c.ellipse((cx, cy), (ro, ro))
    c.ellipse((cx, cy), (ri, ri), value=0)
    c.path(T_CROWN)
    c.line(*T_HOUR, T_HAND_W)
    c.line(*T_MINUTE, T_HAND_W)
    return c.mask()


# ---------------------------------------------------------------- SVG
def svg_mark(fill="#000000"):
    holes = " ".join(ellipse_d(*e) for e in EYES) + " " + d_attr(NOSE)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" width="128" height="128">
  <title>Draugr Time mark</title>
  <path fill="{fill}" d="{d_attr(CROWN)}"/>
  <path fill="{fill}" fill-rule="evenodd" d="{d_attr(SKULL)} {holes}"/>
</svg>
'''


def svg_app():
    holes = " ".join(ellipse_d(*e) for e in EYES) + " " + d_attr(NOSE)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" width="128" height="128">
  <title>Draugr Time app icon</title>
  <rect width="128" height="128" rx="28" fill="#10141A"/>
  <g transform="translate(16.6 15) scale(0.74)">
    <path fill="#5FD4B8" d="{d_attr(CROWN)}"/>
    <path fill="#ECE7DC" fill-rule="evenodd" d="{d_attr(SKULL)} {holes}"/>
  </g>
</svg>
'''


def svg_tray(fill=TRAY_ON):
    (cx, cy), ro, ri = T_RING
    ring = ellipse_d((cx, cy), (ro, ro)) + " " + ellipse_d((cx, cy), (ri, ri))
    (a, b), (c, d) = T_HOUR, T_MINUTE
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="32" height="32">
  <title>Draugr Time tray icon (recolour by state: recording #2BB596, not recording #D55A46)</title>
  <path fill="{fill}" fill-rule="evenodd" d="{ring}"/>
  <path fill="{fill}" d="{d_attr(T_CROWN)}"/>
  <path stroke="{fill}" stroke-width="{T_HAND_W}" stroke-linecap="round" fill="none" d="M {a[0]:g} {a[1]:g} L {b[0]:g} {b[1]:g} M {c[0]:g} {c[1]:g} L {d[0]:g} {d[1]:g}"/>
</svg>
'''


# ---------------------------------------------------------------- output
def png_bytes(img):
    buf = io.BytesIO()
    img.save(buf, "PNG", optimize=True)
    return buf.getvalue()


def ico_wrap(png, size=128):
    entry = struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(png), 22)
    return struct.pack("<HHH", 0, 1, 1) + entry + png


def icns_wrap(png):
    chunk = b"ic07" + struct.pack(">I", 8 + len(png)) + png
    return b"icns" + struct.pack(">I", 8 + len(chunk)) + chunk


def main():
    LOGO.mkdir(exist_ok=True)
    (LOGO / "draugr-time-mark.svg").write_text(svg_mark())
    (LOGO / "draugr-time-app.svg").write_text(svg_app())
    (LOGO / "draugr-time-tray.svg").write_text(svg_tray())

    black = mono_mark((0, 0, 0))
    white = mono_mark((255, 255, 255))
    black.save(LOGO / "blackDraugr128.png", optimize=True)
    white.save(LOGO / "whiteDraugr128.png", optimize=True)

    app = png_bytes(app_icon(128))
    (LOGO / "appIcon128.png").write_bytes(app)
    (LOGO / "draugr-time.ico").write_bytes(ico_wrap(app))
    (LOGO / "draugr-time.icns").write_bytes(icns_wrap(app))

    solid(tray_mask(32), (255, 255, 255)).save(LOGO / "tray32.png", optimize=True)
    print("wrote", ", ".join(sorted(p.name for p in LOGO.iterdir())))


if __name__ == "__main__":
    main()
