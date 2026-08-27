# PREFLIGHT — state of the project (state after round 3)

> **SUPERSEDED ON 2026-08-26.** This document describes work on the
> **Rust kernel** of this repository. That kernel is deleted — the kernel
> comes from the Osum repository ([KERNELWECHSEL.md](KERNELWECHSEL.md)).
> Everything here that names `run-qemu.sh`, `cargo`, `kernel/src` or
> `Cargo.toml` no longer exists; where things are now is stated in
> [tests/GELOESCHT.md](tests/GELOESCHT.md). The document stays as a
> **record**: what was planned and measured back then is part of the
> history of this project and is not rewritten after the fact.


**This project is built, compiles without warnings and demonstrably boots.**
Do not start over, do not throw anything away, do not delete a working file.
Whoever breaks the boot has produced a total failure, not progress.

## Demonstrated state (2026-08-13, complete `./test.sh` run)

```
 1/21 Host-Tests (151 Tests, 3 Crates)                     bestanden
 2/21 Kernel baut (Release, mit POSIX, --fresh)            bestanden
 3/21 Kernel baut OHNE POSIX-Schicht                       bestanden
 4/21 Boot in QEMU (BIOS)                                  bestanden
 5/21 Boot in QEMU ohne POSIX-Schicht                      bestanden
 6/21 Selbsttest Page Fault                                bestanden
 7/21 Selbsttest Double Fault (echter #DF auf IST-Stapel)  bestanden
 8/21 Selbsttest Panic-Handler                             bestanden
 9/21 Selbsttest .rodata schreibgeschuetzt                 bestanden
10/21 Selbsttest NX (Daten ausfuehren)                     bestanden
11/21 Selbsttest #GP (nicht kanonische Adresse)            bestanden
12/21 Selbsttest #UD (ud2)                                 bestanden
13/21 Boot ueber UEFI (OVMF)                               bestanden
14/21 Produktname kommt nur aus branding.rs                bestanden
15/21 Architekturgrenze und Codehygiene                    bestanden
16/21 Verdraengung: Wechsel ohne yield, Prioritaeten       bestanden
17/21 Ring 3: Programm, Systemaufruf, Schutzwall           bestanden
18/21 ELF-Lader aus dem Startdateisystem                   bestanden
19/21 Capabilities: Handle-Negativtests                    bestanden
20/21 Ring 3 wird verdraengt und FORTGESETZT               bestanden
21/21 Doku-Verweise zeigen auf den genannten Code          bestanden
############ ALLE TESTS BESTANDEN ############
```

Log of the run: `build/FINAL-21-verify.log`. Measured values of the run:
18,290 lines of Rust, 170/170 self-test checks during the boot, 38 checked
features per QEMU boot, 0 compiler warnings (switched to errors in the build
system).

## Added in round 3 (rework after the jury)

* **Ring 3 is preempted AND resumed** — not just cleared away. The frame of
  the privilege change lies on the kernel stack of the carrying thread
  (`TRAP_GAP`), which is why it is allowed to switch in the middle of the
  frame. Demonstrated in every boot (`Ring 3 verdraengt: … danach fortgesetzt
  … Ende 0`).
* **SMEP/SMAP really are on in every test run**: `run-qemu.sh` starts with
  `-cpu max`; the skip path is checked separately with `--cpu-basic`.
* **Warnings are errors** (`[workspace.lints.rust] warnings = "deny"`), and
  the proof runs against a log of a REAL fresh compile (`build.sh --fresh`,
  `BUILD-EVIDENZ` line).
* **Documentation references are checked mechanically** (`tests/verweise.py`,
  format `` `datei.rs:Zeile` (`Anker`) ``) — slipped line numbers show up
  immediately.

## What runs (do not reinvent — read it, then extend it)

* an own target `x86_64-osum-none.json`, `build.sh`, `run-qemu.sh`, `test.sh`, `rename.sh`
* Limine 9.6.7 vendored (offline), BIOS **and** UEFI, higher-half at `0xffffffff80000000`
* COM1 + framebuffer text console (an own 8×16 font), `println!`/`klog!`,
  a panic handler with a frame pointer backtrace
* GDT+TSS with 3 IST stacks, IDT with 256 gates (typed wrappers
  `set_handler*`), exceptions 0–21 in plain text, a **real** `#DF`
* PIC at `0x20`, PIT 100 Hz
* a bitmap frame allocator, **own** 4-level page tables + CR3 switching,
  rights per section, NX, CR0.WP
* an **own** heap allocator (no foreign crate) that grows up to 16 MiB
* `arch/x86_64/context.rs` (`naked_asm`) + `kcore/sched.rs`: a **cooperative**
  scheduler with sleep/wake, deadlock protection, stack guard zones
* `abi/native.rs` (handle table with generations), `abi/posix.rs` (feature-gated)
* 79 self-test checks in the boot log, 25 checked features per boot

## NEW in round 2 (hand work before this run, do not undo)

1. **Branding centralised.** `kernel/src/kcore/branding.rs` is the ONLY place
   with a product name. Fed from `kernel/Cargo.toml` (`name`,
   `[package.metadata.branding] os-name`) through `kernel/build.rs`.
   **Rule: no product name as a literal in `kernel/src`.** Instead of
   `"osum"` you write `kcore::branding::KERNEL_NAME`. `test.sh` step 14
   enforces it.
2. `./rename.sh <kernel> <os>` renames everything (verified with
   `./rename.sh nova Novaos` in `/tmp`, booted). Instructions: `RENAME.md`.
3. **Arch leak closed.** `main.rs` called
   `arch::x86_64::gdt::double_fault_stack_top()` directly. Now:
   `ArchOps::fault_stack_top() -> Option<VirtAddr>`, façade
   `arch::fault_stack_top()`.
4. New design documents that the work has to align with: `PACKAGING.md`,
   `FILESYSTEM.md`, `LANGUAGE.md`, an extended `ROADMAP.md`
   (phase 9 = aarch64/mobile).
5. Zero compiler warnings — enforced in `test.sh` step 15
   (`build/cargo-build.log` is checked).

## Hard rules for this run

1. **`./test.sh` has to be green at the end.** Run it before handing in and
   quote the result. The number of steps may rise, never fall.
2. No x86 details outside `kernel/src/arch/` — not in `main.rs` either:
   `grep -rnE '\b(cr[0-3]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq)\b' kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'`
   has to stay empty.
3. **No product name as a literal** in `kernel/src` apart from `branding.rs`.
4. **Zero compiler warnings.** No `todo!()`/`unimplemented!()`.
5. No new external crate without an entry in ARCHITECTURE.md § 8. Currently 2
   (`limine`, `spin`).
6. **Every place where Rust gets in the way is entered in `LANGUAGE.md`** —
   with file, line, problem, workaround, and what a language of our own would
   have to do differently.
7. Bring the documentation along and **re-measure** the key figures, do not
   guess them (`size -A`, `wc -l`, test counts from the run). A claim without
   code = a mistake.
8. Do not touch `vendor/limine/`.

## Toolchain

* rustc/cargo 1.99.0-nightly, `rust-src` present
* `build-std` is a flag in `build.sh`, **not** in `.cargo/config.toml`
  (otherwise the host tests fail with `duplicate lang item: sized`)
* qemu-system-x86_64 7.2, xorriso, nasm, mtools, OVMF under `/usr/share/OVMF/`
* no network needed, all crates are in the local Cargo cache
