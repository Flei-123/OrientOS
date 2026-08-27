# Roadmap of OrientOS

Goal: an operating system of its own — no fork, no adopted foreign code,
kernel and system in [Firn](LANGUAGE.md). Time horizon: years. Sorted by
dependency, not by attractiveness.

Legend: **done** · **partial** · **open**

What runs today is in [README.md](README.md) under "What it can do" and
"What it cannot do". This file says what is still to be done.

**State of the figures: 2026-08-26, Osum `c5fe12f`.** Every figure in this
file comes from a run that was really carried out — the kernel figures from
the runners of the respective round (`tools/k13/run.sh`, `tools/k14/run.sh`,
`tools/k15/run.sh`, `tools/k16/run.sh` in the Osum repository, logged in its
`docs/ROUNDK*.md`), the line counts recounted with `wc -l` on the tree of
`c5fe12f` and not taken over from the logbooks. Where a figure from a
logbook differs from the recounted one, the recounted one is what stands
here.

**What these figures are NOT: a total.** The four rounds K13 to K16 were each
measured for themselves against the same 1486 checks of `main`, in four
separate runs. `./test.sh` in the Osum repository has therefore **not** been
run in one go over the merged state, and an added-up total would be a claim.
What stands here are the figures per round.

---

## 1 — Making the system usable

Without these items Osum is a demo system, not a working tool.

| | What | State |
|---|---|---|
| 1.1 | **Editor** — full-screen, raw terminal mode, search/replace, undo. Without it no file on the system can be changed. | **done** (K11, `/bin/edit`) |
| 1.2 | **Toolbox** — `find`, `sed`, `diff`, `patch`, `tar`, `gzip`, `xargs`, `du`, `top`, `mount`, `cut`, `tr`, `tee` | **done** (K11, 20 tools measured against their GNU counterparts) |
| 1.3 | **Shell scripts** — `if`, `for`, `while`, `case`, functions, variable substitution, `test` | **done** (K11) |
| 1.4 | **Users and permissions** — `uid`/`gid`, `chmod`, `chown`, `setuid`, login, `passwd` | **done** (K13). The ids live in the task record and are inherited in `sched.create`, so equally for `fork`, `execve`, `SYS_OSUM_SPAWN` and `proc.create`. The permission check is **one** function (`kernel/perm.fi`, 285 lines, `may_ids`), called from five gates — `open`, `mkdir`, `unlink`, `chdir`, `execve`; there is no second version of the rule in the tree. OFS carries `mode`, `uid`, `gid` in the inode. Tools in ring 3: `id` (120 l.), `whoami` (30), `chmod` (192, octal **and** letter form), `chown` (104), `su` (114), `passwd` (115), `login` (123). 103 declared system calls (79 + 24). Measured: `tools/k13/run.sh` **99 checks, 0 failures**; in the run 177 permission questions, 2 of them denied, the setuid bit taking effect 6 times |
| 1.5 | **`init` and services** — a first process that starts services, watches them, restarts them and cleans up on shutdown | **done** (K13). The kernel starts `/sbin/init` as process 1 instead of the shell (`kernel/kmain.fi`, line 1819 ff.); `init` (474 l.) reads `/etc/inittab`, starts services, restarts crashed ones, adopts orphans and shuts down through real ACPI. Plus `svc` (137 l.) for inspecting and controlling. The escape hatch is deliberately there and named: if there is no `/sbin/init` on the disk, the kernel takes `/bin/sh` — otherwise every image from the rounds up to K12 would have become unbootable (`kernel/kstate.fi`, word `initsh`) |
| 1.6 | **VFS** — mount several file systems side by side | **done** (K14). A file system registers with **nine** operations (`lookup readdir attr read write create unlink rename trunc`) plus a mask saying which of them it really can do; `vfs.fi` checks the bit **before** it uses the pointer. These are real indirect calls (`call *%rax`) — Firn stage 0 can neither turn a function pointer into a number nor call one out of a table field, but it can call a struct field. Files: `vfsops.fi` 138 l. (the shape), `mnt.fi` 327 (mount table, 8 entries of 128 bytes), `vfs.fi` 530 (the namespace), `ofs.fi` 178 (OFS as a driver of this layer), `procfs.fi` 1226 (`/proc`, produced in memory), `devfs.fi` 486 (`/dev` as a real file system). With that the special call `SYS_OSUM_PSTAT` is gone as well, the one `ps` and `top` used to ask through. Measured: `tools/k14/run.sh` **152 checks, 0 failures** |
| 1.7 | **FAT32** — so that Osum reads disks another system has written | **done, and more than asked for** (K14): `kernel/fat.fi`, 2007 lines, 93 functions, **reading AND writing**, with long names; measured against `mkfs.vfat`, `mcopy`, `mdir` and `fsck.fat`. Plus `kernel/part.fi` (498 l.): MBR and GPT with **both** CRC32 checksums — before that Osum looked in block 0 for an OFS superblock and found a partition table on every foreign disk. Named limits: only 512 bytes per sector (a FAT16 is refused), names up to 63 characters, a NEW name only in US-ASCII, at most 16 simultaneously open nodes, no `fsync` |
| 1.7b | **ext4 (reading)** — the second part of the old item 1.7, split off here deliberately | **open**. In the kernel tree of `c5fe12f` there is **not one line** about it: `grep -ril ext4 kernel/` finds nothing. This is not a subclause of 1.7 but a file system of its own, with extents, a journal and feature bits |

