# Architektur von OrientOS

Dieses Dokument beschreibt **den gebauten Zustand**, nicht Absichten.
Alles, was hier steht, ist im Code oder im Testlauf nachprüfbar; wo etwas
noch fehlt, steht es ausdrücklich als „noch nicht".

> **Diese Fassung ist vom 26.08.2026 und beschreibt etwas anderes als die
> vorherige.** Bis dahin standen hier zehn Abschnitte über einen
> Rust-Kernel: Schichten, arch-Traits, Frame-Allocator, Scheduler, Ring 3,
> zwei ABIs. Dieser Kernel ist **gelöscht**
> ([KERNELWECHSEL.md](KERNELWECHSEL.md) § 7). Er ist nicht spurlos weg —
> die alte Fassung dieses Dokuments steht vollständig in der Historie:
>
> ```sh
> git show 91182a0:ARCHITECTURE.md
> ```
>
> Was davon inhaltlich weiterlebt, steht in Abschnitt 6.

---

## 0. Die Teilung: Kernel und System

**OrientOS schreibt seinen Kernel nicht.**

| | Repository | Sprache | Rolle |
|---|---|---|---|
| **Osum** | eigenes Repo, hier eingebunden als `vendor/osum` (festgenagelt über `vendor/osum/COMMIT`) | Firn + Assembler | **Der Kernel und die Programme darauf.** Start, Speicher, Adressräume, Prozesse, Dateisystem, Treiber, Netz, Signale, Bildschirm, die beiden ABIs, Shell und 25 Werkzeuge. |
| **OrientOS** | dieses Repo | Shell, TOML, Python, Doku | **Das System darum herum.** Marken, Zusammenstellung des Userlands, Paketformat, Assistenten-Schnittstelle, das Verpacken zum bootfähigen Produkt und die **Abnahme des Gesamtsystems**. |

Die Regel dazwischen ist dieselbe, die dieses Repo schon für den
Firn-Übersetzer benutzt: **festgenagelter Commit, kein eingechecktes
Abbild, kein Kopieren ohne Herkunft.** `vendor/osum/hole-osum.sh` baut
Kernel **und** Programme aus dem genannten Commit — mit **Osums eigenem**
Firn-Übersetzer, nicht mit dem dieses Repos.

`test.sh` Schritt 1 prüft das nach: der Commit ist ein Hash, das gebaute
Abbild stammt daraus, und im Repo liegt weder ein Kernelabbild noch ein
Programm noch `mkfs.py`.

---

## 1. Was dieses Repo tut, in vier Schritten

```
./build.sh
  │
  │  1. HERKUNFT
  ├─ vendor/osum/hole-osum.sh
  │     git archive <COMMIT> → /tmp/osum-pin-<hash>
  │     dort Osums eigenen Firn-Übersetzer bauen
  │     dort tools/build-kernel.sh  → vendor/osum/osum.mb
  │     dort jedes kernel/user/*.fi → vendor/osum/bin/*
  │
  │  2. ZUSAMMENSTELLUNG
  ├─ userland/PROGRAMME   welche Programme, welche Verzeichnisse,
  │                       welche Dateien aus userland/dateien/
  ├─ vendor/osum/mkfs.py  → build/<slug>-userland.img  (OFS, 512-B-Blöcke)
  ├─ zlib.crc32           → die Summe, die der Kernel nachrechnen wird
  │
  │  3. MARKE
  ├─ brand.sh + brands/<marke>.toml   → OS_NAME, SLUG
  │
  │  4. PRODUKT
  ├─ build/isoroot/boot/osum                     der Kernel
  ├─ build/isoroot/boot/<slug>-userland.img      das Modul
  ├─ build/isoroot/boot/limine/limine.conf       protocol: multiboot1
  │                                              path, module_path,
  │                                              cmdline "… modfs modcrc=…"
  └─ xorriso + limine bios-install → build/<slug>.iso
```

Kein Übersetzer läuft dabei in diesem Repo. Das ist Absicht: alles, was
übersetzt wird, gehört zu Osum, und Osum nagelt dafür seinen eigenen
Firn-Commit fest.

---

## 2. Der Bootweg, und warum er über Multiboot geht

**Limine mit `protocol: multiboot1`**, nicht mit dem Limine-Protokoll.
Der Grund liegt beim Kernel: Osum bootet über Multiboot 1 (`kernel/boot.s`),
und ein Lader muss den Kern so starten, wie der Kern es erwartet.

Der Weg hat zwei Stellen, an denen es früher gehakt hat, und beide sind
gemessen:

