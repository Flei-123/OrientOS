# RUN — building, starting and checking OrientOS

> **SUPERSEDED ON 2026-08-26.** This document describes work on the
> **Rust kernel** of this repository. That kernel is deleted — the kernel
> comes from the Osum repository ([KERNELWECHSEL.md](KERNELWECHSEL.md)).
> Everything here that names `run-qemu.sh`, `cargo`, `kernel/src` or
> `Cargo.toml` no longer exists; where things are now is stated in
> [tests/GELOESCHT.md](tests/GELOESCHT.md). The document stays as a
> **record**: what was planned and measured back then is part of the
> history of this project and is not rewritten after the fact.

**The way that holds today, in four lines:**

```sh
./build.sh                              # build/orientos.iso (kernel + userland module)
./run-osum.sh                           # boot through SeaBIOS, serial output in the terminal
./run-osum.sh --uefi                    # the same image through OVMF
./run-osum.sh --script 'ls /bin;exit'   # hand in a shell script
```

Everything else is in [README.md](README.md) under "Scripts".


Kernel `osum`, operating system `OrientOS`, Rust `no_std`, target
`x86_64-osum-none`. All commands run in the project root directory, all paths
are relative.

## Prerequisites

```sh
rustup toolchain install nightly && rustup default nightly
rustup component add rust-src llvm-tools
sudo apt install qemu-system-x86 xorriso mtools nasm ovmf python3
```

`nasm` + `ld` + `python3` are needed for the unprivileged program and the boot
file system. If they are missing, `build.sh` still builds through and the
kernel reports honestly that there is no archive — the ELF tests then drop out.

Network access to crates.io is not needed; Limine lies prebuilt in
`vendor/limine/`. `build-std` is deliberately a flag in `build.sh` and **not**
in `.cargo/config.toml` — otherwise the host tests fail on
`duplicate lang item`.

## The short way

```sh
./build.sh          # kernel + userland + initramfs + bootable build/orientos.iso
./run-qemu.sh       # interactive start, serial output in the terminal (Ctrl-C ends it)
./test.sh           # the full proof: 19 steps, exit code 0 = everything green
```

One run of `./test.sh` takes about **2.5 minutes** on this machine (154 s
measured, 14 real QEMU boots). After a `cargo clean` it is about **6.5
minutes** (392 s), because `core`/`alloc` get recompiled through `build-std`.

## Individual switches

| command | effect |
|---|---|
| `./build.sh` | release build + userland + `build/initramfs.img` + ISO + symbol map `build/osum.map` |
| `./build.sh --debug` | debug build |
| `./build.sh --no-posix` | counter-check without the POSIX layer (`--no-default-features`) |
| `./run-qemu.sh --check` | boots and checks 38 features in the log, exit code 0/1 |
| `./run-qemu.sh --check --no-posix` | has to report „posix-Schicht NICHT einkompiliert“ |
| `./run-qemu.sh --check --uefi` | boot through OVMF instead of SeaBIOS |
| `./run-qemu.sh --test-pagefault` | a deliberate `#PF`, CR2 and cause in plain text |
| `./run-qemu.sh --test-doublefault` | a **real** `#DF` on its own emergency stack (not `int 8`) |
| `./run-qemu.sh --test-panic` | the panic handler with a backtrace |
| `./run-qemu.sh --test-rodata` | an attempt to write to `.rodata` has to trigger a `#PF` |
| `./run-qemu.sh --test-nx` | an attempt to execute data has to trigger a `#PF` |
| `./run-qemu.sh --test-gp` | a non-canonical address has to trigger a `#GP` |
| `./run-qemu.sh --test-ud` | an invalid instruction (`ud2`) |
| `./run-qemu.sh --test-preempt` | preemption: counting loops without `yield`, tick distribution by priority |
| `./run-qemu.sh --test-ring3` | an unprivileged program in ring 3 + a negative test on a kernel address |
| `./run-qemu.sh --test-elf` | load ELF64 out of the boot file system, refuse garbage in a defined way |
| `./run-qemu.sh --test-handles` | handle negative tests (index, generation, missing right) |
| `./rename.sh <kernel> <os>` | renames kernel and OS across the whole tree (see RENAME.md) |

### Deselecting POSIX — the exact command line

`./build.sh --no-posix` is the recommended form. Whoever calls `cargo`
directly **has to pass the `build-std` flags along** — they are deliberately in
`build.sh` and not in `.cargo/config.toml` (otherwise the host tests fail on
`duplicate lang item: sized`). A bare `cargo build --no-default-features`
therefore fails with `can't find crate for core`; that is expected and not a
defect of the POSIX separation. The complete form:

