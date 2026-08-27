# The kernel switch

**State: 2026-08-26.** OrientOS has given up its Rust kernel as a product.
The kernel now comes from the Osum repository — in Firn, with a history of
its own, pinned through `vendor/osum/COMMIT`. OrientOS is the **system
around it**.

This document is not an announcement. It records **what was compared before
anything was replaced**, what is ported, what was deleted — and what was
measured.

**The chronology in one paragraph.** On 2026-08-25 the Rust kernel still
stood there complete; the condition was: *delete nothing before the
replacement demonstrably runs*. On 2026-08-26 the last two open items were
ported to Firn (SMEP/SMAP, boot modules) and the Rust kernel was deleted —
**18,255 lines of Rust** and **1,206 lines of C**, plus 965 lines of Firn
that belonged to it. Section 7 says what exactly, and section 4 says what
was **not** ported and why it was allowed to go anyway.

**About the figures, precisely once.** What is counted is what `git` knew
(`git ls-tree -r 586ac70 --name-only | grep '\.rs$'`). Whoever counts in the
working directory finds 18,521 — the difference of 266 lines are generated
files under `build/` that were never checked in. Both figures are right;
this file names the checked-in one throughout.

---

## 1. Why at all

There were two kernels with the same name.

* **Osum** (its own repository): 26,088 lines of Firn in the kernel, 5,114
  lines of Firn in the userland, 1,598 lines of libc, 1,336 lines of
  assembly. Its own file system, an ELF64 loader off the disk, NVMe over
  DMA, PCI, APIC, SMP, TCP/IP, signals, terminals, a screen, a POSIX layer
  with the system call numbers of Linux, a shell with 25 tools.
* **The kernel of this repository**: 18,255 lines of Rust (`kernel/src`
  14,157, `libs/` 3,860, `kernel/build.rs` 144, the ELF yardstick 94) and
  965 lines of Firn. On 2026-08-21 `LANGUAGE.md` had recorded: *"osum
  becomes Firn-only"*, and the migration was running module by module — at
  about 5 %.

The arithmetic was simple. The way from 5 % to 100 % is about 18,000 lines
of porting work; the result would be a kernel that can do less than the one
that already stands finished in Firn next to it. So the question is not
*whether* but *what gets lost in the process* — and that is exactly what
section 2 is about, and it happened **before** the first deleted byte.

---

## 2. The comparison, module by module

The table compares **capabilities**, not line counts. A module that needs
800 lines in Rust and 500 in Firn is not worse for it — the question is
always: *who can do more, and what would be lost in the switch?*

Legend: **Osum** = only Osum can do this · **OrientOS** = only the Rust
kernel could do this · **both** = present in equal measure.

The rightmost column is the **state after the cut of 2026-08-26**.

### 2.1 Boot and architecture

| module (Rust) | counterpart in Osum | who can do more | state |
|---|---|---|---|
| `boot/limine.rs` (206) — the Limine protocol: memory map, HHDM, framebuffer, modules, RSDP | `kernel/boot.s` + `mem.fi` — Multiboot 1, memory map, command line, **framebuffer, modules** | both | **DONE.** UEFI on 2026-08-25 (Osum `c4427fa`), boot modules on 2026-08-26 (Osum `kernel/bootmod.fi`). Deleted. |
| `arch/x86_64/gdt.rs` (172), `idt.rs` (137), `interrupts.rs` (257) | `kernel/boot.s`, `idt.fi` (175), `trap.fi` (300), `isr.s` (501) | both | deleted |
| `arch/x86_64/paging.rs` (367) | `kernel/mem.fi` (532), `proc.fi` (777) | **Osum** (separate address spaces per process; OrientOS had one) | deleted |
| `arch/x86_64/context.rs` (132), `preempt.rs` (281) | `kernel/switch.s` (116), `sched.fi` (1,021) | both | deleted |
| `arch/x86_64/cpu.rs`, `msr.rs`, `port.rs`, `timer.rs` | `kernel/cpu.fi`, `hw.fi`, `apic.fi` | **Osum** (APIC/IOAPIC instead of only PIC/PIT) | deleted |
| `arch/x86_64/serial.rs` (59) | `kernel/serial.fi` (171) | both | deleted |
| `arch/x86_64/user.rs` (797) — ring 3, **SMEP/SMAP in CR4** | `kernel/user.fi` (268), `proc.fi`, `uprog.fi`, **`guard.fi` (328)** | both | **DONE** on 2026-08-26, see 3.3. Deleted. |
| `kcore/arch_iface.rs` (330) — the arch boundary as traits, with a test step that forbids x86 terms outside of `arch/` | *nothing* | **OrientOS** | **NOT PORTED.** Stays as `vorlage/arch_iface.rs` (378 lines), not built. See 4.1. |

