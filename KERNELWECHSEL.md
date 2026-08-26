# Der Kernelwechsel

**Stand: 26.08.2026.** OrientOS hat seinen Rust-Kernel als Produkt
aufgegeben. Der Kernel kommt jetzt aus dem Osum-Repo — in Firn, mit
eigener Historie, festgenagelt über `vendor/osum/COMMIT`. OrientOS ist
das **System darum herum**.

Dieses Dokument ist kein Ankündigungstext. Es hält fest, **was verglichen
wurde, bevor etwas ersetzt wurde**, was portiert ist, was gelöscht wurde
— und was gemessen wurde.

**Die Zeitrechnung in einem Absatz.** Am 25.08.2026 stand der Rust-Kernel
noch vollständig da; die Auflage lautete: *nichts löschen, bevor der
Ersatz nachweislich läuft*. Am 26.08.2026 wurden die letzten zwei offenen
Punkte nach Firn portiert (SMEP/SMAP, Boot-Module) und der Rust-Kernel
gelöscht — **18 255 Zeilen Rust** und **1 206 Zeilen C**, dazu 965 Zeilen
Firn, die zu ihm gehörten. Abschnitt 7 sagt, was genau, und Abschnitt 4,
was **nicht** portiert wurde und warum es trotzdem weg durfte.

**Zu den Zahlen, einmal genau.** Gezählt ist, was `git` kannte
(`git ls-tree -r 586ac70 --name-only | grep '\.rs$'`). Wer im
Arbeitsverzeichnis zählt, findet 18 521 — die Differenz von 266 Zeilen
sind erzeugte Dateien unter `build/`, die nie eingecheckt waren. Beide
Zahlen sind richtig; diese Datei nennt durchgehend die eingecheckte.

---

## 1. Warum überhaupt

Es gab zwei Kernel mit demselben Namen.

* **Osum** (eigenes Repo): 26 088 Zeilen Firn im Kern, 5 114 Zeilen Firn
  im Userland, 1 598 Zeilen libc, 1 336 Zeilen Assembler. Eigenes
  Dateisystem, ELF64-Lader von der Platte, NVMe über DMA, PCI, APIC, SMP,
  TCP/IP, Signale, Terminals, Bildschirm, POSIX-Schicht mit den
  Systemaufrufnummern von Linux, Shell mit 25 Werkzeugen.
* **Der Kernel dieses Repos**: 18 255 Zeilen Rust (`kernel/src` 14 157,
  `libs/` 3 860, `kernel/build.rs` 144, der ELF-Maßstab 94) und 965 Zeilen
  Firn. `LANGUAGE.md` hatte am 21.08.2026 festgehalten:
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

Legende: **Osum** = das kann nur Osum · **OrientOS** = das konnte nur der
Rust-Kernel · **beide** = gleichwertig vorhanden.

Die Spalte ganz rechts ist der **Stand nach dem Schnitt vom 26.08.2026**.

