# PLAN — round 3: from a boot kernel to a system with a real userspace

> **SUPERSEDED ON 2026-08-26.** This document describes work on the
> **Rust kernel** of this repository. That kernel is deleted — the kernel
> comes from the Osum repository ([KERNELWECHSEL.md](KERNELWECHSEL.md)).
> Everything here that names `run-qemu.sh`, `cargo`, `kernel/src` or
> `Cargo.toml` no longer exists; where things are now is stated in
> [tests/GELOESCHT.md](tests/GELOESCHT.md). The document stays as a
> **record**: what was planned and measured back then is part of the
> history of this project and is not rewritten after the fact.


This run **extends** an already booting kernel. Starting state:
`PREFLIGHT.md` (15 steps green, BIOS + UEFI). Throw nothing away, rebuild
nothing, replace no working file. **A green boot is worth more than half a
feature.**

---

## 0. What the lead has already built in this run (it stands, it is green)

All the seams of the four milestones are **created, wired up and visible in
the boot log**. No `todo!()`, no warning, `./test.sh` stays green. That way
every module can start right away in ITS file, without waiting for another one.

| seam | file | called from | state today |
|---|---|---|---|
| `ArchOps::{set_preemption, preemption_available}` | `kcore/arch_iface.rs` → `arch/x86_64/preempt.rs` | `kcore::preempt` | reports `nicht verfuegbar` |
| `kcore::preempt::{on_tick, note_*_switch, quantum, enable}` | `kcore/preempt.rs` (new) | the timer path | counters run, `on_tick` delivers the end of the time slice |
| `kcore::sched::preempt_selftest()` | `kcore/sched.rs` | `kcore::preempt::boot_selftest` | a placeholder line, delivers `(0,0)` |
| `ArchOps::{cpu_features, init_user_support, user_support_ready, set_kernel_stack, enter_user, code_selector}` + `CpuFeatures`, `UserExit` | `kcore/arch_iface.rs` → `arch/x86_64/user.rs` | `kcore::user` | reports `nicht verfuegbar` |
| `kcore::user::{USER_BASE, USER_LIMIT, is_user_addr, init, run, report_exit, boot_selftest}` | `kcore/user.rs` (new) | `kmain` | logs the CPU protection features honestly |
| `kcore::elf::{parse, Image, Segment, ElfError, selftest}` | `kcore/elf.rs` (new) | `kmain` | **finished and green: 11/11 negative cases** |
| `kcore::initramfs::{set, image, find, count, report}` | `kcore/initramfs.rs` (new) | `kmain` | reports `kein Archiv uebergeben` |
| `abi::native::capability_selftest()` | `abi/native.rs` | `kmain` | **finished and green: 3/3 handle negative tests** |
| the self-test balance extended | `main.rs` | — | `93/93 bestanden (… Praeemption 0/0, ELF 11/11, Ring3 0/0, Handles 3/3)` |
| the features `test-preempt/test-ring3/test-elf/test-handles` | `kernel/Cargo.toml` | — | created, the switches exist in `run-qemu.sh` |
| a log contract per feature | `run-qemu.sh` (`case "$FEATURES"`) | CI | the patterns stand, waiting for the modules |
| test steps as individual files | `test.sh` + `tests/step-*.sh` | CI | `test.sh` counts automatically; every module creates ITS file |

The measured state after the skeleton (a real run, `./run-qemu.sh --check`):

```
[osum] elf        ELF-Negativtest: 11/11 Faelle wie erwartet abgewiesen
[osum] abi        Handle-Negativtest: 3/3 abgewiesen (ungueltiger Index ok,
                   veraltete Generation ok, fehlendes Recht ok)
[osum] boot       Selbsttestbilanz: 93/93 bestanden
```

---

## 1. Immovable rules (they apply to EVERY module)

1. After every change: `./build.sh && ./run-qemu.sh --check`. Before handing
   in: `./test.sh` completely. Red = take the change back.
2. **The arch boundary.** No x86 term outside `kernel/src/arch/`:
   `grep -rnE '\b(cr[0-4]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq|swapgs|sysret|STAR|LSTAR|SFMASK|SMEP|SMAP)\b' kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'`
   has to stay empty (step 15 checks it). `main.rs` too.
3. **The product name** only from `kcore/branding.rs` (step 14).
4. Zero compiler warnings (`-D warnings`), no `todo!()`/`unimplemented!()`.
5. No new external crate without a justification in `ARCHITECTURE.md` § 8.
   Currently two: `limine`, `spin`. Our own heap/bitmap/page tables stay.
6. `--no-default-features` has to **build and boot**. Nothing POSIX-like
   (errno, fork, inode) in `kcore/`.
7. Every new Rust friction point (naked_asm, ABI, a kind of unsafe, allocator)
   is logged in `LANGUAGE.md`: file, line, problem, workaround, what a language
   of our own would have to do differently.