## 2 — User interface

| | What | State |
|---|---|---|
| 2.1 | **Mouse** (PS/2, later USB HID) | **done** (K10, PS/2 on IRQ 12) |
| 2.2 | **Window server** — create, move, stack windows, focus, event delivery, recompose only the dirty regions. Through capability handles. | **done** (K10) |
| 2.3 | **Real fonts** — read and rasterise TrueType, antialiasing, kerning. The 8x16 font is only good enough for a console. | **done** (K10, TrueType with antialiasing, checked per character against a second rasterisation) |
| 2.4 | **Widget library** — buttons, lists, scroll bars, text fields, menus | **done** (K15), and specifically **in ring 3**: `kernel/user/wlib.fi` 2843 lines (widgets, layout, event loop) and `kernel/user/wlibc.fi` 862 (canvas, primitives, glyphs, colour scheme) against **499** lines of seam in the kernel (`kernel/wig.fi`, seven calls 1800..1806 that know no widget). The canvas is a **strip** of 800 x 81 pixels (259,200 bytes from `mmap`) and not the window buffer — a process has 832 KiB here between `brk` and `mmap`, a window of 640 x 420 needs 1,075,200 bytes. What is measured is **per character** and not per surface (the lesson from K7B, where text was "87 percent" right while every single letter was missing). Counter-check `nodirty`: 18,966,648 instead of 40,800,000 recomposed pixels for the same click |
| 2.4a | **NO graphical interface designer.** Explicitly rejected on 2026-08-26. The widget library is used like Windows Forms WITHOUT the designer: assemble windows in the source text, no drag surface, no property window, no code generator. It stands here so that the question does not come back in half a year. | rejected |
| 2.5 | **Terminal window** — the shell in a window instead of on the whole surface | **done** (K10, `/bin/sh` runs in it) |
| 2.6 | **File explorer** — `/bin/explorer`, displayed as File Explorer, second name `/bin/files`. NO proper name: the name IS the description, as in Windows. Plus a program directory `/apps/<name>.prog/` (bundles as with Apple, but NOT `.app` — too much Apple) and an instant search across the whole file system modelled on Everything. | **done** (K15). `kernel/user/explorer.fi` 1013 lines; `/bin/files` is a **second directory entry pointing at the same inode**, not a second copy. Bundles: `kernel/user/appdir.fi` 503 l., five `.prog` directories with `INFO`, `start`, `symbol` (format OSYM, 12 byte header) and `daten/`; `start` is again a second name for `/bin/<prog>`, so five bundles for 896 KiB of programs cost 5 x 1036 bytes of icon and **not one** block of code. Launcher with keyword search: `starter.fi` 443 l. Name index by the Everything method: `kernel/nidx.fi` 191 l. (journal ring in kernel memory, written in exactly two places — `fs.dir_add` and `fs.dir_remove`), `kernel/user/nidx.fi` 531 l., `suchen.fi` 356 l. The figures are below in a table of their own. Measured: `tools/k15/run.sh` **251 checks, 0 failures**, 31 QEMU runs, 26 of them with a screenshot |
| 2.6b | **The name index, in figures** — it stands here and not in a footnote, because it is the only claim of this group that asserts an order of magnitude | **measured** (K15, `tools/k15/gross.py`, image with 4000 empty files in seventeen folders, `--inodes=4096`). See the table below this group |
| 2.7 | **Settings, themes, wallpapers** — colour scheme as a file, load an image, a program for it. Legwork, as soon as the window server stands. | open |
| 2.8 | **Task management** — `top` on the console, then graphical: processes, memory, load, terminating | `top` done (K11), graphical open |