### 2.1 Start und Architektur

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Stand |
|---|---|---|---|
| `boot/limine.rs` (206) — Limine-Protokoll: Memory-Map, HHDM, Framebuffer, Module, RSDP | `kernel/boot.s` + `mem.fi` — Multiboot 1, Memory-Map, Kommandozeile, **Rahmenpuffer, Module** | beide | **ERLEDIGT.** UEFI am 25.08. (Osum `c4427fa`), Boot-Module am 26.08. (Osum `kernel/bootmod.fi`). Gelöscht. |
| `arch/x86_64/gdt.rs` (172), `idt.rs` (137), `interrupts.rs` (257) | `kernel/boot.s`, `idt.fi` (175), `trap.fi` (300), `isr.s` (501) | beide | gelöscht |
| `arch/x86_64/paging.rs` (367) | `kernel/mem.fi` (532), `proc.fi` (777) | **Osum** (getrennte Adressräume je Prozess; OrientOS hatte einen) | gelöscht |
| `arch/x86_64/context.rs` (132), `preempt.rs` (281) | `kernel/switch.s` (116), `sched.fi` (1 021) | beide | gelöscht |
| `arch/x86_64/cpu.rs`, `msr.rs`, `port.rs`, `timer.rs` | `kernel/cpu.fi`, `hw.fi`, `apic.fi` | **Osum** (APIC/IOAPIC statt nur PIC/PIT) | gelöscht |
| `arch/x86_64/serial.rs` (59) | `kernel/serial.fi` (171) | beide | gelöscht |
| `arch/x86_64/user.rs` (797) — Ring 3, **SMEP/SMAP in CR4** | `kernel/user.fi` (268), `proc.fi`, `uprog.fi`, **`guard.fi` (328)** | beide | **ERLEDIGT** am 26.08., siehe 3.3. Gelöscht. |
| `kcore/arch_iface.rs` (330) — die arch-Grenze als Traits, mit Testschritt, der x86-Begriffe außerhalb `arch/` verbietet | *nichts* | **OrientOS** | **NICHT PORTIERT.** Bleibt als `vorlage/arch_iface.rs` (378 Zeilen) stehen, nicht gebaut. Siehe 4.1. |

### 2.2 Speicher

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Stand |
|---|---|---|---|
| `mm/frame.rs` (786), `mm/firn_bitmap.rs` (192), `kernel/firn/bitmap.fi` (344) | `kernel/mem.fi` (532) | beide | gelöscht |
| `mm/heap.rs` (679), `libs/osum-mem/heap.rs` (829) | `kernel/mem.fi` (Halde) | beide | gelöscht. OrientOS' Halde war auf dem Wirt testbar (`cargo test`), Osums nur in QEMU — das ist der einzige echte Verlust dieses Blocks, und er kostet Prüfkomfort, keine Fähigkeit. |
| `kcore/mem.rs` (461) — Typen `PhysAddr`/`VirtAddr`/`MapFlags` | *implizit* | **OrientOS** (Typsicherheit) | gelöscht; die fünf Typen stehen im Kopf von `vorlage/arch_iface.rs` |

### 2.3 Prozesse und Zeit

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Stand |
|---|---|---|---|
| `kcore/sched.rs` (1 534) — Threads, Prioritäten, Zeitscheiben, Schlafen/Wecken, Leerlauf-Thread | `kernel/sched.fi` (1 021), `tasks.fi`, `proc.fi` | **Osum** | gelöscht. OrientOS: Threads in **einem** Adressraum. Osum: Prozesse mit **eigenem** Adressraum, `fork`, `execve`, `wait4`. |
| `kcore/preempt.rs` (248) | `sched.fi` + Zeitgeber | beide | gelöscht |
| — | `kernel/smp.fi` (994), `smp.s` (159), `apic.fi`, `acpi.fi` | **Osum** | **Mehrere Prozessoren.** OrientOS hat das nie gehabt. |
| — | `kernel/signal.fi` (1 134), `tty.fi` (727), `time.fi`, `rand.fi` | **Osum** | **Signale, Terminals, Uhr, Zufall.** OrientOS hatte davon nichts. |