8. Do not touch `vendor/limine/`. No renamings, no reformatting.
9. **Respect file ownership** (section 3). Whoever touches somebody else's file
   creates a conflict and thereby work for everyone.

---

## 2. The log contract (this is what the jury measures and what `run-qemu.sh` checks)

The patterns are already in `run-qemu.sh` in the block `case "$FEATURES"`.
Whoever builds his module builds against exactly these lines:

**preempt** (`--test-preempt`)
```
Praeemption : aktiv, Zeitscheibe N Tick(s)
praeemptive Wechsel: N, kooperative Wechsel: M
ohne yield: N Wechsel zwischen 3 Threads
Thread 1: Prio 2, 41 Ticks
Prioritaet wirkt: Thread 1 (Prio 2) 41 Ticks > Thread 2 (Prio 1) 20 Ticks
```

**ring3** (`--test-ring3`)
```
Ring 3      : CS=0x002b (RPL=3), CPL=3, Stapel=0x00007fff..., N Systemaufruf(e), Ende 0
Ring-3-Zugriff auf Kerneladresse 0xffffffff8...: sauber abgewiesen (Schutzverletzung, Userspace)
```

**elf** (`--test-elf`)
```
Initramfs   : 2 Eintraege, 8192 B
ELF-Lader   : hello, 2 Segmente geladen, Einsprung 0x0000000000401000
  Segment 0: 0x401000 4 KiB RX
ELF-Negativtest: 11/11 Faelle wie erwartet abgewiesen
```

**handles** (`--test-handles`)
```
Handle-Negativtest: 3/3 abgewiesen (ungueltiger Index ok, veraltete Generation ok, fehlendes Recht ok)
```

In addition, for EVERY run: no line with `FEHLER` or `WARNUNG`, the self-test
balance has to be `N/N`, and `Startvorgang abgeschlossen` has to come.

---

## 3. File ownership — who may touch what

**Locked (the lead's, nobody else changes them):**
`kernel/src/main.rs`, `kernel/src/kcore/mod.rs`, `kernel/src/kcore/arch_iface.rs`,
`kernel/src/arch/mod.rs`, `kernel/src/arch/x86_64/mod.rs`, `kernel/Cargo.toml`,
`Cargo.toml`, `run-qemu.sh`, `test.sh`, `PLAN.md`.
If something is missing there, it is reported to the lead — not changed
yourself.

| module | may change/create exclusively these files |
|---|---|
| **preempt** | `kernel/src/arch/x86_64/preempt.rs`, `.../idt.rs`, `.../timer.rs`, `kernel/src/kcore/preempt.rs`, `kernel/src/kcore/sched.rs`, `kernel/src/arch/x86_64/context.rs`, `tests/step-16-preempt.sh` |
| **ring3** | `kernel/src/arch/x86_64/user.rs`, `.../gdt.rs`, `.../interrupts.rs`, `.../msr.rs`, `.../cpu.rs`, `kernel/src/kcore/user.rs`, `tests/step-17-ring3.sh` |
| **elf** | `kernel/src/kcore/elf.rs`, `kernel/src/kcore/initramfs.rs`, `kernel/src/boot/limine.rs`, `kernel/src/boot/mod.rs`, `kernel/src/mm/mod.rs`, `userland/**` (new), `build.sh`, `limine.conf`, `tests/step-18-elf.sh` |
| **handles** | `kernel/src/abi/**`, `libs/osum-abi-native/**`, `libs/osum-abi-posix/**`, `tests/step-19-handles.sh` |
| **doku** | `ARCHITECTURE.md`, `ROADMAP.md`, `README.md`, `PACKAGING.md`, `FILESYSTEM.md`, `LANGUAGE.md`, `RENAME.md`, `RUN.md` — and **no** source file |

Overlaps are deliberately excluded: `preempt` owns the interrupt ENTRY POINT
(`idt.rs`, its own entry in `preempt.rs`), `ring3` owns the exception HANDLERS
(`interrupts.rs`) and the descriptor tables (`gdt.rs`).

---

## 4. Order and dependencies

* **preempt**, **ring3** and **handles** can start immediately and
  independently.
* **elf** delivers `kcore::initramfs::find("hello")`; **ring3** uses that as
  soon as it is there. Until then **ring3** may use an embedded byte array as
  an intermediate step — in the end the program has to come out of the archive,
  otherwise the intermediate step is listed as open in ROADMAP.md.
* **handles** needs from **ring3** only the dispatcher entry
  `abi::native::dispatch(nr, args) -> i64` (it already exists, the signature
  stays).
* **doku** measures afterwards: every figure in the README (kernel size,
  frames, lines, test count) and every log excerpt has to come from a REAL run.

## 5. If something does not run stably

Leave it out, report `nicht verfuegbar` honestly in the log, list the item as
open in `ROADMAP.md` — and do NOT create the corresponding `tests/step-*.sh`.
A kernel that does not boot is a total failure, an honestly open item is not.
