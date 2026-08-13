# Karstos — Betriebssystem von Grund auf

**Kernel: `karst` · OS: `Karstos` · Sprache: Rust (`no_std`) · Ziel: `x86_64-karst-none`**

Kein Linux-Fork, kein übernommener Fremdcode. Der Kernel bootet auf BIOS **und**
UEFI, baut seine eigenen Seitentabellen, hat einen funktionierenden Heap, prüft
sich beim Start mit 79 eingebauten Zusagen selbst und wechselt bereits zwischen
Kernel-Threads.

> Alles hier ist reproduzierbar: `./test.sh` baut, bootet und prüft in 15
> Schritten. Was noch fehlt, steht ehrlich in
> [ARCHITECTURE.md § 10](ARCHITECTURE.md) und [ROADMAP.md](ROADMAP.md).

---

## Schnellstart

```sh
# 1. Voraussetzungen (Debian/Ubuntu)
rustup toolchain install nightly && rustup default nightly
rustup component add rust-src llvm-tools
sudo apt install qemu-system-x86 xorriso mtools ovmf

# 2. Bauen und in QEMU starten
./build.sh          # Kernel + bootfähiges build/karstos.iso
./run-qemu.sh       # startet, serielle Ausgabe im Terminal (Strg-C beendet)

# 3. Alles prüfen (Host-Tests + 10 echte QEMU-Boots)
./test.sh
```

### Skripte

| Befehl | Wirkung |
|---|---|
| `./build.sh` | Release-Build + ISO (`build/karstos.iso`) + Symbolkarte (`build/karst.map`) |
| `./build.sh --debug` | Debug-Build |
| `./build.sh --no-posix` | Gegenprobe: Kernel ohne POSIX-Schicht (`--no-default-features`) |
| `./run-qemu.sh` | interaktiver Start, serielle Ausgabe auf stdout |
| `./run-qemu.sh --check` | bootet, prüft **25 Merkmale** im Log, **Exitcode** 0/1 (für CI) |
| `./run-qemu.sh --uefi` | Boot über OVMF statt SeaBIOS |
| `./run-qemu.sh --test-pagefault` | löst absichtlich `#PF` aus und prüft die Meldung |
| `./run-qemu.sh --test-doublefault` | löst einen **echten** `#DF` aus (nicht `int 8`) |
| `./run-qemu.sh --test-panic` | prüft Panic-Handler und Backtrace |
| `./run-qemu.sh --test-rodata` | Schreibversuch auf `.rodata` muss `#PF` auslösen |
| `./run-qemu.sh --test-nx` | Sprung in Daten muss `#PF` mit Instruktionsabruf auslösen |
| `./run-qemu.sh --test-gp` | nicht kanonische Adresse muss `#GP` geben, nicht `#PF` |
| `./run-qemu.sh --test-ud` | `ud2` muss als `#UD` gemeldet werden |
| `./test.sh` | alles zusammen, 15 Schritte |
| `./rename.sh <kernel> <os>` | benennt Kernel und OS im ganzen Baum um (siehe [RENAME.md](RENAME.md)) |
| `cargo test --target x86_64-unknown-linux-gnu -p karst-mem -p karst-abi-native -p karst-abi-posix` | **129 Host-Tests** der hardwarefreien Logik |

---

## Was der Kernel wirklich tut

Echter Boot, aufgezeichnet in `build/boot.log` (gekürzt — Memory-Map-Tabelle
und Wiederholungen entfernt, sonst wörtlich):