### 2.4 Dateien, Geräte, Userland

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Stand |
|---|---|---|---|
| `kcore/initramfs.rs` (428) — eigenes Archivformat, CRC32 je Eintrag, aus einem Bootloader-Modul | `kernel/fs.fi` (935) — OFS mit Inodes, Verzeichnissen, `getdents64`; **`kernel/bootmod.fi` (251)** — Boot-Modul mit CRC32 | **Osum** | **ERLEDIGT** am 26.08., siehe 3.4. Gelöscht. |
| `kcore/elf.rs` (741), `kcore/firn_elf.rs` (265), `kernel/firn/elf.fi` (461) | `kernel/elf.fi` (803) | beide | gelöscht — **aber die 53 Prüffälle nicht.** Sie laufen jetzt gegen Osums Lader, siehe 7.3. |
| — | `kernel/blk.fi`, `hw.fi`, `nvme.fi` (698), `pci.fi` (591), `virtio.fi` (925), `inet.fi` (1 135) | **Osum** | **Blockgeräte, PCI, NVMe über DMA, Netz.** OrientOS hatte keinen einzigen Treiber. |
| `drivers/fbcon.rs` (154) + `drivers/font.rs` (221) — Textkonsole auf dem Framebuffer, eigener 8×16-Font | `kernel/fb.fi` (1 520), `font.fi` (130), `tty.fi` (727) | **Osum** | **ERLEDIGT** durch Osums Runde K7/K7B (76 geprüfte Zusagen, gemessen an echten Bildschirmfotos). Gelöscht. |
| — | `kernel/kbd.fi`, `kernel/user/*.fi` (5 114): Shell mit Röhren und Umlenkung, 25 Werkzeuge | **Osum** | OrientOS' Userland war **ein** Programm in Assembler (`hello.asm`, 64 Zeilen). Siehe 7.2. |

### 2.5 Die ABIs

| Modul (Rust) | Gegenstück in Osum | Wer kann mehr | Stand |
|---|---|---|---|
| `abi/posix.rs` (188) + `libs/osum-abi-posix` (672) — POSIX als abtrennbare Übersetzungsschicht auf die native ABI | `kernel/sys.fi` (3 252) + `lib/libc/` (1 598) — Systemaufrufe **mit den Nummern von Linux x86-64**, libc in Firn | **Osum** | gelöscht. OrientOS' Schicht war sauberer geschnitten (per Feature abwählbar), Osums ist ungleich vollständiger und trägt ein echtes Userland. |
| `abi/native.rs` (2 013) + `libs/osum-abi-native` (1 643) — **Capability-Handles**: Slot+Generation, 10 Rechtebits, 8 Objektarten, Kanäle, Ports, Namensräume, `ProcessSpawn` | `kernel/cap.fi` (357) + `sys.fi` ab Nummer 2000 | geteilt | Handle-Modell **portiert** (3.2), die Objekte daran **nicht** (4.2). Gelöscht. |

### 2.6 Alles oberhalb des Kernels — bleibt bei OrientOS

| Teil | Rolle nach dem Wechsel |
|---|---|
| `brands/*.toml`, `brand.sh`, `BRANDING.md`, `rename.sh` | **bleibt.** Ein Quellbaum, zwei Produkte — wie in FreeViewer. Der Kernel heißt in jeder Marke `osum`, so wie NT in jeder Windows-Ausgabe NT heißt. |
| `userland/`, `PACKAGING.md` | **bleibt, mit neuer Rolle.** Aus „ein Programm in Assembler" ist die **Zusammenstellung des Produkt-Userlands** geworden (`userland/PROGRAMME`). Siehe 7.2. |
| `ARCHITECTURE.md`, `ASSISTENT.md`, `ROADMAP.md` | **bleibt.** Die Architektur des Systems und die Schnittstelle für einen Assistenten. |
| `test.sh`, `tests/step-*.sh` | **bleibt.** Die Abnahme des **Gesamtsystems**. Osum nimmt seinen Kernel selbst ab (dort `./test.sh`, 15 Abschnitte). |
| `build.sh`, `vendor/limine/` | **bleibt.** OrientOS verpackt Kernel und Userland zu einem bootfähigen Produkt. |

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
OVMF.

Nachgemessen in `tests/step-30-boot.sh`: Beendigungscode 21 auf beiden
Wegen, und die Speicherkarten unterscheiden sich — es waren wirklich zwei
verschiedene Starts.

### 3.2 Die Capability-Handles → Osum-Commit `2d89634`

