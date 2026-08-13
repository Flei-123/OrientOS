# PREFLIGHT — Zustand VOR Gauntlet-Runde 2

**Dieses Projekt ist gebaut, kompiliert warnungsfrei und bootet nachweislich.**
Nicht neu anfangen, nichts wegwerfen, keine funktionierende Datei löschen.
Wer den Boot kaputt macht, hat einen Totalausfall produziert, keinen Fortschritt.

## Nachgewiesener Zustand (13.08.2026, vollständiger `./test.sh`-Lauf)

```
 1/15 Host-Tests (129 Tests, 3 Crates)                     bestanden
 2/15 Kernel baut (Release, mit POSIX)                     bestanden
 3/15 Kernel baut OHNE POSIX-Schicht                       bestanden
 4/15 Boot in QEMU (BIOS)                                  bestanden
 5/15 Boot in QEMU ohne POSIX-Schicht                      bestanden
 6/15 Selbsttest Page Fault                                bestanden
 7/15 Selbsttest Double Fault (echter #DF auf IST-Stapel)  bestanden
 8/15 Selbsttest Panic-Handler                             bestanden
 9/15 Selbsttest .rodata schreibgeschuetzt                 bestanden
10/15 Selbsttest NX (Daten ausfuehren)                     bestanden
11/15 Selbsttest #GP (nicht kanonische Adresse)            bestanden
12/15 Selbsttest #UD (ud2)                                 bestanden
13/15 Boot ueber UEFI (OVMF)                               bestanden
14/15 Produktname kommt nur aus branding.rs                bestanden
15/15 Architekturgrenze und Codehygiene                    bestanden
############ ALLE TESTS BESTANDEN ############
```

## Was läuft (nicht neu erfinden — lesen, dann erweitern)

* eigenes Target `x86_64-karst-none.json`, `build.sh`, `run-qemu.sh`, `test.sh`, `rename.sh`
* Limine 9.6.7 vendored (offline), BIOS **und** UEFI, higher-half bei `0xffffffff80000000`
* COM1 + Framebuffer-Textkonsole (eigener 8×16-Font), `println!`/`klog!`,
  Panic-Handler mit Frame-Pointer-Backtrace
* GDT+TSS mit 3 IST-Stapeln, IDT 256 Tore (typisierte Hüllen `set_handler*`),
  Ausnahmen 0–21 im Klartext, **echter** `#DF`
* PIC auf `0x20`, PIT 100 Hz
* Bitmap-Frame-Allocator, **eigene** 4-Ebenen-Seitentabellen + CR3-Umschaltung,
  Rechte je Sektion, NX, CR0.WP
* **eigener** Heap-Allocator (keine Fremd-Crate) mit Wachstum bis 16 MiB
* `arch/x86_64/context.rs` (`naked_asm`) + `kcore/sched.rs`: **kooperativer**
  Scheduler mit sleep/wake, Verklemmungsschutz, Stapelwachzonen
* `abi/native.rs` (Handle-Tabelle mit Generationen), `abi/posix.rs` (Feature-gated)
* 79 Selbsttest-Zusagen im Boot-Log, 25 Prüfmerkmale je Boot

## NEU in Runde 2 (Handarbeit vor diesem Lauf, nicht rückgängig machen)

1. **Branding zentralisiert.** `kernel/src/kcore/branding.rs` ist die EINZIGE
   Stelle mit einem Produktnamen. Gespeist aus `kernel/Cargo.toml`
   (`name`, `[package.metadata.branding] os-name`) über `kernel/build.rs`.
   **Regel: kein Produktname als Literal in `kernel/src`.** Statt `"karst"`
   schreibt man `kcore::branding::KERNEL_NAME`. `test.sh` Schritt 14 erzwingt das.
2. `./rename.sh <kernel> <os>` benennt alles um (verifiziert mit
   `./rename.sh nova Novaos` in `/tmp`, gebootet). Anleitung: `RENAME.md`.
3. **Arch-Leck geschlossen.** `main.rs` rief `arch::x86_64::gdt::double_fault_stack_top()`
   direkt auf. Jetzt: `ArchOps::fault_stack_top() -> Option<VirtAddr>`,
   Fassade `arch::fault_stack_top()`.
4. Neue Entwurfsdokumente, an denen sich die Arbeit ausrichten muss:
   `PACKAGING.md`, `FILESYSTEM.md`, `LANGUAGE.md`, erweiterte `ROADMAP.md`
   (Phase 9 = aarch64/Mobil).
5. Null Compilerwarnungen — in `test.sh` Schritt 15 erzwungen
   (`build/cargo-build.log` wird geprüft).

## Harte Regeln für diesen Lauf

1. **`./test.sh` muss am Ende grün sein.** Vor der Abgabe ausführen und das
   Ergebnis zitieren. Zahl der Schritte darf steigen, nie fallen.
2. Keine x86-Details außerhalb `kernel/src/arch/` — auch nicht in `main.rs`:
   `grep -rnE '\b(cr[0-3]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq)\b' kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'`
   muss leer bleiben.
3. **Kein Produktname als Literal** in `kernel/src` außer `branding.rs`.
4. **Null Compilerwarnungen.** Kein `todo!()`/`unimplemented!()`.
5. Keine neue externe Crate ohne Eintrag in ARCHITECTURE.md § 8. Aktuell 2
   (`limine`, `spin`).
6. **Jede Stelle, an der Rust behindert, wird in `LANGUAGE.md` eingetragen** —
   mit Datei, Zeile, Problem, Workaround, was eine eigene Sprache anders machen
   müsste.
7. Doku nachziehen und Kennzahlen **nachmessen**, nicht raten
   (`size -A`, `wc -l`, Testzahlen aus dem Lauf). Behauptung ohne Code = Fehler.
8. `vendor/limine/` nicht anfassen.

## Toolchain

* rustc/cargo 1.99.0-nightly, `rust-src` vorhanden
* `build-std` steht als Flag in `build.sh`, **nicht** in `.cargo/config.toml`
  (sonst scheitern die Host-Tests an `duplicate lang item: sized`)
* qemu-system-x86_64 7.2, xorriso, nasm, mtools, OVMF unter `/usr/share/OVMF/`
* kein Netz nötig, alle Crates liegen im lokalen Cargo-Cache