### The name index against the directory walk (K15)

An image with **4000 empty files** in a tree of seventeen folders, measured
in the same process: first the index, then the same search term as a
recursive tree walk.

| | |
|---|---:|
| names from the inode table | **4021** |
| system calls for building it | 65 (63 records per call) |
| time for building it | 820,123 µs, i.e. 203 µs per name |
| truncated | 0 |

| word | hits | index | tree walk | names differing | faster by |
|---|---:|---:|---:|---:|---:|
| `kupfer` | 1 | 6,966 µs | 1,966,404 µs | **0** | **282x** |
| `07` | 179 | 9,228 µs | 1,766,744 µs | **0** | 191x |
| `quaste` | 0 | 6,892 µs | 1,804,693 µs | **0** | 261x |

**Why 282 stands here and not 332.** The brief for this catch-up round said
"factor 332". In the logbook of the round (Osum `docs/ROUNDK15.md`, section
6c) stand 282, 191 and 261 — depending on the search word. What is entered
is what was measured. What the runner pins down is not the microseconds
anyway, but the **hit counts** and the **equality of the names**: both ways
deliver not just the same number but the same names, sorted and compared
byte for byte (`ungleich=0`). The times vary by about a fifth with the load
of the machine, the order of magnitude does not.

**And the counter-check was broken at first, that belongs here too.** The
tree walk initially remembered *every* name it read and was full after sixty
names — it then no longer descended into any subdirectory and found too
little. On top of that `getdents64` was quadratic (31,000 read operations
for 250 files), the first measuring run over 4000 files ran for ten minutes
and was not finished. A counter-check you win by tripping your opponent is
none; both were repaired before the figure above came into being.

**What the index CANNOT do**, so that the figure does not promise more than
it covers: it survives no restart (the journal lies in kernel memory,
rebuild 0.94 s for 4021 names), the journal ring holds 56 records and
reports `lost` when it overflows, above 4200 names it reports "truncated",
and it holds **names, not paths**.

## 3 — Devices

| | What | State |
|---|---|---|
| 3.1 | **USB** — xHCI host controller, device classes (keyboard, mouse, mass storage), hot plugging. The biggest chunk of this group, and a precondition for almost every real machine. | **in progress: round K17** (Osum, branch `k17-usb`, branched off `c5fe12f`). State on 2026-08-26, 15:30: `kernel/xhci.fi` with 1105 lines in the working tree, not yet checked in; plus changes to `kmain.fi`, `kstate.fi`, `ps2m.fi`, `boot.s`. There are **no measured figures** yet — what stands here is the size, not the result |
| 3.2 | **Sound** — AC97 or Intel HDA, mixer, volume, a playback program | open |
| 3.3 | **Touchscreen** — through USB HID or I2C; only sensible after 2.2 and 3.1 | open |
| 3.4 | **Graphics card** — today only the linear framebuffer of the bootloader. Mode setting and acceleration would be the next stage. | open |
| 3.5 | **More disks** — AHCI/SATA besides NVMe | open |