### 2.2 Memory

| module (Rust) | counterpart in Osum | who can do more | state |
|---|---|---|---|
| `mm/frame.rs` (786), `mm/firn_bitmap.rs` (192), `kernel/firn/bitmap.fi` (344) | `kernel/mem.fi` (532) | both | deleted |
| `mm/heap.rs` (679), `libs/osum-mem/heap.rs` (829) | `kernel/mem.fi` (the heap) | both | deleted. OrientOS' heap was testable on the host (`cargo test`), Osum's only in QEMU — that is the only real loss of this block, and it costs checking comfort, not a capability. |
| `kcore/mem.rs` (461) — the types `PhysAddr`/`VirtAddr`/`MapFlags` | *implicit* | **OrientOS** (type safety) | deleted; the five types are in the header of `vorlage/arch_iface.rs` |

### 2.3 Processes and time

| module (Rust) | counterpart in Osum | who can do more | state |
|---|---|---|---|
| `kcore/sched.rs` (1,534) — threads, priorities, time slices, sleep/wake, an idle thread | `kernel/sched.fi` (1,021), `tasks.fi`, `proc.fi` | **Osum** | deleted. OrientOS: threads in **one** address space. Osum: processes with **their own** address space, `fork`, `execve`, `wait4`. |
| `kcore/preempt.rs` (248) | `sched.fi` + the timer | both | deleted |
| — | `kernel/smp.fi` (994), `smp.s` (159), `apic.fi`, `acpi.fi` | **Osum** | **Multiple processors.** OrientOS never had that. |
| — | `kernel/signal.fi` (1,134), `tty.fi` (727), `time.fi`, `rand.fi` | **Osum** | **Signals, terminals, clock, randomness.** OrientOS had none of it. |

### 2.4 Files, devices, userland

| module (Rust) | counterpart in Osum | who can do more | state |
|---|---|---|---|
| `kcore/initramfs.rs` (428) — an archive format of its own, CRC32 per entry, out of a bootloader module | `kernel/fs.fi` (935) — OFS with inodes, directories, `getdents64`; **`kernel/bootmod.fi` (251)** — a boot module with CRC32 | **Osum** | **DONE** on 2026-08-26, see 3.4. Deleted. |
| `kcore/elf.rs` (741), `kcore/firn_elf.rs` (265), `kernel/firn/elf.fi` (461) | `kernel/elf.fi` (803) | both | deleted — **but not the 53 test cases.** They now run against Osum's loader, see 7.3. |
| — | `kernel/blk.fi`, `hw.fi`, `nvme.fi` (698), `pci.fi` (591), `virtio.fi` (925), `inet.fi` (1,135) | **Osum** | **Block devices, PCI, NVMe over DMA, network.** OrientOS had not a single driver. |
| `drivers/fbcon.rs` (154) + `drivers/font.rs` (221) — a text console on the framebuffer, an 8×16 font of its own | `kernel/fb.fi` (1,520), `font.fi` (130), `tty.fi` (727) | **Osum** | **DONE** through Osum's round K7/K7B (76 checked assertions, measured against real screenshots). Deleted. |
| — | `kernel/kbd.fi`, `kernel/user/*.fi` (5,114): a shell with pipes and redirection, 25 tools | **Osum** | OrientOS' userland was **one** program in assembly (`hello.asm`, 64 lines). See 7.2. |

### 2.5 The ABIs

