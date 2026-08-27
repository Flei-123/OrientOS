#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Erzeugt den Massstab fuer den ELF64-Pruefteil.

WARUM ES DEN GIBT

`kernel/src/kcore/elf.rs` hatte **null** `#[test]`. Was es gab, war ein
Selbsttest im Kernel mit 17 Faellen, sichtbar im Boot-Log. Das ist besser als
nichts, aber es ist kein Massstab, an dem sich eine Neufassung messen laesst:
die Faelle stehen als Rust-Code IN der zu ersetzenden Datei.

Deshalb liegen die Faelle jetzt als **Dateien** vor. Beide Fassungen -- die
alte in Rust und die neue in Firn -- lesen dieselben Bytes und muessen
denselben Fehlercode liefern. Ein Unterschied ist dann ein Unterschied im
Parser, nicht im Testaufbau.

WAS ABGEDECKT IST

* gueltige Abbilder (eins, zwei Segmente, Grenzfaelle der Ausrichtung)
* abgeschnittene Dateien -- an jeder interessanten Stelle, nicht nur bei 0
* jedes Feld des Kopfes einzeln verfaelscht
* luegende Offsets und Groessen, auch solche, die erst beim Addieren
  ueberlaufen (das ist der Fall, den eine ungeprüfte Sprache still falsch macht)
* ueberlappende Segmente, Segmente in derselben Seite
* Segmente ausserhalb des unprivilegierten Bereichs, auch per Ueberlauf
* Programmkopftabelle jenseits der Datei, auch per Ueberlauf

