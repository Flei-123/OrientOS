# Der Kernelwechsel

**Stand: 25.08.2026.** OrientOS hat seinen Rust-Kernel als Produkt
aufgegeben. Der Kernel kommt jetzt aus dem Osum-Repo — in Firn, mit
eigener Historie, festgenagelt über `vendor/osum/COMMIT`. OrientOS ist
das **System darum herum**.

Dieses Dokument ist kein Ankündigungstext. Es hält fest, **was verglichen
wurde, bevor etwas ersetzt wurde**, was portiert ist, was noch in Rust
steht — und was gemessen wurde.

---

## 1. Warum überhaupt

Es gab zwei Kernel mit demselben Namen.

* **Osum** (eigenes Repo, 83 Commits): 15 495 Zeilen Firn im Kern,
  3 592 Zeilen Firn im Userland, 1 234 Zeilen libc, 1 204 Zeilen
  Assembler. Eigenes Dateisystem, ELF64-Lader von der Platte, NVMe über
  DMA, PCI, APIC, SMP, POSIX-Schicht mit den Systemaufrufnummern von
  Linux, Shell mit 23 Werkzeugen.
* **Der Kernel dieses Repos**: 18 017 Zeilen Rust (`kernel/src`, `libs/`)
  und 922 Zeilen Firn. `LANGUAGE.md` hatte am 21.08.2026 festgehalten:
  *„osum wird Firn-only"*, und die Migration lief modulweise — bei rund
  5 %.

Die Rechnung war einfach. Der Weg von 5 % auf 100 % sind rund 18 000
Zeilen Portierungsarbeit; das Ergebnis wäre ein Kernel, der weniger kann
als der, der daneben schon fertig in Firn dasteht. Also ist die Frage
nicht *ob*, sondern *was dabei verloren geht* — und genau darum geht
Abschnitt 2, und zwar **vor** dem ersten gelöschten Byte.

---

## 2. Der Abgleich, Modul für Modul

Die Tabelle vergleicht **Fähigkeiten**, nicht Zeilenzahlen. Ein Modul,
das in Rust 800 Zeilen braucht und in Firn 500, ist deswegen nicht
schlechter — die Frage ist immer: *wer kann mehr, und was ginge beim
Wechsel verloren?*

Legende: **Osum** = das kann nur Osum · **OrientOS** = das kann nur der
Rust-Kernel · **beide** = gleichwertig vorhanden.

### 2.1 Start und Architektur

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Was beim Wechsel geschähe |
|---|---|---|---|
| `boot/limine.rs` (206) — Limine-Protokoll: Memory-Map, HHDM, Framebuffer, Module, RSDP | `kernel/boot.s` + `mem.fi` — Multiboot 1, Memory-Map, Kommandozeile | **OrientOS** (BIOS + UEFI, Framebuffer, Module) | **BEHOBEN**, siehe 3.1: Osums Multiboot-Kopf verlangt jetzt einen linearen Rahmenpuffer und bootet über Limine auf BIOS **und** UEFI. Framebuffer-Info und Boot-Module bleiben offen. |
| `arch/x86_64/gdt.rs` (172), `idt.rs` (137), `interrupts.rs` (257) | `kernel/boot.s`, `idt.fi` (175), `trap.fi` (210), `isr.s` (434) | beide | — |
| `arch/x86_64/paging.rs` (367) | `kernel/mem.fi` (532), `proc.fi` (686) | **Osum** (getrennte Adressräume je Prozess; OrientOS hat einen) | — |
| `arch/x86_64/context.rs` (132), `preempt.rs` (281) | `kernel/switch.s` (116), `sched.fi` (918) | beide | — |
| `arch/x86_64/cpu.rs`, `msr.rs`, `port.rs`, `timer.rs` | `kernel/cpu.fi`, `hw.fi`, `apic.fi` | **Osum** (APIC/IOAPIC statt nur PIC/PIT) | — |
| `arch/x86_64/serial.rs` (59) | `kernel/serial.fi` (157) | beide | — |
| `arch/x86_64/user.rs` (797) — Ring 3, **SMEP/SMAP in CR4** | `kernel/user.fi` (250), `proc.fi`, `uprog.fi` | **OrientOS** (SMEP/SMAP) / **Osum** (echte Prozesse) | **OFFEN**: Osum setzt SMEP/SMAP nicht. Siehe 4.1. |
| `kcore/arch_iface.rs` (330) — die arch-Grenze als Traits, mit Testschritt, der x86-Begriffe außerhalb `arch/` verbietet | *nichts* | **OrientOS** | **OFFEN**: Osum ist x86-64-only ohne Architekturgrenze. Siehe 4.2. |

