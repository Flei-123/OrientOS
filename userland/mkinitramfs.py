#!/usr/bin/env python3
"""Baut ein Startdateisystem (Initramfs) im eigenen, bewusst simplen Format.

Aufruf:  mkinitramfs.py <ausgabe> <name>=<datei> [<name>=<datei> ...]

WARUM EIN EIGENES FORMAT (und nicht cpio oder tar)?
  * cpio und tar transportieren POSIX-Metadaten: Modus, uid/gid, mtime,
    Verzeichnisse, Symlinks, Pfade mit '/'. Der Kern dieses Systems kennt
    davon NICHTS: es gibt keine ambient authority, keinen globalen
    Pfadnamensraum und keine Benutzer-IDs. Ein cpio-Parser muesste diese
    Felder lesen und dann wegwerfen — Code, der nur Angriffsflaeche ist.
  * Beide Formate sind stromorientiert: um Eintrag N zu finden, muss man
    N-1 Koepfe entlanghangeln und dabei ASCII-Oktalzahlen parsen. Das ist
    im Kernel mehr Fehlerquelle als Nutzen.
  * Das Format hier ist eine Tabelle fester Breite am Anfang: Anzahl und
    Laenge stehen im Kopf, jeder Eintrag hat Offset und Laenge als u64.
    Der Kernel prueft alle Grenzen mit checked-Arithmetik gegen die
    tatsaechliche Archivlaenge und kommt ohne jede Zeichenkettenanalyse aus.

  * Jeder Eintrag traegt eine CRC32-Pruefsumme ueber SEINE Daten. Ein
    Bootloader-Modul kommt ueber Firmware, Medium und Speicher zum Kernel;
    ein einzelnes gekipptes Byte im Programmtext wuerde sonst erst als
    unerklaerlicher Absturz im unprivilegierten Programm auffallen. Der
    Kernel prueft die Summe, BEVOR er ein Abbild anfasst.

AUFBAU (alles little-endian):
    0x00  8 B   Kennung "IRFS0002"
    0x08  u32   Anzahl Eintraege
    0x0c  u32   Gesamtlaenge des Archivs in Bytes
    0x10  Tabelle, Anzahl * 56 B:
            +0x00  32 B  Name, mit Nullbytes aufgefuellt (kein '/', ASCII)
            +0x20  u64   Offset der Daten ab Archivanfang
            +0x28  u64   Laenge der Daten
            +0x30  u32   CRC32 (IEEE, reflektiert) ueber die Daten
            +0x34  u32   reserviert, 0
    danach: die Daten, jeweils auf 16 B ausgerichtet.
"""

import struct
import sys
import zlib

MAGIC = b"IRFS0002"
NAME_LEN = 32
ENTRY_LEN = 56
HEADER_LEN = 16
ALIGN = 16


def main(argv):
    if len(argv) < 3:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    out = argv[1]
    items = []
    for spec in argv[2:]:
        if "=" not in spec:
            print(f"unbrauchbare Angabe (erwartet name=datei): {spec}", file=sys.stderr)
            return 2
        name, path = spec.split("=", 1)
        raw = name.encode("ascii")
        if not raw or len(raw) >= NAME_LEN or b"/" in raw:
            print(f"unbrauchbarer Name: {name}", file=sys.stderr)
            return 2
        # Doppelte Namen weist auch der Kernel ab (mehrdeutig): dann lieber
        # hier beim Bauen scheitern als spaeter beim Booten.
        if any(raw == vorher for vorher, _ in items):
            print(f"Name doppelt vergeben: {name}", file=sys.stderr)
            return 2
        with open(path, "rb") as f:
            items.append((raw, f.read()))

    data_start = HEADER_LEN + ENTRY_LEN * len(items)
    off = data_start
    blobs = []
    table = []
    for raw, data in items:
        table.append((raw, off, len(data)))
        blobs.append((off, data))
        off += len(data)
        off = (off + ALIGN - 1) & ~(ALIGN - 1)
    total = off

    buf = bytearray(total)
    buf[0:8] = MAGIC
    struct.pack_into("<II", buf, 8, len(items), total)
    for i, (raw, o, ln) in enumerate(table):
        base = HEADER_LEN + i * ENTRY_LEN
        buf[base:base + len(raw)] = raw
        crc = zlib.crc32(items[i][1]) & 0xFFFFFFFF
        struct.pack_into("<QQII", buf, base + NAME_LEN, o, ln, crc, 0)
    for o, data in blobs:
        buf[o:o + len(data)] = data

    with open(out, "wb") as f:
        f.write(buf)
    print(f"   Initramfs: {out} ({len(items)} Eintraege, {total} B)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
