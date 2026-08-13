#!/usr/bin/env python3
"""Prueft, ob die Quelltext-Verweise in der Doku noch stimmen.

Hintergrund: Zeilennummern in Dokumenten verrutschen bei jeder Aenderung am
Code. Ein Verweis wie `datei.rs:158`, der auf eine voellig andere Stelle zeigt,
ist schlimmer als gar keiner — er sieht ueberpruefbar aus und ist es nicht.

Deshalb gilt im Projekt EIN Format, und dieses Skript setzt es durch:

    `pfad/zur/datei.rs:ZEILE` (`ANKER`)

* ANKER ist ein Stueck Text, das an dieser Stelle wirklich im Quelltext steht.
* Geprueft wird ein Fenster von +/- TOLERANZ Zeilen um ZEILE. So ueberleben
  Verweise kleine Verschiebungen, ohne dass ein falscher Verweis durchrutscht.

Exitcode 0 = alle Verweise stimmen.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# Wie weit darf der Anker von der genannten Zeile entfernt sein?
TOLERANZ = 3

# Automatisch erzeugte Protokolle pruefen wir nicht — sie zitieren fremde
# Befunde und werden nicht von Hand gepflegt.
AUSGENOMMEN = {"GAUNTLET.md"}

# `datei.rs:123` (`anker`)
# Der Anker ist der ERSTE in Backticks gesetzte Text in der Klammer; was
# danach in der Klammer noch steht (weitere Symbole, Erklaerungen), ist frei.
MIT_ANKER = re.compile(r"`([A-Za-z0-9_./-]+\.rs):(\d+)`\s*\(\s*`([^`]+)`")
# Jeder Verweis der Form `datei.rs:123` — auch der ohne Anker, damit keiner
# sich an der Pflicht vorbeischleicht.
JEDER = re.compile(r"`([A-Za-z0-9_./-]+\.rs):(\d+)`")


def main() -> int:
    wurzel = Path(__file__).resolve().parent.parent
    fehler: list[str] = []
    geprueft = 0

    for doc in sorted(wurzel.glob("*.md")):
        if doc.name in AUSGENOMMEN:
            continue
        text = doc.read_text(encoding="utf-8")
        mit_anker = {(m.group(1), m.group(2)) for m in MIT_ANKER.finditer(text)}

        for m in JEDER.finditer(text):
            datei, zeile = m.group(1), m.group(2)
            if (datei, zeile) not in mit_anker:
                fehler.append(
                    f"{doc.name}: Verweis `{datei}:{zeile}` ohne Anker "
                    f"— erwartet: `{datei}:{zeile}` (`etwas aus dem Quelltext`)"
                )

        for m in MIT_ANKER.finditer(text):
            datei, zeile, anker = m.group(1), int(m.group(2)), m.group(3)
            geprueft += 1
            pfad = wurzel / datei
            if not pfad.is_file():
                fehler.append(f"{doc.name}: Datei fehlt: {datei}")
                continue
            zeilen = pfad.read_text(encoding="utf-8").splitlines()
            if zeile < 1 or zeile > len(zeilen):
                fehler.append(
                    f"{doc.name}: {datei}:{zeile} liegt ausserhalb der Datei "
                    f"({len(zeilen)} Zeilen)"
                )
                continue
            von = max(0, zeile - 1 - TOLERANZ)
            bis = min(len(zeilen), zeile + TOLERANZ)
            if not any(anker in z for z in zeilen[von:bis]):
                treffer = [
                    str(i + 1)
                    for i, z in enumerate(zeilen)
                    if anker in z
                ][:3]
                hinweis = (
                    f" — steht stattdessen in Zeile {', '.join(treffer)}"
                    if treffer
                    else " — kommt in der Datei gar nicht vor"
                )
                fehler.append(
                    f"{doc.name}: {datei}:{zeile} nennt Anker '{anker}', "
                    f"der dort nicht steht{hinweis}"
                )

    if fehler:
        print(f"  {len(fehler)} falsche(r) Verweis(e):")
        for f in fehler:
            print(f"    {f}")
        return 1
    print(f"  {geprueft} Quelltext-Verweis(e) in der Doku stimmen (+/-{TOLERANZ} Zeilen)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