**a) UEFI.** Ein Multiboot-Lader unter UEFI muss dem Kern einen Bildschirm
übergeben, bevor er die Firmware verlässt. Steht im Multiboot-Kopf kein
Bildschirmwunsch, nimmt er den **Textmodus** an — und den gibt es unter
UEFI nicht:

```
PANIC: multiboot1: Cannot use text mode with UEFI.
```

Osums Kopf verlangt seit Commit `c4427fa` mit **Flag-Bit 2** und
`mode_type = 0` einen **linearen Rahmenpuffer**. Damit bootet dasselbe
Abbild über SeaBIOS **und** über OVMF. `tests/step-20-produkt.sh` liest
den Kopf aus dem gebauten Abbild und prüft Prüfsumme, Bit 2 und
`mode_type`; `tests/step-30-boot.sh` fährt beide Wege wirklich.

**b) Das Boot-Modul.** Ein ISO hat keine Platte. Was ein Multiboot-Lader
weiterreichen kann, ist ein **Modul** — `module_path:` in `limine.conf`,
und beim Kern kommt es als Eintrag in der Modultabelle an (Multiboot 1,
Informationsstruktur, Flag-Bit 3). Osums `kernel/bootmod.fi` liest sie,
rechnet **CRC32** über das ganze Modul, vergleicht mit `modcrc=` auf der
Kommandozeile und übergibt es an `blk.ram_init`. Für das Dateisystem
ändert sich dabei nichts: dieselben 512 Oktette je Block, nur aus einem
anderen Gerät.

**Der Beweis, dass es zwei echte Starts waren** und nicht zweimal
derselbe: die Speicherkarte kommt von der Firmware, und SeaBIOS und OVMF
liefern verschieden viele Einträge. `tests/step-30-boot.sh` vergleicht die
beiden Zahlen und fällt durch, wenn sie gleich sind.

---

## 3. Die Kommandozeile ist die Schnittstelle

Was der Kernel tut, steht auf seiner Kommandozeile, und sie wird **beim
Bauen** in `limine.conf` geschrieben — nicht beim Starten. Deshalb baut
`run-osum.sh` vorher.

| Wort | Wirkung (Osum) |
|---|---|
| `osum` | starte `/bin/sh` von der Wurzelplatte |
| `modfs` | nimm das Boot-Modul als Wurzelplatte |
| `modcrc=<8 hex>` | die verlangte Prüfsumme des Moduls |
| `script=…` | ein Skript, das durch dieselbe Zeilendisziplin geht wie die Tastatur (`;` ist ein Zeilenumbruch) |
| `caps` | die Capability-Runde in Ring 3 |
| `nosmep`, `nosmap` | Gegenproben: das jeweilige Schutzbit weglassen |
| `smapraw`, `smepraw` | Gegenproben: ein Zugriff, der scheitern **muss** |
| `nokbd`, `nosched`, `noproc`, `nofs`, `noring3` | Stufen des Kernels weglassen |

**Eine Reihenfolge ist nicht beliebig:** `script=` liest bis zum Ende der
Zeile. Ein Kernelwort dahinter wäre ein Shell-Befehl. `build.sh` schiebt
`modfs modcrc=…` deshalb **vor** `script=` — genau das ist einmal
schiefgegangen und stand als `osum$ exit modfs modcrc=…` im Protokoll.

---

## 4. Marken: ein Quellbaum, zwei Produkte

Die einzige Rolle, die sich **nicht** in den Kernel schieben lässt.

* `brands/<marke>.toml` — `os-name`, `slug`, `publisher`, `web`, `feed`.
* `brands/STANDARD` — welche Marke gilt, wenn keine genannt wird. Bis zum
  Kernelwechsel stand das in `kernel/Cargo.toml`; das gibt es nicht mehr.
* `brand.sh` — löst auf und **bricht ab** bei einem unbekannten Namen,
  statt still auf die Standardmarke zurückzufallen.
* Der Kernel heißt in jeder Marke `osum`. Eine Marke, die das anders will,
  setzt `kernel-name` — das benennt dann nur die Datei im Abbild um.

Was der Testlauf davon misst, ist nicht die Existenz der Dateien, sondern
die **Wirkung**: die zweite Marke wird gebaut, es entsteht ein zweites ISO
unter einem anderen Namen, sein Bootmenü trägt den anderen Produktnamen,
und eine erfundene Marke bricht ab.

---

## 5. Die Abnahme

`test.sh` sammelt die Schritte aus `tests/step-*.sh` ein. Jede einzelne
Zusage wird gezählt und einzeln ausgegeben; eine Sammelzahl allein wäre zu
leicht grün zu bekommen.