Aufruf: python3 tests/firn-elf/faelle.py [zielverzeichnis]
Erzeugt dort je Fall eine `.elf` und eine `erwartet.txt` mit
`<datei> <fehlercode> <beschreibung>` je Zeile.
"""

import struct
import sys
from pathlib import Path

# --- die Fehlercodes. Muessen mit ElfError in beiden Fassungen uebereinstimmen.
OK = 0
TOO_SHORT = 1
BAD_MAGIC = 2
BAD_CLASS = 3
BAD_ENDIAN = 4
BAD_TYPE = 5
BAD_MACHINE = 6
BAD_TABLE = 7
SEG_OUT_OF_FILE = 8
SEG_OUT_OF_RANGE = 9
SEG_OVERLAP = 10
SEGS_SHARE_PAGE = 11
SEG_INSANE = 12
BAD_ALIGN = 13
NEEDS_INTERP = 14
BAD_ENTRY = 15
TOO_MANY_SEGMENTS = 16

EHDR_LEN = 64
PHDR_LEN = 56
ET_EXEC = 2
EM_X86_64 = 62
PT_LOAD = 1
PT_INTERP = 3
PT_NOTE = 4
PAGE = 0x1000
USER_BASE = 0x0000_0000_0040_0000
USER_LIMIT = 0x0000_8000_0000_0000
MAX_SEGMENTS = 16
U64MAX = 0xFFFF_FFFF_FFFF_FFFF

faelle = []


def phdr(ptype=PT_LOAD, flags=5, off=0, vaddr=0x401000, filesz=128, memsz=0x1000,
         align=0, paddr=None):
    """Ein Programmkopf, 56 Oktette."""
    if paddr is None:
        paddr = vaddr
    return struct.pack("<IIQQQQQQ", ptype, flags, off, vaddr, paddr,
                       filesz, memsz, align)


def ehdr(entry=0x401000, phoff=EHDR_LEN, phnum=1, phentsize=PHDR_LEN,
         etype=ET_EXEC, machine=EM_X86_64, klasse=2, endian=1, magic=b"\x7fELF"):
    """Ein ELF64-Kopf, 64 Oktette."""
    b = bytearray(EHDR_LEN)
    b[0:4] = magic
    b[4] = klasse
    b[5] = endian
    b[6] = 1  # Version
    struct.pack_into("<H", b, 16, etype & 0xFFFF)
    struct.pack_into("<H", b, 18, machine & 0xFFFF)
    struct.pack_into("<I", b, 20, 1)  # e_version
    struct.pack_into("<Q", b, 24, entry & U64MAX)
    struct.pack_into("<Q", b, 32, phoff & U64MAX)
    struct.pack_into("<Q", b, 40, 0)  # e_shoff
    struct.pack_into("<I", b, 48, 0)  # e_flags
    struct.pack_into("<H", b, 52, EHDR_LEN)
    struct.pack_into("<H", b, 54, phentsize & 0xFFFF)
    struct.pack_into("<H", b, 56, phnum & 0xFFFF)
    return bytes(b)


def datei(kopf, *koepfe, gesamt=256):
    """Kopf + Programmkoepfe, mit Nullen auf `gesamt` aufgefuellt."""
    b = bytearray(kopf)
    for k in koepfe:
        b += k
    if len(b) < gesamt:
        b += bytes(gesamt - len(b))
    return bytes(b)


def fall(name, daten, erwartet, beschreibung):
    faelle.append((name, daten, erwartet, beschreibung))


# ===================================================================== gueltig

fall("gueltig-ein-segment",
     datei(ehdr(), phdr()),
     OK, "ein ladbares Segment, R+X, Einsprung darin")

# Zwei Segmente in getrennten Seiten -- der Normalfall eines echten Programms.
fall("gueltig-zwei-segmente",
     datei(ehdr(phnum=2),
           phdr(off=0, vaddr=0x401000, filesz=128, memsz=0x1000, flags=5),
           phdr(off=0x40, vaddr=0x403000, filesz=16, memsz=0x1000, flags=6)),
     OK, "zwei Segmente, RX und RW, verschiedene Seiten")

fall("gueltig-align-passt",
     datei(ehdr(entry=0x401008),
           phdr(off=8, vaddr=0x401008, filesz=64, memsz=0x1000, align=0x1000)),
     OK, "Ausrichtung 4096, Datei- und Speicherlage stimmen modulo ueberein")

fall("gueltig-align-eins",
     datei(ehdr(), phdr(align=1)),
     OK, "Ausrichtung 1 stellt keine Bedingung")

# Ein PT_NOTE dazwischen muss uebersprungen werden, nicht abgewiesen.
fall("gueltig-fremder-kopftyp",
     datei(ehdr(phnum=2),
           phdr(ptype=PT_NOTE, vaddr=0, memsz=0),
           phdr()),
     OK, "unbekannter Programmkopftyp wird uebersprungen")

# Segment genau am oberen Rand des erlaubten Bereichs.
fall("gueltig-am-oberen-rand",
     datei(ehdr(entry=USER_LIMIT - 0x1000),
           phdr(vaddr=USER_LIMIT - 0x1000, filesz=128, memsz=0x1000)),
     OK, "Segment endet genau auf USER_LIMIT")

# Segment genau an der unteren Grenze.
fall("gueltig-am-unteren-rand",
     datei(ehdr(entry=USER_BASE),
           phdr(vaddr=USER_BASE, filesz=128, memsz=0x1000)),
     OK, "Segment beginnt genau auf USER_BASE")

# ============================================================ zu kurze Dateien

fall("leer", b"", TOO_SHORT, "Datei ohne ein einziges Oktett")
fall("kurz-1", b"\x7f", TOO_SHORT, "ein Oktett")
fall("kurz-magie", b"\x7fELF", TOO_SHORT, "nur die Kennung")
fall("kurz-63", datei(ehdr())[:63], TOO_SHORT, "ein Oktett zu kurz fuer den Kopf")
# Genau 64 Oktette: der Kopf ist da, die Tabelle nicht.
fall("kurz-nur-kopf", ehdr(), BAD_TABLE, "Kopf vollstaendig, Tabelle fehlt")
# Kopf plus halber Programmkopf.
fall("kurz-halber-phdr",
     datei(ehdr(), phdr(), gesamt=EHDR_LEN + PHDR_LEN - 1)[:EHDR_LEN + PHDR_LEN - 1],
     BAD_TABLE, "Programmkopf um ein Oktett abgeschnitten")

# ================================================================ Kopf kaputt

fall("magie-falsch", datei(ehdr(magic=b"\x7fEXF"), phdr()),
     BAD_MAGIC, "drittes Oktett der Kennung falsch")
fall("magie-null", datei(ehdr(magic=b"\x00\x00\x00\x00"), phdr()),
     BAD_MAGIC, "Kennung komplett null")
fall("klasse-32bit", datei(ehdr(klasse=1), phdr()),
     BAD_CLASS, "32-Bit-Klasse")
fall("klasse-null", datei(ehdr(klasse=0), phdr()),
     BAD_CLASS, "Klasse 0")
fall("endian-gross", datei(ehdr(endian=2), phdr()),
     BAD_ENDIAN, "big endian")
fall("typ-rel", datei(ehdr(etype=1), phdr()),
     BAD_TYPE, "Objektdatei statt ausfuehrbar")
fall("typ-dyn", datei(ehdr(etype=3), phdr()),
     BAD_TYPE, "dynamisch gebundenes Abbild (ET_DYN)")
fall("maschine-arm", datei(ehdr(machine=40), phdr()),
     BAD_MACHINE, "ARM statt x86-64")
fall("maschine-aarch64", datei(ehdr(machine=183), phdr()),
     BAD_MACHINE, "aarch64 statt x86-64")

# =========================================================== Tabelle luegt

fall("phentsize-falsch", datei(ehdr(phentsize=32), phdr()),
     BAD_TABLE, "Groesse eines Programmkopfs stimmt nicht")
fall("phentsize-null", datei(ehdr(phentsize=0), phdr()),
     BAD_TABLE, "Groesse eines Programmkopfs ist 0")
fall("phnum-null", datei(ehdr(phnum=0), phdr()),
     BAD_TABLE, "kein einziger Programmkopf")
fall("phoff-jenseits", datei(ehdr(phoff=0xFFFF_0000), phdr()),
     BAD_TABLE, "Tabelle beginnt weit hinter dem Dateiende")
fall("phoff-knapp-jenseits", datei(ehdr(phoff=256 - PHDR_LEN + 1), phdr()),
     BAD_TABLE, "Tabelle ragt um ein Oktett ueber das Dateiende")
# Der Fall, den eine ungeprüfte Sprache still falsch macht: phoff + tabellenlaenge
# laeuft ueber und ergibt eine kleine Zahl.
fall("phoff-ueberlauf", datei(ehdr(phoff=U64MAX - 8), phdr()),
     BAD_TABLE, "Tabellenbeginn so gross, dass beginn+laenge ueberlaeuft")
# phnum so gross, dass phnum * 56 ueberlaeuft (bei 64-Bit-Rechnung nicht, aber
# die Tabelle liegt dann weit jenseits der Datei).
fall("phnum-riesig", datei(ehdr(phnum=0xFFFF), phdr()),
     BAD_TABLE, "65535 Programmkoepfe, Tabelle passt nicht in die Datei")

# ======================================================= Segment luegt

fall("filesz-groesser-memsz",
     datei(ehdr(), phdr(filesz=0x9000, memsz=0x1000)),
     SEG_INSANE, "Dateilaenge groesser als Speicherlaenge")
fall("memsz-null",
     datei(ehdr(), phdr(filesz=0, memsz=0)),
     SEG_INSANE, "Segment belegt keinen Speicher")
fall("filesz-jenseits",
     datei(ehdr(), phdr(off=200, filesz=200, memsz=0x1000)),
     SEG_OUT_OF_FILE, "Segmentdaten ragen ueber das Dateiende")
fall("fileoff-jenseits",
     datei(ehdr(), phdr(off=0x10000, filesz=16, memsz=0x1000)),
     SEG_OUT_OF_FILE, "Segmentbeginn liegt hinter dem Dateiende")
# off + filesz laeuft ueber -- ohne Pruefung ergaebe das eine kleine Zahl und
# die Bereichspruefung ginge durch.
fall("fileoff-ueberlauf",
     datei(ehdr(), phdr(off=U64MAX - 8, filesz=64, memsz=0x1000)),
     SEG_OUT_OF_FILE, "Dateibeginn+laenge laeuft ueber")

fall("vaddr-im-kernel",
     datei(ehdr(entry=0xFFFF_8000_0000_0000),
           phdr(vaddr=0xFFFF_8000_0000_0000, memsz=0x1000)),
     SEG_OUT_OF_RANGE, "Segment im privilegierten Bereich")
fall("vaddr-unter-user-base",
     datei(ehdr(entry=0x1000), phdr(vaddr=0x1000, memsz=0x1000)),
     SEG_OUT_OF_RANGE, "Segment unterhalb von USER_BASE")
fall("vaddr-null",
     datei(ehdr(entry=0), phdr(vaddr=0, memsz=0x1000)),
     SEG_OUT_OF_RANGE, "Segment auf Adresse 0")
fall("vend-ueber-limit",
     datei(ehdr(entry=USER_LIMIT - 0x800),
           phdr(vaddr=USER_LIMIT - 0x800, filesz=64, memsz=0x1000)),
     SEG_OUT_OF_RANGE, "Segment ragt ueber USER_LIMIT hinaus")
# vaddr + memsz laeuft ueber.
fall("vaddr-ueberlauf",
     datei(ehdr(entry=0x401000),
           phdr(vaddr=U64MAX - 0x100, filesz=0, memsz=0x1000)),
     SEG_OUT_OF_RANGE, "Zieladresse+laenge laeuft ueber")

# ================================================== Ausrichtung unstimmig

fall("align-keine-zweierpotenz",
     datei(ehdr(), phdr(align=3)),
     BAD_ALIGN, "Ausrichtung 3 ist keine Zweierpotenz")
fall("align-krumm-gross",
     datei(ehdr(), phdr(align=1000)),
     BAD_ALIGN, "Ausrichtung 1000 ist keine Zweierpotenz")
fall("align-lage-passt-nicht",
     datei(ehdr(), phdr(off=8, vaddr=0x401000, align=0x200)),
     BAD_ALIGN, "Datei- und Speicherlage stimmen modulo Ausrichtung nicht")

# ================================================== Binder verlangt

fall("interp-allein",
     datei(ehdr(), phdr(ptype=PT_INTERP, vaddr=0, memsz=0)),
     NEEDS_INTERP, "einziger Programmkopf verlangt einen Binder")
fall("interp-zweiter",
     datei(ehdr(phnum=2), phdr(), phdr(ptype=PT_INTERP, vaddr=0, memsz=0)),
     NEEDS_INTERP, "zweiter Programmkopf verlangt einen Binder")

# ================================================== Einsprung

fall("entry-ausserhalb",
     datei(ehdr(entry=0), phdr()),
     BAD_ENTRY, "Einsprung liegt in keinem Segment")
fall("entry-knapp-dahinter",
     datei(ehdr(entry=0x401000 + 0x1000), phdr(vaddr=0x401000, memsz=0x1000)),
     BAD_ENTRY, "Einsprung ein Oktett hinter dem Segment")
fall("entry-in-nicht-ausfuehrbarem",
     datei(ehdr(entry=0x401000), phdr(flags=6, vaddr=0x401000, memsz=0x1000)),
     BAD_ENTRY, "Einsprung liegt in einem nicht ausfuehrbaren Segment")

# ================================================== mehrere Segmente

fall("segmente-ueberlappen",
     datei(ehdr(phnum=2),
           phdr(vaddr=0x401000, filesz=128, memsz=0x1000, flags=5),
           phdr(off=0x40, vaddr=0x401000, filesz=16, memsz=0x1000, flags=6)),
     SEG_OVERLAP, "zwei Segmente auf derselben Adresse")
fall("segmente-ueberlappen-teilweise",
     datei(ehdr(phnum=2),
           phdr(vaddr=0x401000, filesz=128, memsz=0x2000, flags=5),
           phdr(off=0x40, vaddr=0x401800, filesz=16, memsz=0x1000, flags=6)),
     SEG_OVERLAP, "zweites Segment ragt in das erste hinein")
fall("segmente-teilen-seite",
     datei(ehdr(phnum=2),
           phdr(vaddr=0x401000, filesz=128, memsz=0x100, flags=5),
           phdr(off=0x40, vaddr=0x401800, filesz=16, memsz=0x16, flags=6)),
     SEGS_SHARE_PAGE, "zwei Segmente ohne Ueberlappung, aber in derselben Seite")

# 17 ladbare Segmente -- eines mehr, als der Lader annimmt.
viele = [ehdr(phnum=MAX_SEGMENTS + 1, entry=0x401000)]
for i in range(MAX_SEGMENTS + 1):
    viele.append(phdr(off=0, vaddr=0x401000 + i * 0x2000, filesz=0, memsz=0x1000,
                      flags=5))
fall("zu-viele-segmente",
     datei(viele[0], *viele[1:], gesamt=EHDR_LEN + (MAX_SEGMENTS + 1) * PHDR_LEN),
     TOO_MANY_SEGMENTS, "17 ladbare Segmente bei einer Grenze von 16")

# Genau 16 muessen noch gehen.
genau = [ehdr(phnum=MAX_SEGMENTS, entry=0x401000)]
for i in range(MAX_SEGMENTS):
    genau.append(phdr(off=0, vaddr=0x401000 + i * 0x2000, filesz=0, memsz=0x1000,
                      flags=5))
fall("genau-16-segmente",
     datei(genau[0], *genau[1:], gesamt=EHDR_LEN + MAX_SEGMENTS * PHDR_LEN),
     OK, "genau 16 ladbare Segmente sind erlaubt")

# Nur nicht-ladbare Programmkoepfe: es bleibt kein Segment uebrig.
fall("kein-ladbares-segment",
     datei(ehdr(phnum=2),
           phdr(ptype=PT_NOTE, vaddr=0, memsz=0),
           phdr(ptype=7, vaddr=0, memsz=0)),
     BAD_TABLE, "kein einziger ladbarer Programmkopf")


def main():
    ziel = Path(sys.argv[1] if len(sys.argv) > 1 else
                Path(__file__).parent / "faelle")
    ziel.mkdir(parents=True, exist_ok=True)
    for alt in ziel.glob("*.elf"):
        alt.unlink()
    zeilen = []
    for name, daten, erwartet, beschreibung in faelle:
        (ziel / f"{name}.elf").write_bytes(daten)
        zeilen.append(f"{name}.elf {erwartet} {beschreibung}")
    (ziel / "erwartet.txt").write_text("\n".join(zeilen) + "\n", encoding="utf-8")
    gut = sum(1 for f in faelle if f[2] == OK)
    print(f"{len(faelle)} Faelle in {ziel} ({gut} gueltig, {len(faelle) - gut} abzuweisen)")


if __name__ == "__main__":
    main()