## 4 — Power and performance

Not "make it faster", but make it **controllable** — like the power profiles
of Windows or Zorin: **power saving · balanced · maximum performance**.

| | What | State |
|---|---|---|
| 4.1 | **Clock management (P-states)** — set frequency and voltage through `IA32_PERF_CTL`/`HWP`; the three profiles on top of that. That is the heart of the matter. | **in progress: round K18** (Osum, branch `k18-power`, branched off `c5fe12f`). State on 2026-08-26, 15:32: `kernel/pwr.fi` with 755 lines and `tools/k18/msrprobe.s` with 496 lines in the working tree, not yet checked in. **No measured figures** — and with this item that has to be stressed particularly: a clock management that is not measured on real hardware has shown nothing |
| 4.2 | **Idle states (C-states)** — really sleep when idle instead of spinning; `mwait` instead of a busy loop | in progress (K18) |
| 4.3 | **Turbo** — release or block the highest stage, with an eye on the temperature | in progress (K18) |
| 4.4 | **Battery and power supply** — charge level, charging state, remaining time, mains on/off through ACPI; display and warning | open |
| 4.5 | **Temperature and fans** — read them out, throttle before it gets hot | open |
| 4.6 | **Screen** — brightness, switching off after inactivity | open |
| 4.7 | **Standby** — standby and hibernation (S3/S4). Laborious, because every driver has to play along. | open |
| 4.8 | **Measure instead of guess** — cache for file system blocks, `mmap` instead of copying, swapping, timer resolution. Only sensible once there is enough load to measure. | open |

## 5 — Foreign programs

| | What | State |
|---|---|---|
| 5.1 | **Linux binary compatibility** — statically linked programs through a mapping of the system call numbers. The ELF loader stands, and the POSIX layer was deliberately built with the Linux numbers. That is weeks, not years. | open |
| 5.2 | **Dynamic linking** — a loader for libraries, so that programs not statically bound run as well | open |
| 5.3 | **Hypervisor** — AMD-V (VT-x open), nested page tables, guest machines. With it foreign systems run unchanged; the benefit goes far beyond Windows (isolation, Linux guests, testing Osum in Osum). | **done for AMD-V** (K12: VMCB, NPT, guests, handles from ring 3). VT-x open — not checkable on the measuring machine (AMD EPYC without /dev/kvm) |
| 5.4 | **Windows programs** — through 5.3 in a guest system. A reimplementation of the Windows interfaces of our own (the Wine way) is explicitly NOT planned: Wine has been working on it since 1993 and has 783 of about 990 functions for `user32.dll` alone. | later |

## 6 — Software for the system

| | What | State |
|---|---|---|
| 6.1 | **Package management** — the format is designed ([PACKAGING.md](PACKAGING.md)): immutable, content-addressed packages, three data pools, system generations. None of it is built. Network, file system and checksums are already there. | open |
| 6.2 | **Package source** — a place to install from; signatures | open |
| 6.3 | **Browser** — comes into being in the Firn repository: HTML tree building and DOM (94.89 % of the html5lib tests), layout with flexbox (31.72 % of the Web Platform Tests), painting in progress. After that: bind JS to the DOM, HTTP, user interface. | in progress |
| 6.4 | **The Firn compiler runs on Osum itself** — the real touchstone. A system on which you can build your own system is grown up; everything before that is cross-compiling from outside. | **done** (K16). Osum compiled, linked and ran a Firn program on itself, and the result is **byte for byte** the same as what the same compiler makes from the same source on Linux: assembly text **14,909 bytes character-identical**, ELF file **8192 bytes character-identical**. Plus an assembler and linker of its own in Firn, `kernel/user/fas.fi`, 2423 lines — possible because the compiler uses only a small slice of x86-64: measured over 1,533,513 lines of assembly text **58** mnemonics, **96** pairs of mnemonic and operand form, 8 directives, 4 memory expressions, no index register, no scale, no segment. `fas` links **54 of 54** programs of the userland; 16 of them held against `as`+`ld`, 16 times the same output. `.fi` is double-clickable (`kernel/ftype.fi`, `kernel/user/firun.fi`, 242 l.). Measured: `tools/k16/run.sh` **64 checks, 0 failures** |
| 6.4a | **…but `firnc` is not in the OrientOS product yet.** That is the honest separation between "Osum can do it" and "the product has it" | **open, and easy to overlook.** `vendor/osum/hole-osum.sh` builds the 69 programs from `kernel/user/*.fi`; among them are `fas` and `firun`, but **not** `firnc` — that one comes into being in K16 by cross-building Firn's `bin/firnc1.fi` against `kernel/user/user.ld`, and it needs the Firn repository for that. As long as that is not in `hole-osum.sh`, you can assemble a `.s` on a booted OrientOS, but you cannot compile a `.fi` |