| Schritt | Was er misst |
|---|---|
| Herkunft | Commit ist ein Hash, das Abbild stammt daraus, im Repo liegt nichts Gebautes |
| Kein Rust | 0 Zeilen Rust außer der Vorlage, kein `Cargo.toml`, die Historie hat alles noch |
| Marken | zwei Produkte, wirklich gebaut; erfundene Marke bricht ab |
| Produkt | ISO enthält genau das gebaute Abbild und genau das gebaute Modul, `limine.conf` nennt die richtige CRC32, Multiboot-Kopf stimmt |
| Boot | BIOS **und** UEFI, verschiedene Speicherkarten |
| Userland | `/bin/sh` aus dem Modul, `ls`/`uname`/`cat`/`wc` in Ring 3, kein Rahmen bleibt liegen, das Modul ist am Ende unverändert |
| Gegenproben | ohne Modul und mit falscher Summe läuft **keine** Shell |
| Capabilities | 18 Zusagen aus Ring 3, einzeln nachgelesen; ohne `caps` meldet niemand etwas |
| Schutzbits | CR4 zurückgelesen; `smapraw` muss einen `#PF` geben, mit `nosmap` nicht |
| ELF-Korpus | 53 kaputte ELF-Köpfe durch den Lader, in **einem** Boot, Kernel überlebt alle |
| Doku | jeder genannte Pfad existiert; was offen ist, steht als offen da |

**Jede Zusage hat eine Gegenprobe.** Eine Eigenschaft ohne Gegenprobe ist
eine Behauptung.

---

## 6. Was vom Rust-Kernel inhaltlich weiterlebt

Vier Dinge, und sie sind der Grund, warum der Wechsel kein Verlust war:

1. **Das Capability-Modell.** Handle = Slot + Generation + prozesseigener
   Würfelwert, 10 Rechtebits, 8 Objektarten, 9 Fehlerwerte — dieselben
   Zahlen, in Firn: Osum `kernel/cap.fi`. Der Unterschied zu POSIX in
   einer Zeile: gültiges Handle ohne Recht → `RightsDenied` (−2),
   gefälschtes → `BadHandle` (−1). POSIX hat für beides nur `-EBADF`.
2. **SMEP und SMAP.** Osum `kernel/guard.fi`, portiert aus
   `arch/x86_64/user.rs`. Beide Bits in CR4, auf jedem Kern, mit dem
   `stac`-Fenster an genau vier Stellen.
3. **Boot-Module mit Prüfsumme.** Osum `kernel/bootmod.fi`, portiert aus
   `kcore/initramfs.rs` — nicht das Archivformat, sondern die Sache: was
   von außen hereinkommt, wird nachgerechnet, bevor es benutzt wird.
4. **Der ELF-Prüfkorpus.** `tests/firn-elf/faelle/` — 53 von Hand gebaute
   Köpfe, jeder mit genau einem Fehler. Der Code, der sie geprüft hat, ist
   weg; die Fälle laufen jetzt gegen Osums Lader, im Produkt-ISO, aus
   Ring 3.

**Was nicht weiterlebt** und warum, steht in
[KERNELWECHSEL.md § 4](KERNELWECHSEL.md): die Architekturgrenze (bleibt
als `vorlage/arch_iface.rs` stehen, nicht gebaut) und die Objekte der
nativen ABI (Kanäle, Ports, Namensräume — `NotSupported`).

---

## 7. Was noch nicht da ist

* **Keine Architekturgrenze im Kernel.** Osum ist x86-64-only, obwohl
  Firn auch aarch64 kann. Die Anforderung steht in
  `vorlage/arch_iface.rs`; der eigentliche Inhalt ist nicht der
  Trait-Text, sondern die Regel: kein x86-Begriff außerhalb von `arch/`,
  und ein Testschritt, der das erzwingt.
* **Kein Paketformat im Produkt.** Das Wurzeldateisystem ist eine feste
  Liste (`userland/PROGRAMME`), kein Store mit Inhalts-Hashes. Der Weg
  dahin steht in [PACKAGING.md](PACKAGING.md) und
  [userland/README.md](userland/README.md).
* **Keine markenabhängigen Userlands.** Zwei Marken unterscheiden sich
  bisher nur im Namen. `userland/PROGRAMME.<marke>` ist der nächste
  Schritt.
* **Kein Schreibpfad auf eine echte Platte.** Das Modul ist eine
  RAM-Platte; Änderungen überleben den Lauf nicht. Osum kann ATA und
  NVMe, aber es gibt keinen Partitionstabellen-Leser und keinen
  Installationsweg.
* **Kein Assistent.** [ASSISTENT.md](ASSISTENT.md) beschreibt die
  Schnittstelle; gebaut ist davon nichts.
* **Getestet wird in QEMU**, nicht auf echter Hardware.