```
[karst] boot       karst v0.1.0 — Kernel von Karstos
[karst] boot       Architektur : x86_64 (Basisseite 4096 B)
[karst] boot       ABI         : karst-native + posix (abwaehlbar)
[karst] boot       Konfiguration: Bauart release, POSIX ja, Fehler-Selbsttest keine
[karst] boot       Bootloader  : Limine 9.6.7
[karst] mm         HHDM-Fenster: virt = phys + 0xffff800000000000
[karst] mm         Kernelabbild: phys 0x000000001fcdf000 -> virt 0xffffffff80000000
[karst] mm           .text  0xffffffff80000000..0xffffffff80013000  (76 KiB, R+X)
[karst] mm           .rodata0xffffffff80013000..0xffffffff80018000  (20 KiB, R)
[karst] mm           .data  0xffffffff80018000..0xffffffff8002a000  (72 KiB, R+W, NX)
[karst] mm           gesamt 168 KiB geladenes Abbild (42 Seiten a 4096 B)
[karst] video      Framebuffer : 1280x800 @ 32 bpp -> Textkonsole 160x50
[karst] cpu        GDT + TSS geladen, Datensegmente flach
[karst] cpu        IDT: 256 Tore, Ausnahmen 0..21 belegt
[karst] cpu        #DF laeuft auf eigenem IST-Stapel (0xffffffff8001ea20)
[karst] cpu        NX aktiv, CR0.WP aktiv
[karst] irq        8259A-PIC (umgeleitet auf 0x20) initialisiert, alle Leitungen maskiert
[karst] trap       #BP Breakpoint bei RIP=0xffffffff80006a1c — setze fort
[karst] cpu        Selbsttest: #BP ausgeloest und sauber fortgesetzt
[karst] mm         Memory-Map (16 Eintraege):
[karst] mm           0x000000100000..0x00001fcda000    507 MiB  usable
[karst] mm           0x0000fd000000..0x0000fd3e8000      3 MiB  framebuffer
[karst] mm           0x00fd00000000..0x010000000000     12 GiB  reserved
[karst] mm         Memory-Map  : 16 Regionen, 77 Frames unter 1 MiB gesperrt
[karst] mm         Speicher    : 12 GiB gesamt, davon 508 MiB nutzbar
[karst] mm         Frames      : 131039 verwaltet, 130032 frei (507 MiB)
[karst] mm         Frame-Bitmap: 16 KiB bei phys 0x000000100000
[karst] mm         Vor Umschaltung geprueft: 5/5 Pflichtbereiche im neuen Adressraum erreichbar
[karst] mm         Abbild nachgeprueft: 42 Seiten virt->phys korrekt, HHDM-Raender ok
[karst] mm         Eigener Adressraum aktiv (CR3 = 0x000000104000)
[karst] mm           HHDM   : 3 GiB als 2026 grosse Seiten
[karst] mm         Selbsttest Paging: map/schreiben/lesen ok, translate ok, unmap ok
[karst] mm         Selbsttest MapError: 23/23 Fehlerfaelle nachweisbar ausgeloest
                   (Misaligned 10, AlreadyMapped 3, NotMapped 4, OutOfFrames 4, ParentHugePage 2)
[karst] mm         Selbsttest Grenzfaelle: 13/13 Zusagen erfuellt
[karst] mm         Selbsttest Memory-Map: 6/6 Zusagen (4 kaputte Karten abgewiesen)
[karst] mm         Selbsttest Frames: 26/26 Zusagen erfuellt, 130013 frei
[karst] mm           Frame-Buchhaltung: 1026 belegt + 130013 frei = 131039, stimmig: ja
[karst] heap       Kernel-Heap : 1024 KiB bei virt 0xffffff0000000000 (eigener Free-List-Allocator)
[karst] heap       Testallokation: Box@0xffffff0000000010 = 0xdeadbeef
[karst] heap       Testallokation: Vec mit 1024 Elementen, Summe 357389824, String "Karstos lebt"
[karst] heap       belegt 8272 B von 1048576 B
[karst] heap       nach Freigabe: belegt 0 B von 1048576 B
[karst] heap         Heap-Wachstum: 2166784 B abgebildet, 1 Erweiterung(en), Grenze 16777216 B
[karst] heap       Selbsttest Heap: 13/13 Zusagen erfuellt, 0 B belegt, 1 Loecher
[karst] abi        karst-native: Version=1, Handles=1, altes Handle -> BadHandle
[karst] abi        posix-Schicht aktiv: read(2) auf unbekannten Fd -> -9 (erwartet -9 = -EBADF)
[karst] irq        Zeitgeber laeuft mit 100 Hz, Interrupts frei (IF gesetzt)
[karst] irq        Tick 5 — 5 s Laufzeit, 500 Ticks gezaehlt
[karst] sched      Thread A (Kennung 1): Durchlauf 3/3
[karst] sched      Thread B (Kennung 2): Durchlauf 3/3
[karst] sched      Threadwechsel: 16 Wechsel, zurueck in kmain (2 Threads a 16 KiB Stapel,
                   Spur 121212, 5/5 Zusagen)
[karst] sched      Schlafen/Wecken: 3 Ticks geschlafen, Wartender geweckt: ja, 7/7 Zusagen
[karst] sched      Stapelnutzung: hoechstens 560 von 16384 B je Thread, Wachzonen unversehrt
[karst] sched      Verklemmungsschutz: run() kehrte nach 2 Wechseln zurueck, 3/3 Zusagen
[karst] boot       Selbsttestbilanz: 79/79 bestanden
                   (#BP 1/1, Paging 3/3, MapError 36/36, Frames 26/26, Heap 13/13)
[karst] boot       Startbilanz : 129482 Frames frei (1557 belegt), Heap 0 / 2166784 B, 508 Ticks
[karst] boot       Startvorgang abgeschlossen. karst laeuft.
```

### Fehlerfälle sind nachweisbar, nicht behauptet

```
$ ./run-qemu.sh --test-doublefault
[karst] test       loese absichtlich einen Double Fault aus ...
=============== CPU-AUSNAHME ===============
Ausnahme : #DF Double Fault
IST-Stapel: 0xffffffff8001ea20 (eigener Stack, deshalb lebt der Kernel noch)
Kein Weiterlaufen moeglich — CPU wird angehalten.
```