| module (Rust) | counterpart in Osum | who can do more | state |
|---|---|---|---|
| `abi/posix.rs` (188) + `libs/osum-abi-posix` (672) — POSIX as a separable translation layer onto the native ABI | `kernel/sys.fi` (3,252) + `lib/libc/` (1,598) — system calls **with the numbers of Linux x86-64**, a libc in Firn | **Osum** | deleted. OrientOS' layer was more cleanly cut (deselectable by feature), Osum's is incomparably more complete and carries a real userland. |
| `abi/native.rs` (2,013) + `libs/osum-abi-native` (1,643) — **capability handles**: slot+generation, 10 rights bits, 8 object kinds, channels, ports, namespaces, `ProcessSpawn` | `kernel/cap.fi` (357) + `sys.fi` from number 2000 on | split | The handle model is **ported** (3.2), the objects on it are **not** (4.2). Deleted. |

### 2.6 Everything above the kernel — stays with OrientOS

| part | role after the switch |
|---|---|
| `brands/*.toml`, `brand.sh`, `BRANDING.md`, `rename.sh` | **stays.** One source tree, two products — as in FreeViewer. The kernel is called `osum` in every brand, the way NT is called NT in every Windows edition. |
| `userland/`, `PACKAGING.md` | **stays, with a new role.** "One program in assembly" has become the **assembly of the product userland** (`userland/PROGRAMME`). See 7.2. |
| `ARCHITECTURE.md`, `ASSISTENT.md`, `ROADMAP.md` | **stays.** The architecture of the system and the interface for an assistant. |
| `test.sh`, `tests/step-*.sh` | **stays.** The acceptance suite of the **whole system**. Osum accepts its kernel itself (its own `./test.sh`, 15 sections). |
| `build.sh`, `vendor/limine/` | **stays.** OrientOS packs the kernel and the userland into a bootable product. |

---

## 3. What was ported

### 3.1 The UEFI boot path → Osum commit `c4427fa`

Osum booted only through BIOS. The cause was **one number**: the Multiboot
header said `MB_FLAGS = 0x3` (aligned modules + memory map). A loader reads
from that that the kernel has no screen request, and assumes **text mode**.
Under UEFI there is no text mode — Limine aborts:

```
PANIC: multiboot1: Cannot use text mode with UEFI.
```

Measured before the change came into being. With flag bit 2 and
`mode_type = 0` the kernel demands a **linear framebuffer** that the
firmware can set. After that **the same image** boots through SeaBIOS and
through OVMF.

Measured afterwards in `tests/step-30-boot.sh`: exit code 21 on both paths,
and the memory maps differ — these really were two different boots.

### 3.2 The capability handles → Osum commit `2d89634`

The most valuable part of the Rust kernel in substance. What is ported is
the **core of the model**, from `libs/osum-abi-native/` (Rust, no_std) to
`kernel/cap.fi` (Firn, 357 lines) — **with the same numbers and the same
meanings**: 10 rights bits, handle = slot + generation, 8 object kinds, a
per-process random value, 9 error values.

The three assertions, and every one of them is measured **from ring 3**:

1. **A fresh table is empty.** Nothing is inherited.
2. **A closed handle never hits anything again**, not even after the slot
   has been reused.
3. **Rights can only get smaller.**

The difference from POSIX in one line: a **valid** handle without the
required right is `RightsDenied` (−2), a **forged** one is `BadHandle`
(−1). POSIX has only `-EBADF` for both.

Measured in `tests/step-50-schutz.sh`: 18 checks, read back individually,
plus the counter-check that without the word `caps` nobody reports
anything.

### 3.3 SMEP/SMAP → Osum, `kernel/guard.fi` (2026-08-26)

The item that stood open as § 4.1 until 2026-08-26. `guard.fi` (328 lines
of Firn) sets both bits in CR4 as soon as CPUID reports them — **on every
core**, because CR4 is per processor, and `smp.ap_main` sets them too. The
`stac`/`clac` window stands in exactly four places (`sys.peek`, `sys.poke`,
`sys.copy_in`, `sys.copy_out`) plus at the signal frame
(`signal.poke64`/`peek64`).

What the Rust kernel had better here: nothing. What this port has in
addition: **the counter-checks**, and they are the real content.