### 2.2 Speicher

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Bemerkung |
|---|---|---|---|
| `mm/frame.rs` (786), `mm/firn_bitmap.rs` (192), `kernel/firn/bitmap.fi` (344) | `kernel/mem.fi` (532) | beide | Beide haben einen Bitmap-Rahmenallokator; OrientOS' Fassung ist bereits zur Hälfte in Firn. |
| `mm/heap.rs` (679), `libs/osum-mem/heap.rs` (829) | `kernel/mem.fi` (Halde) | beide | OrientOS' Halde ist auf dem Wirt testbar (`cargo test`), Osums nur in QEMU. |
| `kcore/mem.rs` (461) — Typen `PhysAddr`/`VirtAddr`/`MapFlags` | *implizit* | **OrientOS** (Typsicherheit) | Kosmetik gegenüber dem Rest. |

### 2.3 Prozesse und Zeit

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Bemerkung |
|---|---|---|---|
| `kcore/sched.rs` (1 534) — Threads, Prioritäten, Zeitscheiben, Schlafen/Wecken, Leerlauf-Thread | `kernel/sched.fi` (918), `tasks.fi`, `proc.fi` | **Osum** | OrientOS: Threads in **einem** Adressraum. Osum: Prozesse mit **eigenem** Adressraum, `fork`, `execve`, `wait4`. |
| `kcore/preempt.rs` (248) | `sched.fi` + Zeitgeber | beide | — |
| — | `kernel/smp.fi` (994), `smp.s` (159), `apic.fi`, `acpi.fi` | **Osum** | **Mehrere Prozessoren.** OrientOS hat das nie gehabt. |

### 2.4 Dateien, Geräte, Userland

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Bemerkung |
|---|---|---|---|
| `kcore/initramfs.rs` (428) — eigenes Archivformat, CRC32 je Eintrag, aus einem Bootloader-Modul | `kernel/fs.fi` (935) — OFS mit Inodes, Verzeichnissen, `getdents64`, auf RAM-Platte, ATA oder NVMe | **Osum** | Osum kann Dateien **schreiben**; ein Initramfs ist nur lesbar. **OFFEN (klein)**: Osum nimmt kein Boot-Modul entgegen und hat keine Prüfsummen. Siehe 4.4. |
| `kcore/elf.rs` (741), `kcore/firn_elf.rs` (265), `kernel/firn/elf.fi` (461) | `kernel/elf.fi` (803) | beide | Beide prüfen ELF64 streng; OrientOS' Prüfteil ist schon in Firn und hat 53 Negativfälle. |
| — | `kernel/blk.fi`, `hw.fi`, `nvme.fi` (698), `pci.fi` (591) | **Osum** | **Blockgeräte, PCI, NVMe über DMA.** OrientOS hat keinen einzigen Treiber dafür. |
| `drivers/fbcon.rs` (154) + `drivers/font.rs` (221) — Textkonsole auf dem Framebuffer, eigener 8×16-Font | *nichts* | **OrientOS** | **OFFEN**: Osums Konsole ist die serielle Schnittstelle. Siehe 4.3. |
| — | `kernel/kbd.fi`, `kernel/user/*.fi` (3 592): Shell mit Röhren und Umlenkung, 23 Werkzeuge | **Osum** | OrientOS' Userland ist **ein** Programm in Assembler (`userland/hello.asm`, 64 Zeilen). |

### 2.5 Die ABIs

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Bemerkung |
|---|---|---|---|
| `abi/posix.rs` (188) + `libs/osum-abi-posix` (672) — POSIX als abtrennbare Übersetzungsschicht auf die native ABI | `kernel/sys.fi` (1 877) + `lib/libc/` (1 234) — 26 Systemaufrufe **mit den Nummern von Linux x86-64**, libc in Firn | **Osum** | OrientOS' Schicht ist sauberer geschnitten (per Feature abwählbar), Osums ist ungleich vollständiger und trägt ein echtes Userland. |
| `abi/native.rs` (2 013) + `libs/osum-abi-native` (1 643) — **Capability-Handles**: Slot+Generation, 10 Rechtebits, 8 Objektarten, Kanäle, Ports, Namensräume, `ProcessSpawn` | *nichts* | **OrientOS** | **DAS Kernstück.** Teilweise portiert, siehe 3.2 und 4.5. |

