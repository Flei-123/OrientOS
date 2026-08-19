#!/usr/bin/env python3
"""osum-Logo — parametrischer Generator (SVG).

Alle Buchstaben sind reine Geometrie auf einem 100er-Raster (Versalhoehe 100).
Es wird KEINE Schriftdatei fuer OSUM gebraucht, nur fuer den Zusatz "KERNEL".

Zwei Stellschrauben:
  sw   = Strichstaerke (26 war die erste Fassung und ist zu fett; 17 ist ausgewogen)
  kind = Nadeltyp im O:
         mono       beide Haelften cyan (liest sich als Raute, nicht als Nadel)
         mono_fuge  beide Haelften cyan, feine Fuge dazwischen
         duo_soft   cyan / dunkleres cyan  (dezent, empfohlen)
         duo_hard   cyan / schwarz         (harter Kontrast, erste Fassung)

WICHTIG — behobener Fehler: der S-Pfad der ersten Fassung hatte am zweiten
Bogen das falsche sweep-flag (0 statt 1). Dadurch lief der untere Bogen in die
gleiche Richtung wie der obere und das S wurde zu einem ueberlappenden Klecks.
Alle Dateien, die vor diesem Skript entstanden sind, haben dieses kaputte S.
"""
import math

DARK = "#0B0D10"
CY = "#22D3EE"
CY_D = "#12A3BE"


def _sa(fg, sw):
    return (f'fill="none" stroke="{fg}" stroke-width="{sw}" stroke-linecap="butt" '
            f'stroke-linejoin="round" stroke-miterlimit="2"')


def glyphs(sw, fg, mdip=72):
    """(Name, SVG-Element, Breite der Mittellinie). Sichtbare Breite = Mittellinie + sw."""
    r = 50 - sw / 2          # O: Radius der Mittellinie, Aussenkante bleibt bei 50
    rs = (100 - sw) / 4      # S: Radius der beiden Schalen
    ru = 31.5                # U: Radius des Bogens unten
    ty = sw / 2 + rs         # S: Mittelpunkt obere Schale
    by = 100 - sw / 2 - rs   # S: Mittelpunkt untere Schale
    a = _sa(fg, sw)
    return [
        ("O", f'<circle cx="50" cy="50" r="{r:.2f}" {a}/>', 100 - sw),
        ("S", f'<path d="M {2*rs:.2f},{ty:.2f} '
              f'A {rs:.2f},{rs:.2f} 0 1 0 {rs:.2f},50 '
              f'A {rs:.2f},{rs:.2f} 0 1 1 0,{by:.2f}" {a}/>', 2 * rs),
        ("U", f'<path d="M 0,{sw/2:.2f} L 0,{100-sw/2-ru:.2f} '
              f'A {ru},{ru} 0 0 0 63,{100-sw/2-ru:.2f} L 63,{sw/2:.2f}" {a}/>', 63),
        ("M", f'<path d="M 0,{100-sw/2:.2f} L 0,{sw/2:.2f} L 34,{mdip} '
              f'L 68,{sw/2:.2f} L 68,{100-sw/2:.2f}" {a}/>', 68),
    ]


def needle(sw, kind, ang=-40):
    """Doppelnadel im O. Raute mit breitester Stelle exakt in der Mitte,
    dort in zwei Haelften geteilt."""
    ri = 50 - sw                 # Innenkante des Rings
    hl = ri * 0.80               # halbe Nadellaenge
    w = ri * 0.42                # Nadelbreite in der Mitte
    a = math.radians(ang)
    ux, uy = math.cos(a), math.sin(a)
    px, py = -uy, ux
    cx = cy = 50.0
    tip_a = (cx + ux * hl, cy + uy * hl)
    tip_b = (cx - ux * hl, cy - uy * hl)
    o = (1.8 if kind == "mono_fuge" else 0.0) / 2
    q1 = (cx + px * w / 2 + ux * o, cy + py * w / 2 + uy * o)
    q2 = (cx - px * w / 2 + ux * o, cy - py * w / 2 + uy * o)
    p1 = (cx + px * w / 2 - ux * o, cy + py * w / 2 - uy * o)
    p2 = (cx - px * w / 2 - ux * o, cy - py * w / 2 - uy * o)
    c1, c2 = {"mono": (CY, CY), "mono_fuge": (CY, CY),
              "duo_soft": (CY, CY_D), "duo_hard": (CY, DARK)}[kind]
    poly = lambda pts, c: ('<polygon points="'
                           + " ".join(f"{x:.2f},{y:.2f}" for x, y in pts)
                           + f'" fill="{c}"/>')
    return poly([tip_a, q1, q2], c1) + poly([tip_b, p1, p2], c2)


def wortmarke(sw=17, kind="duo_soft", dark=False, pad=14, gap=13):
    fg = "#FFFFFF" if dark else DARK
    nk = "duo_soft" if (dark and kind == "duo_hard") else kind
    parts, x = [], pad
    for name, d, spine in glyphs(sw, fg):
        inner = needle(sw, nk) if name == "O" else ""
        parts.append(f'<g transform="translate({x:.2f},{pad})">{d}{inner}</g>')
        x += spine + sw + gap
    w = x - gap + pad
    h = 100 + 2 * pad
    bg = f'<rect width="{w:.1f}" height="{h}" fill="{DARK}"/>' if dark else ""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.1f} {h}" '
            f'width="{w:.1f}" height="{h}">{bg}{"".join(parts)}</svg>')


def signet(sw=17, kind="duo_soft", dark=False, pad=8):
    fg = "#FFFFFF" if dark else DARK
    s = 100 + 2 * pad
    bg = f'<rect width="{s}" height="{s}" fill="{DARK}"/>' if dark else ""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {s} {s}" width="{s}" height="{s}">{bg}'
            f'<g transform="translate({pad},{pad})">'
            f'<circle cx="50" cy="50" r="{50-sw/2:.2f}" fill="none" stroke="{fg}" stroke-width="{sw}"/>'
            f'{needle(sw, kind)}</g></svg>')


if __name__ == "__main__":
    import sys
    sw = int(sys.argv[1]) if len(sys.argv) > 1 else 17
    kind = sys.argv[2] if len(sys.argv) > 2 else "duo_soft"
    open(f"osum-wortmarke-{sw}-{kind}.svg", "w").write(wortmarke(sw, kind))
    open(f"osum-signet-{sw}-{kind}.svg", "w").write(signet(sw, kind))
    print("geschrieben")
