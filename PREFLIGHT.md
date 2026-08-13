# PREFLIGHT — Zustand des Projekts VOR diesem Gauntlet-Lauf

**WICHTIG FÜR ALLE BUILDER: Dieses Projekt ist bereits fertig gebaut, kompiliert
und bootet nachweislich.** Es wird **nicht** neu angefangen. Wer hier etwas
kaputt macht, verliert einen funktionierenden Kernel.

## Nachgewiesener Zustand (Stand 13.08.2026, vollständiger `./test.sh`-Lauf)

```
1/9 Host-Tests (karst-mem, karst-abi-native, karst-abi-posix)  bestanden  (24 Tests)
2/9 Kernel baut (Release, mit POSIX)                           bestanden
3/9 Kernel baut OHNE POSIX-Schicht                             bestanden
4/9 Boot in QEMU (BIOS/SeaBIOS)                                bestanden  (11 Prüfungen)
5/9 Boot in QEMU ohne POSIX-Schicht                            bestanden
6/9 Selbsttest Page Fault                                      bestanden
7/9 Selbsttest Double Fault (echter #DF auf IST-Stapel)        bestanden
8/9 Selbsttest Panic-Handler                                   bestanden
9/9 Boot über UEFI (OVMF)                                      bestanden
############ ALLE TESTS BESTANDEN ############
```

## Was bereits läuft (nicht neu erfinden)

* eigenes Target `x86_64-karst-none.json`, `build.sh`, `run-qemu.sh`, `test.sh`
* Limine 9.6.7 (vendored in `vendor/limine/`, offline), BIOS **und** UEFI
* higher-half Kernel bei `0xffffffff80000000`, Linkerskript mit Sektionssymbolen
* COM1-Serialkonsole, Framebuffer-Textkonsole mit eigenem 8×16-Font
* `println!`, `serial_println!`, `klog!`, Panic-Handler mit Frame-Pointer-Backtrace
* GDT + TSS mit 3 IST-Stapeln, IDT mit 256 Toren, Ausnahmen 0–21 im Klartext
* PIC umgeleitet auf 0x20, PIT 100 Hz, Timer-Ticks im Boot-Log nachweisbar
* Memory-Map-Auswertung, Bitmap-Frame-Allocator (16 KiB Bitmap für 508 MiB RAM)
* **eigene** 4-Ebenen-Seitentabellen + CR3-Umschaltung, Rechte je Sektion, NX, CR0.WP
* **eigener** Heap-Allocator (keine Fremd-Crate), `alloc` funktioniert
* `arch`-Trait-Grenze in `kernel/src/kcore/arch_iface.rs`, keine x86-Lecks im Core
* POSIX per Cargo-Feature abwählbar, Gegenprobe bootet im Testlauf
* README.md, ARCHITECTURE.md, ROADMAP.md geschrieben

## Nur 2 externe Crates

`limine 0.5` (Protokollstrukturen) und `spin 0.9` (Sperren). Jede weitere Crate
braucht eine Begründung in ARCHITECTURE.md — im Zweifel selbst schreiben.

## Harte Regeln für diesen Lauf

1. **`./test.sh` muss am Ende jeder Änderung grün bleiben.** Vor der Abgabe
   ausführen. Ein nicht bootender Kernel ist ein Totalausfall, kein Fortschritt.
2. Keine x86-Details außerhalb `kernel/src/arch/`. Probe:
   `grep -rnE '\b(cr[0-3]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq)\b' kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'`
   muss leer bleiben.
3. Kein `todo!()`/`unimplemented!()` in einem Pfad, den der Boot durchläuft.
4. Keine neue externe Crate ohne Eintrag in ARCHITECTURE.md § 8.
5. Nichts im README/ARCHITECTURE behaupten, was nicht im Code steht.
6. `vendor/limine/` nicht anfassen.

## Toolchain auf dieser Maschine

* rustc/cargo 1.99.0-nightly, `rust-src` vorhanden
* `build-std` steht **nicht** in `.cargo/config.toml`, sondern als Flag in
  `build.sh` — sonst scheitern die Host-Tests an `duplicate lang item: sized`
* qemu-system-x86_64 7.2, xorriso, nasm, mtools, OVMF (`/usr/share/OVMF/`)
* kein Netz zu crates.io nötig; alle Crates liegen im lokalen Cargo-Cache