## 7 — Other machines

| | What | State |
|---|---|---|
| 7.1 | **The architecture boundary** — x86 details lie everywhere in the kernel instead of in one place. The Rust kernel of this repository HAD this boundary (traits plus a test step that forbade x86 terms outside of `arch/`); it has **not** been ported to Firn, because that would be a rebuild of every module and not a port. The requirement stands in the tree as an uncompiled template: `vorlage/arch_iface.rs`. The real content is not the trait text but the rule. See [KERNELWECHSEL.md](KERNELWECHSEL.md) § 4.1. | open, with a template |
| 7.1b | **aarch64** — Firn already compiles to ARM (296 of 300 cases identical on both machines), the kernel does not. Requires 7.1. | open |
| 7.2 | **The device reality on ARM** — no PCI detection path as on the PC, but a device tree, and every SoC is different. The real effort. | open |
| 7.3 | **The browser as an app on Android** — the fastest way to get the result into your hands. Android is Linux plus Bionic; an APK may contain native libraries. So: compile the browser core to `aarch64-linux-android`, pack it as a `.so`, a thin shell for canvas and touch. Precondition: the browser is finished enough and can be split out as a library — to be kept in mind from the start while building. | open |

## 8 — Security and hardening

| | What | State |
|---|---|---|
| 8.1 | **SMEP/SMAP** — ring 0 executes no user code and touches user data only in the `stac` window. On EVERY core (CR4 is per processor), read back instead of claimed. Counter-checks: `smapraw` gives a #PF with the bit and the byte without it. | **done** (2026-08-26, Osum `kernel/guard.fi`) |
| 8.2 | **Memory randomisation (KASLR)**, guard pages, non-executable stack | open |
| 8.3 | **Signed packages and signed boot** | open |
| 8.4 | **Long-running test** — days instead of minutes, with an eye on fragmentation and leaks | open |

---

## 9 — What the kernel switch left open

The switch to the Osum kernel was completed with the cut on 2026-08-26
([KERNELWECHSEL.md](KERNELWECHSEL.md) § 7). Two items were **deliberately**
not ported, and both stand here so that they do not vanish into a footnote.

| | What | State |
|---|---|---|
| 9.1 | **The architecture boundary** — the same thing as 7.1, from the other side: not "we want ARM", but "the Rust kernel could do something the Firn kernel cannot". Template: `vorlage/arch_iface.rs`, 378 lines, not compiled. | open, with a template |
| 9.2 | **Channels, ports, namespaces, `ProcessSpawn`, memory objects** of the native ABI. The numbers (from 2000 on), the error values and the meanings are ported and are in Osum's `sys.fi`; the objects behind them are missing and answer `NotSupported` (−9). `NotSupported` and not `ENOSYS`: the call EXISTS in this ABI, this kernel just does not offer it. | open |
| 9.3 | **A brand-dependent userland.** Two brands differ only in the name so far. `userland/PROGRAMME.<brand>` would be the next step — the source text would stay the same. | open |
| 9.4 | **A write path to a real disk.** The boot module is a RAM disk; changes do not survive the run. | **half done, and the halves belong apart.** The partition table reader is there (K14, `kernel/part.fi`, MBR and GPT with both CRC32s), FAT32 can write (K14, `kernel/fat.fi`), a second disk can be named to the block layer. What is **missing** is the installation path: OrientOS builds an ISO with a boot module, no program writes a root file system onto a disk, and none of that is measured |

