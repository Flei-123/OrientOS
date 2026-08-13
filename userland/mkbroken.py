#!/usr/bin/env python3
"""Erzeugt aus einem gueltigen ELF64 ein absichtlich kaputtes Abbild.

Aufruf:  mkbroken.py <eingabe.elf> <ausgabe.elf>

Das Ergebnis liegt mit im Startdateisystem: der Lader muss es beim ECHTEN
Ladeversuch aus dem Archiv abweisen, nicht nur in einem kuenstlich im Kernel
zusammengebauten Puffer. Veraendert wird genau ein Feld: die Zieladresse des
ersten ladbaren Segments zeigt in den Kernelbereich. Alles andere bleibt
gueltig — damit ist belegt, dass die Bereichspruefung greift und nicht
zufaellig ein anderer Fehler zuschlaegt.
"""

import struct
import sys

PT_LOAD = 1


def main(argv):
    if len(argv) != 3:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    with open(argv[1], "rb") as f:
        buf = bytearray(f.read())
    if buf[:4] != b"\x7fELF":
        print("Eingabe ist kein ELF", file=sys.stderr)
        return 1
    phoff = struct.unpack_from("<Q", buf, 32)[0]
    phentsize = struct.unpack_from("<H", buf, 54)[0]
    phnum = struct.unpack_from("<H", buf, 56)[0]
    for i in range(phnum):
        p = phoff + i * phentsize
        if struct.unpack_from("<I", buf, p)[0] == PT_LOAD:
            struct.pack_into("<Q", buf, p + 16, 0xFFFF_8000_0000_0000)
            break
    else:
        print("kein PT_LOAD gefunden", file=sys.stderr)
        return 1
    with open(argv[2], "wb") as f:
        f.write(buf)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