### 2.6 Alles oberhalb des Kernels — bleibt bei OrientOS

| Teil | Rolle nach dem Wechsel |
|---|---|
| `brands/*.toml`, `brand.sh`, `BRANDING.md`, `rename.sh` | **bleibt.** Ein Quellbaum, zwei Produkte — wie in FreeViewer. Der Kernel heißt in jeder Marke `osum`, so wie NT in jeder Windows-Ausgabe NT heißt. |
| `userland/`, `PACKAGING.md` | **bleibt.** Paketformat und unprivilegierte Programme sind Systemsache, nicht Kernelsache. |
| `ARCHITECTURE.md`, `ASSISTENT.md`, `ROADMAP.md` | **bleibt.** Die Architektur des Systems und die Schnittstelle für einen Assistenten. |
| `test.sh`, `tests/step-*.sh` | **bleibt.** Die Abnahme des **Gesamtsystems**. Osum nimmt seinen Kernel selbst ab (dort `./test.sh`, 11 Abschnitte). |
| `build.sh`, `limine.conf`, `vendor/limine/` | **bleibt.** OrientOS verpackt den Kernel zu einem bootfähigen Produkt. |

---

## 3. Was portiert wurde

### 3.1 Der UEFI-Boot-Pfad → Osum-Commit `c4427fa`

Osum bootete nur über BIOS. Die Ursache war **eine Zahl**: im
Multiboot-Kopf stand `MB_FLAGS = 0x3` (ausgerichtete Module +
Speicherkarte). Ein Lader liest daraus, dass der Kern keinen
Bildschirmwunsch hat, und nimmt den **Textmodus** an. Unter UEFI gibt es
keinen Textmodus — Limine bricht ab:

```
PANIC: multiboot1: Cannot use text mode with UEFI.
```

Gemessen, bevor die Änderung entstand. Mit Flag-Bit 2 und `mode_type = 0`
verlangt der Kern einen **linearen Rahmenpuffer**, den die Firmware
setzen kann. Danach bootet **dasselbe Abbild** über SeaBIOS und über
OVMF. Der Kern *benutzt* den Rahmenpuffer nicht; seine Konsole bleibt die
serielle Schnittstelle. Was Bit 2 leistet, ist allein, dass der Lader
nicht mehr auf etwas besteht, das es nicht gibt.

Nachgemessen in `tests/step-24-osum-kernel.sh`: Beendigungscode 21 auf
beiden Wegen, und die Speicherkarten unterscheiden sich (9 Einträge unter
SeaBIOS, 19 unter OVMF) — es waren wirklich zwei verschiedene Starts.

### 3.2 Die Capability-Handles → Osum-Commit `2d89634`

Der inhaltlich wertvollste Teil des Rust-Kernels. Portiert ist der
**Kern des Modells**, aus `libs/osum-abi-native/` (Rust, no_std) nach
`kernel/cap.fi` (Firn, 357 Zeilen) — **mit denselben Zahlen und
denselben Bedeutungen**:

| Aus OrientOS | Nach Osum | Inhalt |
|---|---|---|
| `rights.rs` | `cap.fi` | 10 Rechtebits, `restrict` als Schnittmenge |
| `handle.rs` | `cap.fi` | Handle = Slot + Generation, 8 Objektarten |
| `table.rs` | `cap.fi` | Handle-Tabelle je Prozess, prozesseigener Würfelwert |
| `lib.rs` (`Error`) | `cap.fi` | 9 Fehlerwerte, −1 bis −9 |
| `syscall.rs` | `sys.fi` | die Aufrufe, jetzt ab 2000 (0/1/2 sind bei Osum `read`/`write`/`open` von Linux) |
| `abi/native.rs` | `sys.fi`, `native()` | die Weiche: Handle auflösen, Rechte prüfen, ausführen |