| command line | expected | measured |
|---|---|---|
| `-cpu max` | CR4 = `0x300020` | ✔ |
| the QEMU default `qemu64` | CR4 = `0x20`, `smep=0 smap=0`, the run still ends with 21 | ✔ |
| `smapraw` with SMAP | `#PF err=0x1`, code **63** | ✔ |
| `smapraw nosmap` | comes back, reads `0x5a`, code **21** | ✔ |
| `smepraw` with SMEP | `#PF err=0x11` (bit 4 = instruction fetch), code **63** | ✔ |
| `smepraw nosmep` | `smep: came back (!)`, code **21** | ✔ |
| `-smp 4` | `guard: aps=3` — all three further cores carry the bits, read back | ✔ |
| `caps` with SMAP | `windows=312` — the window really is used | ✔ |

The difference between the run that stops and the run that goes through is
**one bit in CR4**.

### 3.4 Boot modules and checksums → Osum, `kernel/bootmod.fi` (2026-08-26)

The item that stood open as § 4.4 until 2026-08-26 — and the only one that
was not just a property but made **the difference between an ISO with a
userland and one without**.

`bootmod.fi` (251 lines of Firn) reads the loader's module table (Multiboot
1, flag bit 3), computes **CRC32** (IEEE, reflected, table-free — the same
polynomial as in `kcore/initramfs.rs`) over the whole module, compares it
with `modcrc=` and mounts it as the root disk. For `fs.fi` nothing changes
in the process: the same 512 bytes per block, only out of a different
device.

Three things that are measured in the process and without which it would be
a claim:

1. **Without a given value the checksum is a statement, with one it is a
   condition.** A wrong `modcrc=` leaves the module alone; the kernel says
   `ok=0` and carries on.
2. **A module nobody asked for does not quietly become the root.** Without
   `modfs` it is seen, computed and not used.
3. **The region belongs to the module.** The loader puts it in memory that
   the map lists as *usable* — `mem.scan` takes it back. The counter-check
   for that: the checksum is computed once more at the **end** of the run,
   after processes, page mappings and heap usage.
   `mod: recheck crc=… same=1`.

That is the reason why OrientOS' ISO has had a shell since 2026-08-26.

---

## 4. What is STILL open

Two items. Both are **not** ported, both stand in the product as missing,
and neither of them kept the Rust kernel alive.

### 4.1 The architecture boundary — the one remainder that stays

`kcore/arch_iface.rs` described as traits what an architecture has to be
able to do; a test step searched for x86 terms outside of `arch/` and
failed the run when it found any. Osum does not have this boundary — it is
x86-64 only, although Firn can do aarch64 as well.

**Not ported, and the reason fits in one sentence:** that would not be a
port but a rebuild of every module of that kernel. The x86 details are
spread there across `kmain.fi`, `proc.fi`, `mem.fi`, `apic.fi`, `smp.fi`,
`user.fi` and `guard.fi`, and you do not draw a boundary by laying traits
down next to it.

**That is why exactly this one file stays** — as `vorlage/arch_iface.rs`,
378 lines, **not built**: there is no `cargo` and no `Cargo.toml` in this
repository any more. It is the requirement for the round that will draw
this boundary in Osum, in the form in which it has worked once before. The
real content is not the trait text but the **rule**: no x86 term outside of
`arch/`, and a test step that enforces it.

The state is additionally in Osum's README under "What it lacks" and in
`ROADMAP.md` as an item of its own. It does not disappear quietly.

### 4.2 The rest of the native ABI

The handle model is ported (3.2). What is **not** ported are the objects
that hang off it:

| call | state in Osum |
|---|---|
| `ChannelCreate`, `ChannelSend`, `ChannelRecv` | `NotSupported` (−9) |
| `PortCreate`, `PortWait`, `PortBind` (the replacement for signals) | `NotSupported` |
| `NamespaceOpen`, `NamespaceCreate` | `NotSupported` |
| `ProcessSpawn` with an explicit handle list | `NotSupported` |
| `MemoryCreate`, `MemoryMap`, `MemoryUnmap` | `NotSupported` |

`NotSupported` and not `ENOSYS`: the call **exists** in this ABI, this
kernel just does not offer it. The **numbers, the error values and the
meanings** are ported and are in `kernel/sys.fi` from 2000 on — what is
missing are the objects behind them.

**Why the Rust version was deleted anyway.** It would have been a template
of 3,656 lines, for objects that will look different in Firn regardless
(stage 0 has no global mutable variables; a channel will be a region in the
data area, not a `Vec<Message>`). A template you have to rewrite line by
line is a specification — and that one is shorter and more precise in
Osum's `sys.fi`, where each of these calls already has its number and its
`NotSupported` branch. The Rust version is complete in the history:
`git show 91182a0:kernel/src/abi/native.rs`.

