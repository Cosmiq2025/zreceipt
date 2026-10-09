"""Generates the zreceipt logo files and the security-print patterns.

Run: python3 tools/brand.py   (from the repo root)
Outputs SVGs into web/brand/ and web/assets/.
"""
import math
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

ROOT = Path(__file__).resolve().parent.parent
BRAND = ROOT / "web" / "brand"
ASSETS = ROOT / "web" / "assets"
BRAND.mkdir(parents=True, exist_ok=True)
ASSETS.mkdir(parents=True, exist_ok=True)

INK = "#000000"
PAPER = "#FFFFFF"
SEAL = "#F4B728"   # Zcash yellow: the one line you disclose
SEAL_DARK_BG = "#F4B728"


# ---------- Mark ----------
# A tear-off receipt on a 64 grid. Two lines are redacted (filled), one is
# disclosed (open, green). The torn bottom edge is a zigzag, a quiet "z".
def receipt_outline():
    x0, x1, top, base = 9, 41, 7, 52
    teeth = 5
    w = (x1 - x0) / teeth
    d = f"M{x0} {top + 2.5} Q{x0} {top} {x0 + 2.5} {top} L{x1 - 2.5} {top} Q{x1} {top} {x1} {top + 2.5} L{x1} {base}"
    for i in range(teeth):
        xa = x1 - w * i
        d += f" L{xa - w / 2:.2f} {base + 5} L{xa - w:.2f} {base}"
    d += " Z"
    return d


def mark_group(paper, ink, seal, stroke=3, gap=None):
    # One line is pulled out of the receipt: the payment you disclose.
    gap = gap or paper
    return f"""<g>
  <path d="{receipt_outline()}" fill="{paper}" stroke="{ink}" stroke-width="{stroke}" stroke-linejoin="round"/>
  <rect x="15" y="15" width="20" height="6" fill="{ink}"/>
  <rect x="39.2" y="23" width="3.6" height="13" fill="{gap}"/>
  <rect x="15" y="25.5" width="42" height="8" fill="{seal}"/>
  <rect x="15" y="38" width="20" height="6" fill="{ink}"/>
</g>"""


def svg(w, h, body, extra=""):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}"{extra}>\n{body}\n</svg>\n'


# ---------- Wordmark: Source Serif 4 Semibold, converted to outlines ----------
FONT = TTFont(str(ROOT / "tools" / "Onest-SemiBold.ttf"))
GS = FONT.getGlyphSet()
CMAP = FONT.getBestCmap()
UPM = FONT["head"].unitsPerEm
HMTX = FONT["hmtx"]


def word_path(text, size, x, baseline, tracking=-0.025):
    scale = size / UPM
    out = []
    cx = x
    for ch in text:
        g = CMAP[ord(ch)]
        pen = SVGPathPen(GS)
        tp = TransformPen(pen, (scale, 0, 0, -scale, cx, baseline))
        GS[g].draw(tp)
        out.append(pen.getCommands())
        cx += HMTX[g][0] * scale + tracking * size
    return " ".join(out), cx - x


def write_logos():
    # Mark, colour, transparent
    (BRAND / "zreceipt-mark.svg").write_text(svg(64, 64, mark_group(PAPER, INK, SEAL)))
    # Mark, one colour (for stamps, faxes, print)
    (BRAND / "zreceipt-mark-mono.svg").write_text(svg(64, 64, mark_group("#FFFFFF", "#000000", "#000000")))
    # App icon / favicon: ink square, paper receipt
    icon = f'<rect width="64" height="64" rx="14" fill="{INK}"/>\n<g transform="translate(3 2)">' + mark_group(PAPER, PAPER, SEAL_DARK_BG, stroke=0, gap=INK).replace(f'width="20" height="6" fill="{PAPER}"', f'width="20" height="6" fill="{INK}"') + '</g>'
    (BRAND / "zreceipt-icon.svg").write_text(svg(64, 64, icon))
    (ROOT / "web" / "favicon.svg").write_text(svg(64, 64, icon))

    # Horizontal lockup: mark + wordmark
    for name, paper, ink, seal in [
        ("zreceipt-logo.svg", PAPER, INK, SEAL),
        ("zreceipt-logo-white.svg", "#000000", "#FFFFFF", SEAL_DARK_BG),
    ]:
        d, width = word_path("zreceipt", 38, 68, 43)
        total = math.ceil(68 + width + 4)
        body = mark_group(paper, ink, seal) + f'\n<path d="{d}" fill="{ink}"/>'
        (BRAND / name).write_text(svg(total, 64, body))


# ---------- Security print patterns ----------
def guilloche_band(w=600, h=28, waves=7, lines=9):
    """A horizontal band of interlaced sinusoids, as on cheques and banknotes."""
    paths = []
    for k in range(lines):
        phase = k * math.pi / lines
        pts = []
        for i in range(0, w + 1):
            x = i
            t = x / w * 2 * math.pi * waves
            y = h / 2 + (h / 2 - 2) * math.sin(t + phase) * (0.55 + 0.45 * math.cos(t / waves * 2 + phase))
            pts.append(f"{x:.1f},{y:.2f}")
        paths.append(f'<polyline points="{" ".join(pts)}"/>')
    body = f'<g fill="none" stroke="#1B2B4B" stroke-width="0.45" stroke-opacity="0.55">{"".join(paths)}</g>'
    return svg(w, h, body, ' preserveAspectRatio="none"')


def rosette(r_out=120, petals=24, rings=7):
    """A guilloche rosette: epitrochoids with shifted phases."""
    size = r_out * 2 + 8
    c = size / 2
    paths = []
    for k in range(rings):
        R = r_out * (0.62 + 0.05 * k)
        a = r_out * (0.24 - 0.018 * k)
        pts = []
        steps = 720
        for i in range(steps + 1):
            t = i / steps * 2 * math.pi
            rr = R + a * math.sin(petals * t + k * 0.6)
            pts.append(f"{c + rr * math.cos(t):.2f},{c + rr * math.sin(t):.2f}")
        paths.append(f'<polyline points="{" ".join(pts)}"/>')
    body = f'<g fill="none" stroke="#1B2B4B" stroke-width="0.5" stroke-opacity="0.5">{"".join(paths)}</g>'
    return svg(size, size, body)


def write_patterns():
    (ASSETS / "guilloche-band.svg").write_text(guilloche_band())
    (ASSETS / "rosette.svg").write_text(rosette())


if __name__ == "__main__":
    write_logos()
    pass
    print("ok")