Der inhaltlich wertvollste Teil des Rust-Kernels. Portiert ist der
**Kern des Modells**, aus `libs/osum-abi-native/` (Rust, no_std) nach
`kernel/cap.fi` (Firn, 357 Zeilen) — **mit denselben Zahlen und
denselben Bedeutungen**: 10 Rechtebits, Handle = Slot + Generation, 8
Objektarten, prozesseigener Würfelwert, 9 Fehlerwerte.

Die drei Zusagen, und jede ist **aus Ring 3** gemessen:

1. **Eine frische Tabelle ist leer.** Es wird nichts geerbt.
2. **Ein geschlossenes Handle trifft nie wieder etwas**, auch nicht nach
   Wiederverwendung des Platzes.
3. **Rechte können nur kleiner werden.**

Der Unterschied zu POSIX in einer Zeile: ein **gültiges** Handle ohne das
nötige Recht ist `RightsDenied` (−2), ein **gefälschtes** ist
`BadHandle` (−1). POSIX hat für beides nur `-EBADF`.

Gemessen in `tests/step-50-schutz.sh`: 18 Zusagen, einzeln nachgelesen,
plus die Gegenprobe, dass ohne das Wort `caps` niemand etwas meldet.

### 3.3 SMEP/SMAP → Osum, `kernel/guard.fi` (26.08.2026)

Der Punkt, der bis zum 26.08. als § 4.1 offen stand. `guard.fi` (328
Zeilen Firn) setzt beide Bits in CR4, sobald CPUID sie meldet — **auf
jedem Kern**, denn CR4 ist pro Prozessor, und `smp.ap_main` zieht sie
nach. Das `stac`/`clac`-Fenster steht an genau vier Stellen (`sys.peek`,
`sys.poke`, `sys.copy_in`, `sys.copy_out`) plus am Signalrahmen
(`signal.poke64`/`peek64`).

Was der Rust-Kernel dabei besser hatte: nichts. Was diese Portierung
zusätzlich hat: **die Gegenproben**, und sie sind der eigentliche Inhalt.

| Kommandozeile | Erwartet | Gemessen |
|---|---|---|
| `-cpu max` | CR4 = `0x300020` | ✔ |
| QEMU-Vorgabe `qemu64` | CR4 = `0x20`, `smep=0 smap=0`, Lauf endet trotzdem mit 21 | ✔ |
| `smapraw` mit SMAP | `#PF err=0x1`, Code **63** | ✔ |
| `smapraw nosmap` | kommt zurück, liest `0x5a`, Code **21** | ✔ |
| `smepraw` mit SMEP | `#PF err=0x11` (Bit 4 = Instruktionsabruf), Code **63** | ✔ |
| `smepraw nosmep` | `smep: came back (!)`, Code **21** | ✔ |
| `-smp 4` | `guard: aps=3` — alle drei weiteren Kerne tragen die Bits, zurückgelesen | ✔ |
| `caps` mit SMAP | `windows=312` — das Fenster wird wirklich benutzt | ✔ |

Der Unterschied zwischen dem Lauf, der stehenbleibt, und dem, der
durchläuft, ist **ein Bit in CR4**.

### 3.4 Boot-Module und Prüfsummen → Osum, `kernel/bootmod.fi` (26.08.2026)

Der Punkt, der bis zum 26.08. als § 4.4 offen stand — und der einzige,
der nicht nur eine Eigenschaft war, sondern **den Unterschied zwischen
einem ISO mit Userland und einem ohne** ausmachte.

`bootmod.fi` (251 Zeilen Firn) liest die Modultabelle des Laders
(Multiboot 1, Flag-Bit 3), rechnet **CRC32** (IEEE, reflektiert,
tabellenlos — dasselbe Polynom wie in `kcore/initramfs.rs`) über das
ganze Modul, vergleicht sie mit `modcrc=` und mountet es als
Wurzelplatte. Für `fs.fi` ändert sich dabei nichts: dieselben 512 Oktette
je Block, nur aus einem anderen Gerät.