### 4.3 What is still missing but never hung on the Rust kernel

For completeness, so that the list does not look better than it is: Osum
has no window system, no name resolution, no USB, no SATA/AHCI, no
partition table reader, no journal, no users/permissions, no dynamic
linking. The Rust kernel had none of that either. It is in Osum's README
and in `ROADMAP.md`.

---

## 5. How the two fit together

```
osum (own repo, Firn)                    orientos (this repo)
├── kernel/*.fi   the kernel     ──┐     ├── vendor/osum/COMMIT   ← pinned
├── kernel/user/  shell, 25 tools  │     ├── brands/*.toml        brands
├── lib/libc/     libc in Firn     └──►  ├── userland/PROGRAMME   what goes into the ISO
├── vendor/firn/COMMIT (its own          ├── userland/dateien/    what OrientOS contributes
│                       compiler)        ├── PACKAGING.md         packages
└── test.sh       15 sections            ├── ASSISTENT.md         interface
                                         ├── build.sh             kernel + userland → ISO
                                         └── test.sh              acceptance of the system
```

`vendor/osum/hole-osum.sh` builds **both** from the pinned commit: the
kernel and the unprivileged programs — **with Osum's own Firn compiler**,
not with this repository's. The two repositories pin different Firn
commits, and it is meant to stay that way: two projects, two pins, no quiet
mixing.

Only the hash is checked in. No image, no program, no copying without
provenance.

---

## 6. The history of this switch

* **2026-08-19 to 2026-08-21** — `LANGUAGE.md` records: *"osum becomes
  Firn-only"*. The migration begins module by module: `serial.fi` (117
  lines), then the bitmap frame allocator (`bitmap.fi`, 344), then the ELF
  checking part (`elf.fi`, 461). After three rounds: 922 lines of Firn
  against 17,993 lines of Rust — about 5 %.
* **2026-08-25, in the morning** — Osum becomes a repository of its own.
  With that two kernels of the same name stand side by side, and one of
  them already is what the other wants to become.
* **2026-08-25** — the decision: *Osum is the kernel, OrientOS is the
  operating system around it.*
* **2026-08-25** — this comparison (section 2), **before** any deletion.
  Result: two things that would really have been lost (UEFI,
  capabilities), plus four smaller ones (SMEP/SMAP, the arch boundary, the
  framebuffer console, boot modules).
* **2026-08-25** — UEFI and the handle model ported to Firn (Osum
  `c4427fa` and `2d89634`). **Nothing deleted**: the Rust kernel stands
  there complete and is built, booted and measured in every test run.
* **2026-08-25, in the evening** — Osum's round K7/K7B delivers the screen:
  framebuffer, text console, an 8×16 font of its own, `/dev/fb`, 76
  assertions measured against real screenshots. With that the third of the
  four smaller items is done without anybody having ported it — it was
  needed in Osum anyway.
* **2026-08-26** — the last two items ported: SMEP/SMAP
  (`kernel/guard.fi`) and boot modules with CRC32 (`kernel/bootmod.fi`),
  together 579 lines of Firn, accepted with 55 checks in Osum's
  `tools/guard/run.sh` (section 15 of its `./test.sh`).
* **2026-08-26** — **the cut.** 18,255 lines of Rust, 1,206 lines of C and
  965 lines of Firn deleted, one file stays as a template. Section 7.
* **2026-08-26** — and the gain that is not on the deletion list: for the
  first time the product ISO has a **userland**. `build.sh` assembles an
  OFS file system from Osum's programs and `userland/PROGRAMME`, puts it
  next to the kernel as a boot module, and `/bin/sh` runs out of it in
  ring 3 — with SMEP and SMAP on.

---

## 7. The cut of 2026-08-26

**The condition was: delete nothing before the replacement demonstrably
runs.** It was kept — the replacement is a repository with 15 test sections
and over 1,100 checks, and the four open items were worked off before the
first `git rm` ran: two ported (3.3, 3.4), one was done anyway (the screen,
K7), one stays with a stated reason (4.1).

### 7.1 What was deleted

