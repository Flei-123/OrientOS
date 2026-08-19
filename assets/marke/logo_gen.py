#!/usr/bin/env python3
"""osum-Logo — parametrischer Generator.

Alle Buchstaben sind reine Geometrie auf einem 100er-Raster (Versalhoehe 100).
Fuer OSUM wird KEINE Schriftdatei gebraucht, nur fuer den Zusatz "KERNEL".

Festgelegt (Stand Runde 4):
  STRICHSTAERKE = 17   (26 war die erste Fassung und zu fett)
  NADEL         = Doppelnadel, Achse von links oben nach rechts unten (40 Grad),
                  obere Haelfte CY, untere Haelfte CY_DUNKEL (etwas dunkler)

Behobene Fehler gegenueber Runde 1+2:
  * S-Pfad: zweiter Bogen hatte sweep-flag 0 statt 1 -> S war ein Klecks
  * M: Breite 68 statt 76, Mittelknick y=72 statt 63, runde Ecken statt Miter-Spitzen

Bauen:  python3 logo_gen.py          (schreibt SVG + PNG + Favicons hierher)
"""
import math

DARK = "#0B0D10"
WHITE = "#FFFFFF"
CY = "#22D3EE"          # obere Nadelhaelfte
CY_DUNKEL = "#0D8299"   # untere Nadelhaelfte, etwas dunkler

STRICHSTAERKE = 17
NADEL_WINKEL = 40       # Grad, Achse links-oben -> rechts-unten

# Modus -> (Strichfarbe, Nadel oben, Nadel unten, Fugenbreite)
MODI = {
    "hell":        (DARK,  CY,    CY_DUNKEL, 0.0),
    "dunkel":      (WHITE, CY,    CY_DUNKEL, 0.0),
    "mono":        (DARK,  DARK,  DARK,      2.0),
    "mono-invers": (WHITE, WHITE, WHITE,     2.0),
}
DUNKLER_HINTERGRUND = ("dunkel", "mono-invers")


def _sa(fg, sw):
    return (f'fill="none" stroke="{fg}" stroke-width="{sw}" stroke-linecap="butt" '
            f'stroke-linejoin="round" stroke-miterlimit="2"')


def glyphs(sw, fg, mdip=72):
    """(Name, SVG-Element, Breite der Mittellinie). Sichtbare Breite = Mittellinie + sw."""
    r = 50 - sw / 2
    rs = (100 - sw) / 4
    ru = 31.5
    ty = sw / 2 + rs
    by = 100 - sw / 2 - rs
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


def needle(sw, c_oben, c_unten, fuge=0.0, ang=NADEL_WINKEL):
    """Doppelnadel: Raute, breiteste Stelle exakt in der Mitte, dort geteilt.
    fuge > 0 schiebt die Haelften auseinander (fuer einfarbige Varianten noetig,
    sonst liest sich die Nadel als Raute statt als Kompassnadel)."""
    ri = 50 - sw
    hl = ri * 0.80
    w = ri * 0.42
    a = math.radians(ang)
    ux, uy = math.cos(a), math.sin(a)
    px, py = -uy, ux
    cx = cy = 50.0
    o = fuge / 2
    tip_o = (cx - ux * hl, cy - uy * hl)
    tip_u = (cx + ux * hl, cy + uy * hl)
    o1 = (cx + px * w / 2 - ux * o, cy + py * w / 2 - uy * o)
    o2 = (cx - px * w / 2 - ux * o, cy - py * w / 2 - uy * o)
    u1 = (cx + px * w / 2 + ux * o, cy + py * w / 2 + uy * o)
    u2 = (cx - px * w / 2 + ux * o, cy - py * w / 2 + uy * o)
    poly = lambda pts, c: ('<polygon points="'
                           + " ".join(f"{x:.2f},{y:.2f}" for x, y in pts)
                           + f'" fill="{c}"/>')
    return poly([tip_o, o1, o2], c_oben) + poly([tip_u, u1, u2], c_unten)


def wortmarke(modus="hell", sw=STRICHSTAERKE, pad=14, gap=13):
    fg, co, cu, fu = MODI[modus]
    parts, x = [], pad
    for name, d, spine in glyphs(sw, fg):
        inner = needle(sw, co, cu, fu) if name == "O" else ""
        parts.append(f'<g transform="translate({x:.2f},{pad})">{d}{inner}</g>')
        x += spine + sw + gap
    w = x - gap + pad
    h = 100 + 2 * pad
    bg = f'<rect width="{w:.1f}" height="{h}" fill="{DARK}"/>' if modus in DUNKLER_HINTERGRUND else ""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.1f} {h}" '
            f'width="{w:.1f}" height="{h}">{bg}{"".join(parts)}</svg>')


def signet(modus="hell", sw=STRICHSTAERKE, pad=8):
    """Nur das O mit Nadel — die abtrennbare Bildmarke."""
    fg, co, cu, fu = MODI[modus]
    s = 100 + 2 * pad
    bg = f'<rect width="{s}" height="{s}" fill="{DARK}"/>' if modus in DUNKLER_HINTERGRUND else ""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {s} {s}" width="{s}" height="{s}">{bg}'
            f'<g transform="translate({pad},{pad})">'
            f'<circle cx="50" cy="50" r="{50-sw/2:.2f}" fill="none" stroke="{fg}" stroke-width="{sw}"/>'
            f'{needle(sw, co, cu, fu)}</g></svg>')


def _bauen():
    import cairosvg
    from PIL import Image
    import io
    for modus in MODI:
        for name, svg, breite in (("wortmarke", wortmarke(modus), 1600),
                                  ("signet", signet(modus), 1024)):
            open(f"osum-{name}-{modus}.svg", "w").write(svg)
            cairosvg.svg2png(bytestring=svg.encode(),
                             write_to=f"osum-{name}-{modus}.png",
                             output_width=breite)
    # Favicons aus dem hellen Signet
    roh = cairosvg.svg2png(bytestring=signet("hell").encode(), output_width=1024)
    basis = Image.open(io.BytesIO(roh)).convert("RGBA")
    groessen = [16, 32, 48, 64, 128, 256]
    for p in groessen:
        basis.resize((p, p), Image.LANCZOS).save(f"osum-favicon-{p}.png")
    basis.resize((256, 256), Image.LANCZOS).save(
        "osum-favicon.ico", sizes=[(p, p) for p in groessen])
    print("fertig")


if __name__ == "__main__":
    _bauen()