Drei Dinge, die dabei gemessen werden und ohne die es eine Behauptung
wäre:

1. **Ohne Vorgabe ist die Summe eine Angabe, mit Vorgabe eine
   Bedingung.** Ein falsches `modcrc=` lässt das Modul liegen; der Kernel
   sagt `ok=0` und läuft weiter.
2. **Ein Modul, das niemand angefordert hat, wird nicht heimlich zur
   Wurzel.** Ohne `modfs` wird es gesehen, gerechnet und nicht benutzt.
3. **Der Bereich gehört dem Modul.** Der Lader legt es in Speicher, den
   die Karte als *nutzbar* führt — `mem.scan` nimmt ihn zurück. Die
   Gegenprobe dazu: die Summe wird am **Ende** des Laufs noch einmal
   gerechnet, nach Prozessen, Seitenabbildungen und Haldenbenutzung.
   `mod: recheck crc=… same=1`.

Das ist der Grund, warum OrientOS' ISO seit dem 26.08.2026 eine Shell
hat.

---

## 4. Was NOCH offen ist

Zwei Punkte. Beide sind **nicht** portiert, beide stehen im Produkt als
fehlend da, und keiner von beiden hat den Rust-Kernel am Leben gehalten.

### 4.1 Die Architekturgrenze — der eine Rest, der stehen bleibt

`kcore/arch_iface.rs` beschrieb als Traits, was eine Architektur können
muss; ein Testschritt suchte x86-Begriffe außerhalb von `arch/` und ließ
den Lauf fallen, wenn er welche fand. Osum hat diese Grenze nicht — es
ist x86-64-only, obwohl Firn auch aarch64 kann.

**Nicht portiert, und der Grund steht in einem Satz:** das wäre keine
Portierung, sondern eine Umbauarbeit an jedem Modul jenes Kernels. Die
x86-Einzelheiten stehen dort über `kmain.fi`, `proc.fi`, `mem.fi`,
`apic.fi`, `smp.fi`, `user.fi` und `guard.fi` verteilt, und eine Grenze
zieht man nicht, indem man Traits danebenlegt.

**Deshalb bleibt genau diese eine Datei stehen** — als
`vorlage/arch_iface.rs`, 378 Zeilen, **nicht gebaut**: es gibt in diesem
Repo kein `cargo` und kein `Cargo.toml` mehr. Sie ist die Anforderung an
die Runde, die diese Grenze in Osum ziehen wird, in der Form, in der sie
schon einmal funktioniert hat. Der eigentliche Inhalt ist nicht der
Trait-Text, sondern die **Regel**: kein x86-Begriff außerhalb von
`arch/`, und ein Testschritt, der das erzwingt.

Der Zustand steht außerdem in Osums README unter „Was ihm fehlt" und in
`ROADMAP.md` als eigener Punkt. Er verschwindet nicht stillschweigend.

### 4.2 Der Rest der nativen ABI

Portiert ist das Handle-Modell (3.2). **Nicht** portiert sind die
Objekte, die daran hängen:

| Aufruf | Zustand in Osum |
|---|---|
| `ChannelCreate`, `ChannelSend`, `ChannelRecv` | `NotSupported` (−9) |
| `PortCreate`, `PortWait`, `PortBind` (der Ersatz für Signale) | `NotSupported` |
| `NamespaceOpen`, `NamespaceCreate` | `NotSupported` |
| `ProcessSpawn` mit expliziter Handle-Liste | `NotSupported` |
| `MemoryCreate`, `MemoryMap`, `MemoryUnmap` | `NotSupported` |

`NotSupported` und nicht `ENOSYS`: der Aufruf **existiert** in dieser
ABI, dieser Kernel bietet ihn nur nicht an. Die **Nummern, die
Fehlerwerte und die Bedeutungen** sind portiert und stehen in
`kernel/sys.fi` ab 2000 — was fehlt, sind die Objekte dahinter.