Die drei Zusagen, und jede ist **aus Ring 3** gemessen:

1. **Eine frische Tabelle ist leer.** Es wird nichts geerbt.
2. **Ein geschlossenes Handle trifft nie wieder etwas**, auch nicht nach
   Wiederverwendung des Platzes.
3. **Rechte können nur kleiner werden.**

Der Unterschied zu POSIX in einer Zeile: ein **gültiges** Handle ohne das
nötige Recht ist `RightsDenied` (−2), ein **gefälschtes** ist
`BadHandle` (−1). POSIX hat für beides nur `-EBADF`.

Der Beweis für „nichts wird geerbt" ist der zweite Lauf: der erste macht
seine Tabelle absichtlich voll (16 von 16), der zweite bekommt **denselben
Platz** der Aufgabentabelle und zählt trotzdem genau **ein** Handle.

**Ein Fehler, den diese Portierung gefunden hat.** `proc.map_programs`
benutzte für die Teilung der 2-MiB-Kachel immer dieselbe Seitentabelle.
Das reichte, solange alles, was Ring 3 sehen darf, in eine Kachel passt.
Mit dem neuen Programm wuchs `.utext` unter `firnc1` darüber hinaus — und
die Teilung der zweiten Kachel überschrieb die der ersten: der Kernel zog
sich seine eigene Identitätsabbildung unter den Füßen weg. `map_user_pt`
nimmt die Tabelle jetzt als Argument. A/B nachgemessen: mit künstlich auf
452 KiB aufgeblähtem `.utext` (zwei Kacheln) bootet der Kernel **mit** dem
Fix (21, 113 Seiten abgebildet) und bleibt **ohne** ihn stehen (0).

---

## 4. Was NOCH IN RUST STEHT — die offenen Punkte

Nichts davon ist halb portiert. Halb wäre schlimmer als offen: ein
Kernel, der eine Capability-Schicht *behauptet*, aber nicht durchsetzt,
ist gefährlicher als einer, der ehrlich POSIX macht.

Der Rust-Kernel bleibt deshalb **gebaut und gemessen** (`./build.sh
--kernel rust`, Schritte 2 bis 23 des Testlaufs), bis für jeden Punkt der
Ersatz nachweislich läuft.

### 4.1 SMEP/SMAP

`arch/x86_64/user.rs` setzt beide Bits in CR4, wenn CPUID sie meldet, und
weist ihre Wirkung nach. Osum kennt sie nicht (`grep SMEP kernel/` ist
leer). Osum hat W^X und NX, aber nicht diese beiden.
**Aufwand:** klein — CPUID-Abfrage, zwei Bits, ein `stac`/`clac`-Paar um
die Kopierpfade. **Nicht in dieser Runde gemacht**, weil es die
Kopierpfade von `sys.fi` anfasst, die alle 26 Systemaufrufe tragen.

### 4.2 Die Architekturgrenze

`kcore/arch_iface.rs` beschreibt als Traits, was eine Architektur können
muss; ein Testschritt sucht x86-Begriffe außerhalb von `arch/` und lässt
den Lauf fallen, wenn er welche findet. Osum hat diese Grenze nicht — es
ist x86-64-only, obwohl Firn auch aarch64 kann.
**Aufwand:** groß, und es ist eine Umbauarbeit, keine Portierung.

### 4.3 Rahmenpufferkonsole

`drivers/fbcon.rs` + `drivers/font.rs`: Textkonsole auf dem
Bootloader-Framebuffer mit eigenem 8×16-Font. Osum hat nur die serielle
Schnittstelle. Ironie am Rande: Osum **fordert** seit 3.1 einen
Rahmenpuffer an — er benutzt ihn nur nicht.
**Aufwand:** mittel. Der Multiboot-Kopf des Laders liefert Adresse,
Breite, Höhe und Zeilenlänge bereits mit (Flag-Bit 12); es fehlt der
Font und das Zeichnen.

### 4.4 Boot-Module und Prüfsummen

`kcore/initramfs.rs`: ein Archiv, das der Bootloader als Modul übergibt,
mit CRC32 je Eintrag. Osum liest sein Dateisystem von einer Platte und
kennt kein Boot-Modul. Für ein ISO ohne Platte ist das der Unterschied
zwischen „hat ein Userland" und „hat keins".
**Aufwand:** klein bis mittel.