| gone | lines | replacement |
|---|---:|---|
| `kernel/src/` — the whole Rust kernel | 14,157 | Osum `kernel/*.fi`, 26,088 lines of Firn |
| `libs/osum-abi-native/`, `libs/osum-abi-posix/`, `libs/osum-mem/` | 3,860 | Osum `cap.fi`, `sys.fi`, `mem.fi` |
| `kernel/build.rs`, `tests/firn-elf/rust-massstab.rs` | 238 | nothing — no Rust is compiled here any more |
| `tests/firn-elf/*.c`, `tests/firn-bitmap/`, `tests/firn-fehler/` | 1,206 C | Osum's own test harnesses; the **53 ELF cases stay** (7.3) |
| `kernel/firn/{serial,bitmap,elf}.fi` and `tests/firn-fehler/*.fi` | 965 Firn | Osum `serial.fi`, `mem.fi`, `elf.fi` |
| `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `kernel/Cargo.toml`, `kernel/build.rs`, `kernel/linker-x86_64.ld`, `x86_64-osum-none.json` | — | no cargo in the tree any more |
| `run-qemu.sh`, `limine.conf` | — | `run-osum.sh`; `limine.conf` is generated by `build.sh` |
| `userland/hello.asm`, `user.ld`, `mkinitramfs.py`, `mkbroken.py` | — | Osum's 25 tools in the boot module (7.2) |
| `tests/step-16`…`step-23`, `tests/verweise.py` | — | new steps that measure the **product** |

**All of it with `git rm`.** The history has every line of it; `test.sh`
checks that (`git log -- kernel/src` has to deliver commits). A deleted
kernel is not forgotten, it is only out of the way.

What was **not** deleted although it is C: `vendor/limine/*.c` — that is
the bootloader, not the kernel, and it is still needed.

### 7.2 What became of `userland/`

Before: **one** program, `hello.asm`, 64 lines of assembly, in an archive
format of its own that only the Rust kernel could read.

After: `userland/PROGRAMME` — the **assembly of the product userland**.
Osum writes and builds the programs, OrientOS decides which ones go into
the product and what else lies on the root (`userland/dateien/`). Out of it
comes an OFS file system that lies in the ISO as a boot module.

The proof that this is not a rename is in `tests/step-40-userland.sh`:
`/bin/sh` starts **out of the module**, `ls /bin` shows the tools,
`cat /etc/ausgabe.txt` reads a file that comes from **this** repository,
and `wc -l` counts the same number of lines in it as the host does.

What is to become of it is in `userland/README.md` and `PACKAGING.md`:
brand-dependent lists, then a package index with content hashes, then a
write path to a real disk.

### 7.3 What became of the ELF corpus

`tests/firn-elf/faelle/` — 53 hand-built ELF64 headers, each with exactly
**one** defect — is the only piece of the Rust kernel that was not deleted
and still does something. The code that checked them is gone. The cases are
better than the code that produced them.

They now lie in the **product ISO** under `/f/`, and the shell tries to
start every one of them in **one** boot (`tests/step-60-elf-korpus.sh`).
What is measured is not "all of them are rejected" — Osum's loader has
different limits for `USER_BASE` and `USER_LIMIT` than the Rust kernel, and
a number nobody has recomputed does not belong in a check. What is
measured: the kernel **survives all 53**, the rejections carry error values
instead of crashes, the loader names more than one reason, and the shell
carries on working afterwards.

### 7.4 The figures after the cut

| | before (08-25) | after (08-26) |
|---|---:|---:|
| Rust in `orientos` (checked in) | 18,255 | **0** (+ 378 lines of template, not built) |
| C in `orientos` (without `vendor/limine`) | 1,206 | **0** |
| Firn in `orientos` | 965 | **0** — Firn now stands completely in Osum |
| Firn in `osum` (kernel) | 25,509 | **26,088** |
| programs in the product ISO | 1 (`hello`) | **26** |
| test sections in `orientos` | 19 | 8 |
| checks in `orientos` | 166 | **151**, all with a counter-check |
| test sections in `osum` | 14 | **15** |

The section count of OrientOS drops, and that is as it should be: thirteen
of them measured a kernel that is not written in this repository any more.
They have not disappeared — they are in Osum's `./test.sh`, and there they
have become more.