**Warum die Rust-Fassung trotzdem gelöscht wurde.** Sie wäre eine Vorlage
von 3 656 Zeilen gewesen, für Objekte, die in Firn ohnehin anders
aussehen werden (Stufe 0 hat keine globalen veränderlichen Variablen; ein
Kanal wird ein Bereich in der Datenregion, kein `Vec<Message>`). Eine
Vorlage, die man Zeile für Zeile umschreiben muss, ist eine
Spezifikation — und die steht kürzer und genauer in Osums `sys.fi`, wo
jeder dieser Aufrufe seine Nummer und seinen `NotSupported`-Zweig schon
hat. In der Historie steht die Rust-Fassung vollständig:
`git show 91182a0:kernel/src/abi/native.rs`.

### 4.3 Was noch fehlt, aber nie am Rust-Kernel hing

Zur Vollständigkeit, damit die Liste nicht besser aussieht, als sie ist:
Osum hat kein Fenstersystem, keine Namensauflösung, kein USB, kein
SATA/AHCI, keinen Partitionstabellen-Leser, kein Journal, keine
Benutzer/Rechte, keine dynamische Bindung. Der Rust-Kernel hatte davon
ebenfalls nichts. Das steht in Osums README und in `ROADMAP.md`.

---

## 5. Wie beides zusammenhängt

```
osum (eigenes Repo, Firn)                orientos (dieses Repo)
├── kernel/*.fi   der Kern       ──┐     ├── vendor/osum/COMMIT   ← festgenagelt
├── kernel/user/  Shell, 25 Werkz. │     ├── brands/*.toml        Marken
├── lib/libc/     libc in Firn     └──►  ├── userland/PROGRAMME   was ins ISO kommt
├── vendor/firn/COMMIT (eigener          ├── userland/dateien/    was OrientOS beisteuert
│                       Übersetzer)      ├── PACKAGING.md         Pakete
└── test.sh       15 Abschnitte          ├── ASSISTENT.md         Schnittstelle
                                         ├── build.sh             Kernel + Userland → ISO
                                         └── test.sh              Abnahme des Systems
```

`vendor/osum/hole-osum.sh` baut aus dem festgenagelten Commit **beides**:
den Kernel und die unprivilegierten Programme — **mit Osums eigenem
Firn-Übersetzer**, nicht mit dem dieses Repos. Die beiden Repos nageln
unterschiedliche Firn-Commits fest, und das soll auch so bleiben: zwei
Projekte, zwei Nägel, keine stille Vermischung.

Eingecheckt ist nur der Hash. Kein Abbild, kein Programm, kein Kopieren
ohne Herkunft.

---

## 6. Die Geschichte dieses Wechsels

* **19.–21.08.2026** — `LANGUAGE.md` hält fest: *„osum wird Firn-only"*.
  Die Migration beginnt modulweise: `serial.fi` (117 Zeilen), dann der
  Bitmap-Rahmenverwalter (`bitmap.fi`, 344), dann der ELF-Prüfteil
  (`elf.fi`, 461). Nach drei Runden: 922 Zeilen Firn gegen 17 993 Zeilen
  Rust — rund 5 %.
* **25.08.2026, vormittags** — Osum wird ein eigenes Repository. Damit
  stehen zwei Kernel mit demselben Namen nebeneinander, und der eine ist
  bereits das, was der andere werden will.
* **25.08.2026** — Entscheidung: *Osum ist der Kernel, OrientOS ist das
  Betriebssystem drumherum.*
* **25.08.2026** — dieser Abgleich (Abschnitt 2), **vor** jeder Löschung.
  Ergebnis: zwei Dinge, die wirklich verloren gegangen wären (UEFI,
  Capabilities), plus vier kleinere (SMEP/SMAP, arch-Grenze,
  Rahmenpufferkonsole, Boot-Module).
