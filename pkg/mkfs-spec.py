#!/usr/bin/env python3
"""pkg/mkfs-spec.py -- aus einer Paketwurzel die Angaben fuer mkfs.py.

Zwischen der Wurzel, die `opk.py` auf dem Wirt baut, und dem
OFS-Abbild, das ins Produkt kommt, steht genau eine Uebersetzung: aus
Verzeichnissen, Dateien und HARTEN VERWEISEN werden die Angaben, die
`vendor/osum/mkfs.py` versteht.

    <pfad>/                       ein Verzeichnis
    <pfad>=<wirtsdatei>           eine Datei
    <neu>@<vorhanden>             ein ZWEITER NAME derselben Datei

DIE DRITTE FORM IST DER GRUND, WARUM ES DIESES WERKZEUG GIBT. In der
Wurzel ist `apps/explorer.osp/start` derselbe Inode wie
`store/<hash>/start` -- ein harter Verweis, den `opk.py` mit `os.link`
gelegt hat. Wuerde man beide als Datei uebergeben, laege das Programm
ZWEIMAL im Abbild: 234 448 Oktette auf einer Platte von zwei
Megaoktett. Deshalb wird hier nach `st_ino` gruppiert: der erste Name
einer Datei wird geschrieben, jeder weitere ist ein `@`-Verweis auf den
ersten. Dasselbe Verfahren, mit dem Osums K15 `/bin/explorer` und
`/bin/files` verbindet.

Die Reihenfolge ist wichtig und wird hier hergestellt: `mkfs.py` legt
Verzeichnisse und Dateien in der Reihenfolge der Angaben an, und ein
`@`-Verweis kann nur auf etwas zeigen, das schon da ist.

Verwendung:
    mkfs-spec.py <wurzel> [<praefix>]      Angaben auf die Standardausgabe
"""

import os
import sys


def spec(wurzel, praefix="/"):
    wurzel = os.path.abspath(wurzel)
    praefix = "/" + praefix.strip("/")
    if praefix == "/":
        praefix = ""

    verzeichnisse, dateien = [], []
    for pfad, verz, namen in os.walk(wurzel):
        verz.sort()
        rel = os.path.relpath(pfad, wurzel).replace(os.sep, "/")
        ziel = praefix + ("" if rel == "." else "/" + rel)
        if rel != ".":
            verzeichnisse.append((ziel, pfad))
        for n in sorted(namen):
            p = os.path.join(pfad, n)
            dateien.append((ziel + "/" + n, p))

    # Verzeichnisse zuerst, von aussen nach innen -- ein `mkdir` in einem
    # Verzeichnis, das es noch nicht gibt, waere ein Fehler.
    zeilen = []
    for ziel, _ in sorted(verzeichnisse, key=lambda x: x[0].count("/")):
        zeilen.append(ziel + "/")

    # Dann die Dateien, nach Inode gruppiert.
    erster = {}
    for ziel, p in sorted(dateien):
        ino = os.lstat(p).st_ino
        if ino in erster:
            zeilen.append("%s@%s" % (ziel, erster[ino]))
        else:
            erster[ino] = ziel
            zeilen.append("%s=%s" % (ziel, p))
    return zeilen


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    for z in spec(argv[1], argv[2] if len(argv) > 2 else "/"):
        print(z)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