```
$ ./run-qemu.sh --test-pagefault
Ausnahme : #PF Page Fault
Adresse  : 0xffffff4000000000 (CR2)
Ursache  : Seite nicht praesent, Schreibzugriff, ausgeloest im Kernel
=============== KERNEL PANIC ===============
Backtrace (Frame-Pointer, Adressen via addr2line aufloesbar):
  #0  ip=0xffffffff8000610c
  #1  ip=0xffffffff80000f20
```

Backtrace-Adressen auflösen:

```sh
llvm-addr2line -e target/x86_64-karst-none/release/karst -f -C 0xffffffff8000610c
# oder in build/karst.map nachschlagen
```

---

## Verzeichnisse

```
karstos/
├── build.sh  run-qemu.sh  test.sh      Bauen, Starten, Prüfen
├── x86_64-karst-none.json              eigenes Target
├── limine.conf                         Bootmenü
├── kernel/
│   ├── linker-x86_64.ld                higher-half bei -2 GiB
│   └── src/
│       ├── main.rs                     kmain — der ganze Bootweg an einer Stelle
│       ├── kcore/                      Typen und Traits, architekturneutral
│       │   ├── arch_iface.rs           ← die arch-Grenze
│       │   ├── sched.rs                Scheduler-Trait, Threads
│       │   └── mem.rs  print.rs  panic.rs
│       ├── arch/x86_64/                ← einzige Stelle mit x86-Details
│       │   ├── gdt.rs idt.rs interrupts.rs paging.rs pic.rs timer.rs
│       │   ├── context.rs              Kontextwechsel (naked_asm)
│       │   └── serial.rs cpu.rs msr.rs port.rs selftest.rs
│       ├── mm/                         Frames, Adressräume, Heap, Selbsttests
│       ├── drivers/                    Framebuffer-Konsole + eigener 8×16-Font
│       ├── boot/limine.rs              einzige Datei, die das Bootprotokoll kennt
│       └── abi/                        native.rs + posix.rs (Feature-gated)
├── libs/
│   ├── karst-mem/                      Bitmap-Allocator, Heap, Regionen — host-getestet
│   ├── karst-abi-native/               capability-basierte ABI
│   └── karst-abi-posix/                abtrennbare POSIX-Übersetzung
└── vendor/limine/                      Bootloader-Binaries (offline nutzbar)
```

---

## Kennzahlen

Alle Zahlen sind gemessen, nicht geschätzt — die Befehle stehen daneben.

| | | gemessen mit |
|---|---|---|
| Rust-Zeilen (`kernel/` + `libs/`) | 10 668 | `find kernel/src libs -name '*.rs' -exec cat {} + \| wc -l` |
| Externe Crates | **2** (`limine`, `spin`; `bitflags` transitiv) | `Cargo.lock` |
| Geladene Kernelgröße | **163,8 KiB** | `size -A target/x86_64-karst-none/release/karst` |
| Host-Tests | **129**, alle grün | `cargo test --target x86_64-unknown-linux-gnu -p ...` |
| Prüfmerkmale je QEMU-Boot | **25** | `run-qemu.sh --check` |
| Selbsttest-Zusagen im Kernel | **79** | Boot-Log, Zeile „Selbsttestbilanz" |
| Schritte in `./test.sh` | **15** (davon 10 echte QEMU-Boots) | `./test.sh` |
| Compilerwarnungen | **0** (erzwungen in `test.sh` Schritt 14) | `build/cargo-build.log` |
| Nightly-Features | 4, einzeln begründet | ARCHITECTURE.md § 9 |
| Produktname im Quelltext | **0 Stellen** ausser `branding.rs` | `./test.sh` Schritt 14 |

---

## Grundregeln

1. **Kein Linux-Code, kein Fork.** Nachbauen aus Spezifikationen ja, übernehmen nein.
2. **x86-Details ausschließlich unter `arch/x86_64/`** — nachgeprüft in `test.sh` Schritt 14.
3. **POSIX ist abtrennbar** und wird in jedem Testlauf ohne sich selbst gebootet.
4. **Keine Crate ohne Begründung**, keine `todo!()` in Pfaden, die der Boot durchläuft.
5. **Keine Behauptung ohne Code dahinter.** Kennzahlen werden nachgemessen, wenn sie sich ändern.

## Entwurfsdokumente

| Datei | Inhalt |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Schichten, arch-Grenze, POSIX-Abtrennung, Abhängigkeiten |
| [ROADMAP.md](ROADMAP.md) | Phasen 1–9, inklusive aarch64/Mobil als eigenes Ökosystem |
| [PACKAGING.md](PACKAGING.md) | unveränderliche content-addressed Apps, drei Datentöpfe, Systemgenerationen |
| [FILESYSTEM.md](FILESYSTEM.md) | VFS → FAT32 → ext4 (lesend) → eigenes Dateisystem |
| [LANGUAGE.md](LANGUAGE.md) | Logbuch: wo Rust im Kernel im Weg steht |
| [RENAME.md](RENAME.md) | Kernel und OS in unter 15 Minuten umbenennen |