* **25.08.2026** — UEFI und das Handle-Modell nach Firn portiert
  (Osum `c4427fa` und `2d89634`). **Nichts gelöscht**: der Rust-Kernel
  steht vollständig da und wird bei jedem Testlauf gebaut, gebootet und
  gemessen.
* **25.08.2026, abends** — Osums Runde K7/K7B liefert den Bildschirm:
  Rahmenpuffer, Textkonsole, eigener 8×16-Zeichensatz, `/dev/fb`, 76
  Zusagen an echten Bildschirmfotos gemessen. Damit ist der dritte der
  vier kleineren Punkte erledigt, ohne dass jemand ihn portiert hätte —
  er wurde in Osum ohnehin gebraucht.
* **26.08.2026** — die letzten zwei Punkte portiert: SMEP/SMAP
  (`kernel/guard.fi`) und Boot-Module mit CRC32 (`kernel/bootmod.fi`),
  zusammen 579 Zeilen Firn, abgenommen mit 55 Zusagen in Osums
  `tools/guard/run.sh` (dort Abschnitt 15 von `./test.sh`).
* **26.08.2026** — **der Schnitt.** 18 255 Zeilen Rust, 1 206 Zeilen C und 965 Zeilen Firn
  gelöscht, eine Datei bleibt als Vorlage stehen. Abschnitt 7.
* **26.08.2026** — und der Gewinn, der nicht in der Löschliste steht: das
  Produkt-ISO hat zum ersten Mal ein **Userland**. `build.sh` stellt aus
  Osums Programmen und `userland/PROGRAMME` ein OFS-Dateisystem zusammen,
  legt es als Boot-Modul neben den Kern, und `/bin/sh` läuft daraus in
  Ring 3 — mit SMEP und SMAP an.

---

## 7. Der Schnitt vom 26.08.2026

**Die Auflage lautete: nichts löschen, bevor der Ersatz nachweislich
läuft.** Sie ist eingehalten worden — der Ersatz ist ein Repo mit 15
Testabschnitten und über 1 100 Zusagen, und die vier offenen Punkte sind
abgearbeitet, bevor der erste `git rm` lief: zwei portiert (3.3, 3.4),
einer war ohnehin erledigt (Bildschirm, K7), einer bleibt begründet
stehen (4.1).

### 7.1 Was gelöscht wurde