### 4.5 Der Rest der nativen ABI

Portiert ist das Handle-Modell. **Nicht** portiert sind die Objekte, die
daran hängen:

| Aufruf | Zustand in Osum |
|---|---|
| `ChannelCreate`, `ChannelSend`, `ChannelRecv` | `NotSupported` |
| `PortCreate`, `PortWait`, `PortBind` (der Ersatz für Signale) | `NotSupported` |
| `NamespaceOpen`, `NamespaceCreate` | `NotSupported` |
| `ProcessSpawn` mit expliziter Handle-Liste | `NotSupported` |
| `MemoryCreate`, `MemoryMap`, `MemoryUnmap` | `NotSupported` |

`NotSupported` und nicht `ENOSYS`: der Aufruf **existiert** in dieser
ABI, dieser Kernel bietet ihn nur nicht an. Die Rust-Fassung in
`kernel/src/abi/native.rs` bleibt die Vorlage — sie ist die einzige, die
diese Objekte je hatte.

---

## 5. Wie beides zusammenhängt

```
osum (eigenes Repo, Firn)                orientos (dieses Repo)
├── kernel/*.fi   der Kern       ──┐     ├── vendor/osum/COMMIT   ← festgenagelt
├── kernel/user/  Shell, Werkzeuge │     ├── brands/*.toml        Marken
├── lib/libc/     libc in Firn     └──►  ├── userland/            Programme
├── vendor/firn/COMMIT (eigener          ├── PACKAGING.md         Pakete
│                       Übersetzer)      ├── ASSISTENT.md         Schnittstelle
└── test.sh       11 Abschnitte          ├── build.sh --kernel osum|rust
                                         └── test.sh              Abnahme
```

`vendor/osum/hole-osum.sh` baut den Kernel aus dem festgenagelten Commit
— **mit Osums eigenem Firn-Übersetzer**, nicht mit dem dieses Repos. Die
beiden Repos nageln unterschiedliche Firn-Commits fest, und das soll auch
so bleiben: zwei Projekte, zwei Nägel, keine stille Vermischung.

Eingecheckt ist nur der Hash. Kein Abbild, kein Kopieren ohne Herkunft.

---

## 6. Die Geschichte dieses Wechsels

* **19.–21.08.2026** — `LANGUAGE.md` hält fest: *„osum wird Firn-only"*.
  Die Migration beginnt modulweise: `serial.fi` (117 Zeilen), dann der
  Bitmap-Rahmenverwalter (`bitmap.fi`, 344), dann der ELF-Prüfteil
  (`elf.fi`, 461). Nach drei Runden: 922 Zeilen Firn gegen 17 993 Zeilen
  Rust — rund 5 %.
* **25.08.2026, vormittags** — Osum wird ein eigenes Repository (dort
  Commit `5b920d3`). Damit stehen zwei Kernel mit demselben Namen
  nebeneinander, und der eine ist bereits das, was der andere werden
  will.
* **25.08.2026** — Entscheidung: *Osum ist der Kernel, OrientOS ist das
  Betriebssystem drumherum.*
* **25.08.2026** — dieser Abgleich (Abschnitt 2), **vor** jeder Löschung.
  Ergebnis: zwei Dinge, die wirklich verloren gegangen wären (UEFI,
  Capabilities), plus vier kleinere (SMEP/SMAP, arch-Grenze,
  Rahmenpufferkonsole, Boot-Module).
* **25.08.2026** — UEFI und das Handle-Modell nach Firn portiert
  (Osum `c4427fa` und `2d89634`). Osum: 11 Testabschnitte, darunter 67
  neue Zusagen für die Capability-Schicht und 20 für den Bootkopf.
* **25.08.2026** — OrientOS bindet Osum als `vendor/osum` ein.
  `./build.sh` baut ab jetzt **standardmäßig** das Produkt mit dem
  Osum-Kernel; `--kernel rust` baut die alte Vorlage.

**Was NICHT passiert ist:** kein `rm -rf kernel/src`. Der Rust-Kernel
steht vollständig da und wird bei jedem Testlauf gebaut, gebootet und
gemessen. Er verschwindet Modul für Modul — jedes, wenn sein Ersatz
nachweislich läuft, und keines vorher.
