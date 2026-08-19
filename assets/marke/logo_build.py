#!/usr/bin/env python3
"""osum-Logo — Varianten aus dem Master erzeugen.

Der Master ist `osum-master.json`: die Konturen von Justins Vorlage, mit
potrace vektorisiert (Bezierkurven, nicht nachgebaut). Drei Ebenen:

  letters   OSUM + KERNEL
  n_hell    helle Facette der Kompassnadel (oben rechts)
  n_dunkel  dunkle Facette der Kompassnadel (unten links)

Alles ist so normiert, dass die VERSALHOEHE von OSUM genau 100 Einheiten ist.

Bauen:  python3 logo_build.py     (schreibt SVG + PNG + Favicons hierher)
"""
import json
import os

HIER = os.path.dirname(os.path.abspath(__file__))
MASTER = os.path.join(HIER, "osum-master.json")

SCHWARZ = "#0B0D10"
WEISS = "#FFFFFF"
NADEL_HELL = "#05EEFA"
NADEL_DUNKEL = "#0A8098"

# Modus -> (Schrift, Nadel hell, Nadel dunkel, Hintergrund oder None)
MODI = {
    "hell":        (SCHWARZ, NADEL_HELL, NADEL_DUNKEL, None),
    "dunkel":      (WEISS,   NADEL_HELL, NADEL_DUNKEL, SCHWARZ),
    "mono":        (SCHWARZ, SCHWARZ,    SCHWARZ,      None),
    "mono-invers": (WEISS,   WEISS,      WEISS,        SCHWARZ),
}

RAND = 8.0   # Einheiten Luft rundum


def _laden():
    d = json.load(open(MASTER))
    return d["meta"], {k: tuple(v) for k, v in d["groups"].items()}


def _svg(meta, gruppen, modus, ausschnitt, mit_wort=True):
    """ausschnitt: 'lock' (OSUM+KERNEL), 'osum' (nur Wortmarke), 'sig' (nur das O)."""
    schrift, n_h, n_d, bg = MODI[modus]
    a = meta[ausschnitt]
    s = meta["scale"]
    b_w = a["w"] * s + 2 * RAND
    b_h = a["h"] * s + 2 * RAND

    # Bei einfarbigen Varianten braucht die Nadel eine Fuge, sonst
    # verschmelzen die beiden Facetten zu einer formlosen Raute.
    fuge = ""
    if modus.startswith("mono"):
        rand = bg or WEISS
        # 1 Einheit = 1/s Master-Einheiten; 16 ergibt rund 2 Einheiten Fuge
        fuge = f' stroke="{rand}" stroke-width="16" stroke-linejoin="round"'

    ebenen = []
    for name, farbe, extra in (("letters", schrift, ""),
                               ("n_dunkel", n_d, fuge),
                               ("n_hell", n_h, "")):
        tr, inhalt = gruppen[name]
        st = "" if "stroke=" in extra else ' stroke="none"'
        ebenen.append(f'<g transform="{tr}" fill="{farbe}"{st}{extra}>{inhalt}</g>')

    hg = f'<rect width="{b_w:.2f}" height="{b_h:.2f}" fill="{bg}"/>' if bg else ""
    innen = (f'<g transform="translate({RAND},{RAND}) scale({s:.8f}) '
             f'translate({-a["x"]},{-a["y"]})">' + "".join(ebenen) + "</g>")
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {b_w:.2f} {b_h:.2f}" '
            f'width="{b_w:.2f}" height="{b_h:.2f}">{hg}{innen}</svg>')


def lockup(modus="hell"):
    """OSUM mit KERNEL darunter — die vollstaendige Marke."""
    meta, g = _laden()
    return _svg(meta, g, modus, "lock")


def wortmarke(modus="hell"):
    """Nur OSUM, ohne die Zeile KERNEL."""
    meta, g = _laden()
    return _svg(meta, g, modus, "osum")


def signet(modus="hell"):
    """Nur das O mit der Kompassnadel — die abtrennbare Bildmarke."""
    meta, g = _laden()
    return _svg(meta, g, modus, "sig")


def _bauen():
    import io
    import cairosvg
    from PIL import Image

    for modus in MODI:
        for name, fn, breite in (("lockup", lockup, 1600),
                                 ("wortmarke", wortmarke, 1600),
                                 ("signet", signet, 1024)):
            svg = fn(modus)
            open(f"osum-{name}-{modus}.svg", "w").write(svg)
            cairosvg.svg2png(bytestring=svg.encode(),
                             write_to=f"osum-{name}-{modus}.png",
                             output_width=breite)

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