| Weg | Zeilen | Ersatz |
|---|---:|---|
| `kernel/src/` — der ganze Rust-Kernel | 14 157 | Osum `kernel/*.fi`, 26 088 Zeilen Firn |
| `libs/osum-abi-native/`, `libs/osum-abi-posix/`, `libs/osum-mem/` | 3 860 | Osum `cap.fi`, `sys.fi`, `mem.fi` |
| `kernel/build.rs`, `tests/firn-elf/rust-massstab.rs` | 238 | nichts — es wird hier kein Rust mehr übersetzt |
| `tests/firn-elf/*.c`, `tests/firn-bitmap/`, `tests/firn-fehler/` | 1 206 C | Osums eigene Prüfstände; die **53 ELF-Fälle bleiben** (7.3) |
| `kernel/firn/{serial,bitmap,elf}.fi` und `tests/firn-fehler/*.fi` | 965 Firn | Osum `serial.fi`, `mem.fi`, `elf.fi` |
| `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `kernel/Cargo.toml`, `kernel/build.rs`, `kernel/linker-x86_64.ld`, `x86_64-osum-none.json` | — | kein cargo mehr im Baum |
| `run-qemu.sh`, `limine.conf` | — | `run-osum.sh`; `limine.conf` erzeugt `build.sh` |
| `userland/hello.asm`, `user.ld`, `mkinitramfs.py`, `mkbroken.py` | — | Osums 25 Werkzeuge im Boot-Modul (7.2) |
| `tests/step-16`…`step-23`, `tests/verweise.py` | — | neue Schritte, die das **Produkt** messen |

**Alles mit `git rm`.** Die Historie hat jede Zeile davon; `test.sh`
prüft das nach (`git log -- kernel/src` muss Commits liefern). Ein
gelöschter Kernel ist nicht vergessen, er ist nur nicht mehr im Weg.

Was **nicht** gelöscht wurde, obwohl es C ist: `vendor/limine/*.c` — das
ist der Bootloader, nicht der Kernel, und er wird weiter gebraucht.

### 7.2 Was aus `userland/` geworden ist

Vorher: **ein** Programm, `hello.asm`, 64 Zeilen Assembler, in einem
eigenen Archivformat, das nur der Rust-Kernel lesen konnte.

Nachher: `userland/PROGRAMME` — die **Zusammenstellung des
Produkt-Userlands**. Osum schreibt und baut die Programme, OrientOS
entscheidet, welche ins Produkt kommen und was sonst noch auf der Wurzel
liegt (`userland/dateien/`). Daraus wird ein OFS-Dateisystem, das als
Boot-Modul im ISO liegt.

Der Nachweis, dass das keine Umbenennung ist, steht in
`tests/step-40-userland.sh`: `/bin/sh` startet **aus dem Modul**, `ls
/bin` zeigt die Werkzeuge, `cat /etc/ausgabe.txt` liest eine Datei, die
aus **diesem** Repo kommt, und `wc -l` zählt darin dieselbe Zeilenzahl
wie der Wirt.

Was daraus werden soll, steht in `userland/README.md` und `PACKAGING.md`:
markenabhängige Listen, dann ein Paketindex mit Inhalts-Hashes, dann ein
Schreibpfad auf eine echte Platte.

### 7.3 Was aus dem ELF-Korpus geworden ist

`tests/firn-elf/faelle/` — 53 von Hand gebaute ELF64-Köpfe, jeder mit
genau **einem** Fehler — ist das einzige Stück des Rust-Kernels, das
nicht gelöscht wurde und trotzdem noch etwas tut. Der Code, der sie
geprüft hat, ist weg. Die Fälle sind besser als der Code, der sie
hervorgebracht hat.

Sie liegen jetzt im **Produkt-ISO** unter `/f/`, und die Shell versucht in
**einem** Boot jedes davon zu starten
(`tests/step-60-elf-korpus.sh`). Gemessen wird nicht „alle werden
abgelehnt" — Osums Lader hat andere Grenzen für `USER_BASE` und
`USER_LIMIT` als der Rust-Kernel, und eine Zahl, die man nicht
nachgerechnet hat, gehört nicht in eine Zusage. Gemessen wird: der Kernel
**überlebt alle 53**, die Ablehnungen tragen Fehlerwerte statt Abstürze,
der Lader nennt mehr als einen Grund, und die Shell arbeitet danach
weiter.

### 7.4 Die Zahlen nach dem Schnitt

| | vorher (25.08.) | nachher (26.08.) |
|---|---:|---:|
| Rust in `orientos` (eingecheckt) | 18 255 | **0** (+ 378 Zeilen Vorlage, nicht gebaut) |
| C in `orientos` (ohne `vendor/limine`) | 1 206 | **0** |
| Firn in `orientos` | 965 | **0** — Firn steht jetzt vollständig in Osum |
| Firn in `osum` (Kern) | 25 509 | **26 088** |
| Programme im Produkt-ISO | 1 (`hello`) | **26** |
| Testabschnitte `orientos` | 19 | 8 |
| Zusagen `orientos` | 166 | **151**, alle mit Gegenprobe |
| Testabschnitte `osum` | 14 | **15** |

Die Abschnittszahl von OrientOS sinkt, und das ist richtig so: dreizehn
davon haben einen Kernel gemessen, der in diesem Repo nicht mehr
geschrieben wird. Sie sind nicht verschwunden — sie stehen in Osums
`./test.sh`, und dort sind es mehr geworden.
