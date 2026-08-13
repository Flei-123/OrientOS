# Karstos — Betriebssystem von Grund auf

**Kernel: `karst` · OS: `Karstos` · Sprache: Rust (`no_std`) · Ziel: `x86_64-karst-none`**

Kein Linux-Fork, kein übernommener Fremdcode. Der Kernel bootet auf BIOS **und**
UEFI, baut seine eigenen Seitentabellen, hat einen funktionierenden Heap und
meldet alles nachvollziehbar über die serielle Konsole.

> Was hier steht, ist reproduzierbar: `./test.sh` baut, bootet und prüft.
> Was noch fehlt, steht ehrlich in [ARCHITECTURE.md § 10](ARCHITECTURE.md) und
> [ROADMAP.md](ROADMAP.md).

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

# 3. Alles prüfen (Host-Tests + 6 echte QEMU-Boots)
./test.sh
```

### Skripte

| Befehl | Wirkung |
|---|---|
| `./build.sh` | Release-Build + ISO (`build/karstos.iso`) + Symbolkarte (`build/karst.map`) |
| `./build.sh --debug` | Debug-Build |
| `./build.sh --no-posix` | Gegenprobe: Kernel ohne POSIX-Schicht (`--no-default-features`) |
| `./run-qemu.sh` | interaktiver Start, serielle Ausgabe auf stdout |
| `./run-qemu.sh --check` | bootet, prüft 11 Merkmale im Log, **Exitcode** 0/1 (für CI) |
| `./run-qemu.sh --uefi` | Boot über OVMF statt SeaBIOS |
| `./run-qemu.sh --test-pagefault` | löst absichtlich `#PF` aus und prüft die Meldung |
| `./run-qemu.sh --test-doublefault` | löst einen **echten** `#DF` aus (nicht `int 8`) |
| `./run-qemu.sh --test-panic` | prüft Panic-Handler und Backtrace |
| `./test.sh` | alles zusammen, 9 Schritte |
| `cargo test --target x86_64-unknown-linux-gnu -p karst-mem -p karst-abi-native -p karst-abi-posix` | 24 Host-Tests der hardwarefreien Logik |

---

## Was der Kernel wirklich tut

Auszug aus einem echten Boot (`./run-qemu.sh --check`, gekürzt):

```
[karst] boot       karst v0.1.0 — Kernel von Karstos
[karst] boot       Architektur : x86_64 (Basisseite 4096 B)
[karst] boot       ABI         : karst-native + posix (abwaehlbar)
[karst] boot       Bootloader  : Limine 9.6.7
[karst] mm         HHDM-Fenster: virt = phys + 0xffff800000000000
[karst] mm         Kernelabbild: phys 0x000000001fd57000 -> virt 0xffffffff80000000
[karst] mm           .text  ...  (32 KiB, R+X)
[karst] mm           .rodata...  (12 KiB, R)
[karst] mm           .data  ...  (72 KiB, R+W, NX)
[karst] video      Framebuffer : 1280x800 @ 32 bpp -> Textkonsole 160x50
[karst] cpu        GDT + TSS geladen, Datensegmente flach
[karst] cpu        IDT: 256 Tore, Ausnahmen 0..21 belegt
[karst] cpu        #DF laeuft auf eigenem IST-Stapel (0xffffffff80011990)
[karst] cpu        NX aktiv, CR0.WP aktiv
[karst] irq        8259A-PIC (umgeleitet auf 0x20) initialisiert
[karst] trap       #BP Breakpoint bei RIP=0xffffffff800038fe — setze fort
[karst] cpu        Selbsttest: #BP ausgeloest und sauber fortgesetzt
[karst] mm         Memory-Map (16 Eintraege):
[karst] mm           0x000000100000..0x00001fd52000    508 MiB  usable
[karst] mm           0x0000fd000000..0x0000fd3e8000      3 MiB  framebuffer
[karst] mm           0x00fd00000000..0x010000000000     12 GiB  reserved
[karst] mm         Speicher    : 12 GiB gesamt, davon 508 MiB nutzbar
[karst] mm         Frames      : 131039 verwaltet, 130170 frei (508 MiB)
[karst] mm         Frame-Bitmap: 16 KiB bei phys 0x000000100000
[karst] mm         Eigener Adressraum aktiv (CR3 = 0x000000104000)
[karst] mm           HHDM   : 3 GiB als 2026 grosse Seiten
[karst] mm           Abbild : 29 Seiten mit getrennten Rechten
[karst] mm         Selbsttest Paging: map 0xffffff0010000000 -> 0x00000010d000,
                   schreiben/lesen ok, translate ok, unmap ok
[karst] heap       Kernel-Heap : 1024 KiB bei virt 0xffffff0000000000
[karst] heap       Testallokation: Box@0xffffff0000000010 = 0xdeadbeef
[karst] heap       Testallokation: Vec mit 1024 Elementen, Summe 357389824,
                   String "Karstos lebt"
[karst] heap       nach Freigabe: belegt 0 B von 1048576 B
[karst] abi        karst-native: Version=1, Handles=1, altes Handle -> BadHandle
[karst] abi        posix-Schicht aktiv: read(2) auf unbekannten Fd -> -9 (= -EBADF)
[karst] irq        Zeitgeber laeuft mit 100 Hz, Interrupts frei (IF gesetzt)
[karst] irq        Tick 5 — 5 s Laufzeit, 500 Ticks gezaehlt
[karst] boot       Startvorgang abgeschlossen. karst laeuft.
```

### Fehlerfälle sind nachweisbar, nicht behauptet

```
$ ./run-qemu.sh --test-doublefault
[karst] test       loese absichtlich einen Double Fault aus ...
=============== CPU-AUSNAHME ===============
Ausnahme : #DF Double Fault
RIP      : 0xffffffff80005c65   CS : 0x0008
IST-Stapel: 0xffffffff80011990 (eigener Stack, deshalb lebt der Kernel noch)
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
│       │   ├── mem.rs  print.rs  panic.rs
│       ├── arch/x86_64/                ← einzige Stelle mit x86-Details
│       │   ├── gdt.rs idt.rs interrupts.rs paging.rs pic.rs timer.rs
│       │   └── serial.rs cpu.rs msr.rs port.rs selftest.rs
│       ├── mm/                         Frames, Adressräume, Heap
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

| | |
|---|---|
| Quellzeilen (ohne `vendor/`) | ca. 4 950 |
| Externe Crates | **2** — `limine` (Protokollstrukturen), `spin` (Sperren) |
| Geladene Kernelgröße | ca. 113 KiB (`.text` 32 KiB, `.rodata` 10 KiB, `.data` 6 KiB, `.bss` 64 KiB) |
| Host-Tests | 24, alle grün |
| QEMU-Prüfungen | 11 Merkmale je Boot, 6 Boots im `test.sh` |
| Nightly-Features | 3, einzeln begründet (ARCHITECTURE.md § 9) |

---

## Grundregeln

1. **Kein Linux-Code, kein Fork.** Nachbauen aus Spezifikationen ja, übernehmen nein.
2. **x86-Details ausschließlich unter `arch/x86_64/`** — nachprüfbar per `grep`, siehe ARCHITECTURE.md § 2.
3. **POSIX ist abtrennbar** und wird in jedem Testlauf ohne sich selbst gebootet.
4. **Keine Crate ohne Begründung**, keine `todo!()` in Pfaden, die der Boot durchläuft.

Weiter: [ARCHITECTURE.md](ARCHITECTURE.md) · [ROADMAP.md](ROADMAP.md)