```sh
cargo build -Z build-std=core,compiler_builtins,alloc \
            -Z build-std-features=compiler-builtins-mem \
            --release --no-default-features
```

The boot without POSIX then reports in the log:

```
[osum] boot       ABI         : osum-native (POSIX nicht einkompiliert)
[osum] abi        posix-Schicht NICHT einkompiliert — Core laeuft ohne sie
```

Host tests of the hardware-free logic alone (146 tests, they run in seconds):

```sh
cargo test --target x86_64-unknown-linux-gnu \
    -p osum-mem -p osum-abi-native -p osum-abi-posix
```

## What `./test.sh` covers

The basic steps 1–15 are in `test.sh`, the proofs of the individual work sites
as separate files in `tests/step-*.sh` (they are appended and counted
automatically).

1. host tests (146) · 2. build with POSIX · 3. build without POSIX ·
4. boot BIOS · 5. boot without POSIX · 6. `#PF` · 7. a real `#DF` · 8. panic ·
9. `.rodata` write-protected · 10. NX · 11. `#GP` · 12. `#UD` ·
13. boot through UEFI · 14. the product name only from `branding.rs` ·
15. the architecture boundary and code hygiene · 16. preemption ·
17. ring 3 · 18. the ELF loader · 19. capabilities

Step 15 checks among other things that **no** x86 details stand outside
`kernel/src/arch/` in the code, that there is no `todo!()`, that the last build
produced **zero** compiler warnings and that every `test-*` feature from
`kernel/Cargo.toml` also has a switch in `run-qemu.sh`.

## Evidenced state (last complete run, 2026-08-13)

```
$ ./test.sh ; echo "EXIT=$?"
 1/19 Host-Tests der architekturunabhaengigen Crates                        => bestanden
 2/19 Kernel baut (Release, mit POSIX)                                      => bestanden
 3/19 Kernel baut OHNE POSIX-Schicht                                        => bestanden
 4/19 Boot in QEMU (BIOS)                                                   => bestanden
 5/19 Boot in QEMU ohne POSIX-Schicht                                       => bestanden
 6/19 Selbsttest Page Fault                                                 => bestanden
 7/19 Selbsttest Double Fault (echter #DF auf IST-Stapel)                   => bestanden
 8/19 Selbsttest Panic-Handler                                              => bestanden
 9/19 Selbsttest .rodata schreibgeschuetzt (#PF erwartet)                   => bestanden
10/19 Selbsttest NX: Daten ausfuehren (#PF mit Instruktionsabruf erwartet)  => bestanden
11/19 Selbsttest #GP: nicht kanonische Adresse (kein #PF, sondern #GP)      => bestanden
12/19 Selbsttest #UD: ungueltige Instruktion (ud2)                          => bestanden
13/19 Boot ueber UEFI (OVMF)                                                => bestanden
14/19 Produktname kommt nur aus branding.rs                                 => bestanden
15/19 Architekturgrenze und Codehygiene                                     => bestanden
16/19 Verdraengung: Wechsel ohne freiwilliges yield, Prioritaeten wirken    => bestanden
17/19 Ring 3: unprivilegiertes Programm, Systemaufruf, Schutzwall           => bestanden
18/19 ELF-Lader: Programm aus dem Startdateisystem, Muell wird abgewiesen   => bestanden
19/19 Capabilities: Handle-Negativtests im Boot-Log                         => bestanden
############ ALLE TESTS BESTANDEN ############
EXIT=0
```

Demonstrable in the boot log: the memory map, the frame statistics, our own
switching of the address space, the heap init with a test allocation
(Box/Vec/String), timer ticks, 30 **forced** thread switches without a single
`yield`, a program in ring 3 (`CS=0x0023 (RPL=3), CPL=3`), an ELF64 loaded from
the boot file system, the handle negative tests and the balance
`Selbsttestbilanz: 166/166 bestanden`.

## Troubleshooting

* Resolve backtrace addresses:
  `llvm-addr2line -e target/x86_64-osum-none/release/osum -f -C 0x…`
  or look them up in `build/osum.map`.
* The serial log of the last `--check` run lies in `build/boot.log`.
* If QEMU hangs, the time limit in `run-qemu.sh` takes hold (25 s until the
  abort mark).
* No boot file system in the log? Then `nasm`, `ld` or `python3` was missing at
  build time — `build.sh` says so in one line beginning with `Hinweis:`.