---

## 10 — The pinned kernel, and what had to be corrected in it

OrientOS does not build against the newest Osum but against **one** commit
(`vendor/osum/COMMIT`). On 2026-08-26 the pin moved from `7a53ac3` to
**`c5fe12f`** — four rounds and 32,525 lines in between
(`git diff --stat 7a53ac3 c5fe12f`: 103 files, 32,525 inserted, 267 deleted
lines).

**This commit does not compile.** That is not an aside but the reason why
there has been a patch stack in this repository since 2026-08-26. Two places
got lost in the merge of `k15-ui`:

| file | what is missing | what firnc says |
|---|---|---|
| `kernel/kmain.fi` | ONE closing brace in `mode_of` | `'fn' is only allowed at top level, not inside a function body` — 13 functions end up inside the body of `mode_of` because of it |
| `kernel/sys.fi` | the K15 seam (1800..1806) sits in `k13_call` instead of in `dispatch` | `unknown name 'a4'` — `k13_call` takes a0..a3. And even compiled, the branch would be **dead**: `is_k13` does not enumerate 1800..1806 |

Both are merge losses and not intent: the parent `78f8e86` has the `kmain`
block complete, the parent `fcb5c30` has the seam in `dispatch`. It is
corrected **here** and not in the kernel repository — a moved pin is a
different state and devalues yesterday's measurements. The files lie in
`vendor/osum/patches/`, `hole-osum.sh` applies them while unpacking and
**aborts** when one no longer fits; `tests/step-05-patches.sh` takes each of
them out again on its own and demands that firnc then rejects the kernel.

**Independently confirmed.** The same two defects were found on the same day
by the two running kernel rounds, each on its own: `k17-usb` (commit
`93ad914`, "two merge defects from c5fe12f fixed, nothing builds otherwise")
and `k18-power` (commit `eed1edd`, "main would not compile — two leftovers
of the K15 merge"). Three findings, the same brace. As soon as one of these
branches goes to `main`, the patch no longer fits, the run turns red, and
the file belongs deleted — not adapted.

**And a figure that turned up in the process.** `userland/PROGRAMME` stood
at `bloecke: 6144`. The block map of OFS is ONE block of 512 bytes, so 4096
bits, so at most 4096 blocks per image — up to K15 `mkfs.py` did not check
that and built 6144 without complaint; the upper 2048 blocks were not
addressable. It was never noticed, because the product never needed them: 27
programs, 932,584 bytes, **1822 of 4062 data blocks**. Since K15 `mkfs.py`
refuses 6144, and that is as it should be.

---

## Standing rules

1. **No Linux code, no fork.** Rebuilding from specifications yes, adopting no.
2. **No claim without a measurement.** Every check has a **counter-check** — the same kernel with the property switched off, where the measurement has to break down. A green test without a counter-check does not count.
3. **Nothing is deleted before the replacement demonstrably runs.**
4. **What is missing is stated.** Gaps are named, not written away.
5. **What is deleted stays in the history.** `git rm`, never `rm`. A deleted
   kernel is not forgotten, it is only out of the way — and `./test.sh`
   checks that the history still knows it.

## Related design documents

| file | content |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | layers, architecture boundary, dependencies |
| [PACKAGING.md](PACKAGING.md) | immutable, content-addressed packages, system generations |
| [FILESYSTEM.md](FILESYSTEM.md) | VFS, FAT32, ext4 reading, a file system of its own |
| [LANGUAGE.md](LANGUAGE.md) | Firn: why a language of its own, and what it demands of the kernel |
| [KERNELWECHSEL.md](KERNELWECHSEL.md) | comparison module by module, what is ported and what is outstanding |
| [BRANDING.md](BRANDING.md) | one source text, several products |
