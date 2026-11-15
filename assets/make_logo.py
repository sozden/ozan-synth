"""Writes the OZAN Synth logo files from one geometry (no font for the letters).

    python assets/make_logo.py

Letters are monoline capitals on a 100-unit cap height: O a circle, Z a saw tooth in
the accent colour, A an open peak (Λ, the attack of an envelope), N. Under the word a
line morphs from sine to saw to square: the waveforms the synth starts from.
The same proportions are drawn in crates/ozan/src/logo.rs.
"""

import math
from pathlib import Path

H = 100.0
S = 14.0          # stroke
GAP = 26.0
HALF = S / 2
WIDTHS = {"O": 100.0, "Z": 76.0, "A": 84.0, "N": 76.0}
WORD = sum(WIDTHS.values()) + 3 * GAP  # 414

CREAM = "#e9dfc8"
ORANGE = "#e8742a"
MUTED = "#a39985"
DARK = "#161514"


def letters(ox, oy, text=CREAM, accent=ORANGE):
    st = f'fill="none" stroke-width="{S:g}" stroke-linecap="round" stroke-linejoin="round"'
    top, bot = oy + HALF, oy + H - HALF
    x = ox
    out = [f'<circle cx="{x + 50:g}" cy="{oy + 50:g}" r="{50 - HALF:g}" stroke="{text}" {st}/>']
    x += WIDTHS["O"] + GAP
    l, r = x + HALF, x + WIDTHS["Z"] - HALF
    out.append(f'<path d="M{l:g} {top:g} H{r:g} L{l:g} {bot:g} H{r:g}" stroke="{accent}" {st}/>')
    x += WIDTHS["Z"] + GAP
    l, r = x + HALF, x + WIDTHS["A"] - HALF
    out.append(f'<path d="M{l:g} {bot:g} L{(l + r) / 2:g} {top:g} L{r:g} {bot:g}" stroke="{text}" {st}/>')
    x += WIDTHS["A"] + GAP
    l, r = x + HALF, x + WIDTHS["N"] - HALF
    out.append(f'<path d="M{l:g} {bot:g} V{top:g} L{r:g} {bot:g} V{top:g}" stroke="{text}" {st}/>')
    return out


def wave(ox, y, width, amp, color, stroke):
    """Sine, then saw, then square, a third of the width each."""
    third = width / 3
    pts = []
    cycles = 2
    for i in range(61):
        t = i / 60
        pts.append((ox + t * third, y - amp * math.sin(t * cycles * 2 * math.pi)))
    x0 = ox + third
    period = third / cycles
    for c in range(cycles):
        a = x0 + c * period
        pts += [(a, y), (a + period / 2, y - amp), (a + period / 2, y + amp), (a + period, y)]
    x0 = ox + 2 * third
    for c in range(cycles):
        a = x0 + c * period
        pts += [(a, y), (a, y - amp), (a + period / 2, y - amp), (a + period / 2, y + amp), (a + period, y + amp), (a + period, y)]
    d = "M" + " L".join(f"{px:.1f} {py:.1f}" for px, py in pts)
    return f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{stroke:g}" stroke-linecap="round" stroke-linejoin="round"/>'


def synth_text(cx, y, size, color, spacing):
    return (f'<text x="{cx:g}" y="{y:g}" text-anchor="middle" font-family="Helvetica, Arial, sans-serif" '
            f'font-size="{size:g}" font-weight="600" letter-spacing="{spacing:g}" fill="{color}">SYNTH</text>')


def svg(w, h, body):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:g} {h:g}" width="{w:g}" height="{h:g}">\n  ' + "\n  ".join(body) + "\n</svg>\n"


out = Path(__file__).parent

# Full logo on a dark plate: word, waveform line, SYNTH.
pad = 64
w = WORD + 2 * pad
body = [f'<rect width="{w:g}" height="300" rx="28" fill="{DARK}"/>']
body += letters(pad, 50)
body.append(wave(pad, 196, WORD, 9, MUTED, 4))
body.append(synth_text(w / 2 + 9, 262, 30, CREAM, 18))
(out / "logo.svg").write_text(svg(w, 300, body), encoding="utf-8")

# Transparent lockup for light or dark pages: word and SYNTH only.
body = letters(HALF, HALF) + [synth_text(WORD / 2 + 9, H + S + 46, 30, MUTED, 18)]
(out / "wordmark.svg").write_text(svg(WORD + S, H + S + 56, body), encoding="utf-8")

# Icon: O with the saw-tooth Z inside.
icon = [
    f'<rect width="256" height="256" rx="56" fill="{DARK}"/>',
    f'<circle cx="128" cy="128" r="78" fill="none" stroke="{CREAM}" stroke-width="26"/>',
    f'<path d="M92 94 H164 L92 162 H164" fill="none" stroke="{ORANGE}" stroke-width="20" stroke-linecap="round" stroke-linejoin="round"/>',
]
(out / "icon.svg").write_text(svg(256, 256, icon), encoding="utf-8")
print("logo.svg, wordmark.svg, icon.svg")
