# OrientOS — an operating system from scratch

**OS: `OrientOS` · kernel: `osum` (Firn, [own repo](KERNELWECHSEL.md)) · product: `build/orientos.iso`, boots via BIOS and UEFI**

No Linux fork, no adopted foreign code. The kernel is written in
[Firn](LANGUAGE.md), a language of its own; OrientOS is the system around
it.

> **The kernel switch was completed on 2026-08-26.** The old Rust kernel
> of this repository has been **deleted** — 18,255 lines of Rust, 1,206
> lines of C and 965 lines of Firn that belonged to it, after every open
> item had been worked off. A
> single file stays behind as an uncompiled **template**
> (`vorlage/arch_iface.rs`, with the reasoning in its header). What was
> compared, what is ported, what is still missing and what exactly was
> dropped:
> **[KERNELWECHSEL.md](KERNELWECHSEL.md)**.

## What it can do

**Boot.** Multiboot through Limine, on BIOS **and** UEFI, from the same
image.

**Processes.** Every process has its **own address space**; `fork`,
`execve`, `wait4`. The scheduler preempts by time slice and runs on
**multiple processors** (SMP through ACPI/MADT, APIC, spinlocks).

**Hardware.** PCI detection, APIC and IOAPIC, **NVMe over DMA** (measured
140,799 KiB/s against 5,910 KiB/s with ATA PIO), PS/2 keyboard, serial
port, **framebuffer** 800x600x32 with a text console of 100x37,
**virtio-net**.

**Files.** A file system of its own, **OFS** — superblock, block bitmap,
inodes, directories, direct and indirect blocks, `getdents64`. Writing,
on a RAM disk, ATA or NVMe.

**Programs.** ELF64 loader: the kernel reads `/bin/sh` **off the disk**
and starts it. On top of it a **POSIX layer** with the system call
numbers of Linux (72 calls), a libc in Firn, a **shell** with pipes,
redirection, exit code and job control, and **31 tools** (`ls -l`, `cat`,
`cp`, `mv`, `rm`, `grep`, `sort`, `wc`, `ps`, `kill`, `ping`, `wget`
...).

**What Unix programs take for granted.** Signals with their own handlers
in ring 3 (`sigaction`, `sigreturn`), terminals and pseudo terminals with
a line discipline and process groups (Ctrl-C really does terminate),
real-time clock and monotonic clock, `getrandom`.

**Network.** A TCP/IP stack of its own in Firn — Ethernet, ARP, IPv4,
ICMP, UDP, TCP with a state machine — under virtio-net, with sockets in
ring 3. Measured against the real Linux kernel: 6432 KiB/s over a clean
line, and at 20 % artificial packet loss it keeps transmitting
correctly.

**The kernel protects itself from the userland.** **SMEP and SMAP** stand
in CR4 as soon as the processor reports them — on **every** core, because
CR4 is per processor. Ring 0 executes no user code with them, and it
touches user data only in the `stac` window. What is measured is not the
claim but the register read back: `guard: cr4=0x300020 smep=1 smap=1`.

**An ISO with a userland.** The loader places an OFS file system next to
the kernel as a **boot module**; the kernel recomputes its CRC32 and
mounts it as the root disk. That is why `/bin/sh` runs in an ISO that has
no disk at all. What is inside it is assembled by **this** repository
(`userland/PROGRAMME`).

**Rights.** A **capability-based ABI**: every access goes through a
handle with rights — slot and generation, ten rights bits, eight object
kinds. No inheriting, no ambient authority.

**A user interface.** A pointing device on the second port of the
keyboard controller, a **window server** with stacking order, input focus
and damage tracking, and a **TrueType reader together with a rasteriser
with antialiasing**, entirely in Firn. Two windows stand on the screen: a
terminal in which `/bin/sh` runs off the disk, and a second one that can
be clicked, moved and closed. Measured against real screenshots into
which real mouse movements, clicks and key presses were fed through the
QEMU monitor — and the text in them not against a surface but **per
character** against a second, independent rasterisation of the same
outline.

**You can work on it.** A full-screen **editor** (`/bin/edit`) in ring 3
through the raw terminal mode, and the toolbox to go with it: `find`,
`sed`, `diff`, `patch`, `tar`, `gzip`, `gunzip`, `xargs`, `du`, `top`,
`mount`, `umount`, `tee`, `cut`, `tr`, `seq`, `env`, `which`,
`basename`, `dirname`. The shell has become a language — `if`/`elif`/
`else`, `while`, `until`, `for`, `case`, functions,
`return`/`break`/`continue`/`shift`, `test` and `[`. Every tool is held
against its GNU counterpart; `tar` and `gzip` work with the host's real
`tar` and `gzip` in **both directions**.

**A host for foreign processors.** A **hypervisor** in Firn: control
block, guest state, intercept bits, entry, exit, exit reason, **nested
page tables**. Guest machines are reachable from ring 3 — through handles
with rights, not through small numbers: whoever passes on a copy without
`R_MANAGE` passes on the right to run and not the right to kill.
Implemented is **AMD-V**, not Intel VT-x, and the reason is in
`docs/ROUNDK12.md`: the measuring machine is an AMD EPYC without
`/dev/kvm`, and QEMU's TCG demonstrably does not offer `vmx` — a VMX host
would not have been measurable here, and this project measures.

**Brands.** One source text, several products: `brands/*.toml` describes
everything that is exchangeable, and the source text knows none of these
values (`./build.sh --brand <name>`).

## What it cannot do

- **No desktop.** There are windows, a mouse and fonts, but no
  applications in them apart from the terminal and a test window: no file
  manager, no themes, no wallpapers, no settings.
- **No users.** No `uid`/`gid`, no `chmod`, no login — everything runs
  with all rights.
- **No `init`, no services.** The kernel starts the shell directly.
- **No VFS.** Only OFS; FAT32, ext4 and other file systems are missing,
  and foreign disks are therefore unreadable.
- **No package management.** The format is designed
  ([PACKAGING.md](PACKAGING.md)), but not built.
- **No swapping**, no `mmap` on files, **no USB**, **no sound**, **no
  power management** (ACPI can only switch off so far: no P-states, no
  profiles, no battery level, no brightness, no standby).
- **No off-the-shelf guest systems.** The hypervisor runs small guests of
  its own and measures them; it has never booted an unmodified Linux, let
  alone Windows, and the virtual devices for that (disk, network, screen)
  are missing.
- **Only x86-64.** Firn itself compiles to aarch64 as well, the kernel
  does not.
- **No foreign programs.** What runs is what was written in Firn and
  compiled for Osum. Linux binaries do not run; the way there would be a
  mapping of the Linux system call numbers onto its own.

## Key figures

| | |
|---|---|
| Kernel (Osum, pinned commit) | 34,459 lines of Firn + 1,884 lines of assembly |
| Rust in the kernel | **0** |
| Rust in THIS repository | **0** (+ 378 lines of template, not compiled) |
| C in this repository (apart from the bootloader) | **0** |
| External libraries in the kernel | **0** |
| System calls | 79 declared, 74 in the dispatcher |
| Programs in ring 3 | **52** (kernel/user/*.fi with their own `_start`; plus two libraries without) |
| Checks in the kernel acceptance suite (Osum `./test.sh`) | **1486**, in 18 sections, 0 failures |
| Checks in the system acceptance suite (here, `./test.sh`) | **151**, in 11 steps, each with a counter-check |

All of it verifiable: `./test.sh` builds, boots and measures. Every check
has a **counter-check** — the same kernel with the property switched off,
where the measurement has to break down. A green test without a
counter-check does not count in this project.

## Quick start

```sh
# 1. prerequisites (Debian/Ubuntu)
sudo apt install qemu-system-x86 xorriso mtools ovmf python3 binutils build-essential git
#    ...plus rustc/cargo: Osum's Firn compiler (stage 0) is written in Rust.
#    No Rust is compiled in THIS repository.

# 2. build the product and start it
./build.sh          # kernel from vendor/osum + userland module -> build/orientos.iso
./run-osum.sh       # starts it through SeaBIOS, serial output in the terminal
./run-osum.sh --uefi                     # the same image through OVMF
./run-osum.sh --script 'ls /bin;exit'    # hand in a shell script

# 3. check everything (real QEMU boots, BIOS and UEFI, shell in ring 3)
./test.sh
```

### Scripts

| command | effect |
|---|---|
| `./build.sh` | **product ISO** (`build/orientos.iso`): fetches the pinned Osum kernel **and** its programs, assembles an OFS file system from `userland/PROGRAMME`, computes its CRC32 and packs both with Limine (Multiboot, BIOS + UEFI) |
| `./build.sh --brand xoffi` | second brand, same source text ([BRANDING.md](BRANDING.md)) |
| `./build.sh --cmdline "…"` | a command line of your own for the kernel |
| `./build.sh --ohne-userland` | **counter-check:** ISO without the boot module |
| `./build.sh --kaputte-summe` | **counter-check:** wrong CRC32 in the call — the kernel has to leave the module alone |
| `./run-osum.sh [--uefi] [--script "…"]` | starts the product ISO; return value 0 when the kernel terminates cleanly with 21 |
| `./run-osum.sh --cpu qemu64` | **counter-check:** a processor without SMEP/SMAP |
| `./test.sh` | the acceptance suite of the whole system |
| `./rename.sh <kernel> <os>` | renames kernel and OS across the whole tree ([RENAME.md](RENAME.md)) |
| `./vendor/osum/hole-osum.sh` | builds kernel + programs from `vendor/osum/COMMIT` |

---

## What the product really does

A real boot from 2026-08-26, copied **unchanged** from `./run-osum.sh
--script 'ls /bin;uname;df;exit'`; only the `elf:` lines of the loader
are left out, because there are four of them per started program.

```text
firn kernel r62, profile kernel
mb: flags=0x126d  cmd=osum nokbd nosched noproc nofs noring3 modfs modcrc=5d92f65d script=ls /bin;uname;df;exit
idt: 48 gates, vector 8 IST
pic: master 0x20, slave 0x28
pit: 100 Hz, divisor 11931
kernel end: 0x225040
mmap: 9 entries, 523771 KiB usable, top=0x1ffdf000
frames: 131072 covered, 129721 free
heap: 0x552000  size=262144
frame test: ok
heap test: ok
mod: base=0x252000  bytes=3145728  blocks=6144  crc=0x5d92f65d  want=0x5d92f65d  ok=1
pci: 00:00.0 8086:29c0 class=06:00:00 host-bridge
pci: 00:01.0 1234:1111 class=03:00:00 vga  bar0=0xfd000000/0x1000000
pci: 00:02.0 8086:10d3 class=02:00:00 network  bar0=0xfeb80000/0x20000  irq=11  msi  msix
pci: 00:1f.2 8086:2922 class=01:06:01 ahci  bar5=0xfebd5000/0x1000  irq=10  msi
pci: devices=6
apic: id=0  hz=62537300  ioapic=24  tick=100  window=2
guard: cr4=0x300020  smep=1  smap=1  cpu=1/1
ticks: 20  traps=20
osum: from module 0x252000  blocks=6144
osum: mount=1
osum: bin .:2 ..:2 sh:1 ls:1 cat:1 cp:1 mv:1 rm:1 mkdir:1 rmdir:1 touch:1 head:1
      tail:1 wc:1 grep:1 sort:1 uniq:1 true:1 false:1 sleep:1 ps:1 kill:1 uname:1
      date:1 df:1 hello:1 ping:1 wget:1
sh: ready, osum
osum$ ls /bin
./ ../ sh ls cat cp mv rm mkdir rmdir touch head tail wc grep sort uniq true false
sleep ps kill uname date df hello ping wget
osum$ uname
osum
osum$ df
blocks total=6144 free=2442 used=3702 size=512
inodes total=128 used=31
osum$ exit
sh: bye
osum: kstack deepest=13520 of 16384
osum: sh exit=0
osum: frames_free=129653 of 129653
osum: syscalls=144 forks=0 execves=0 pipes=0 maps=0 opens=1
smp: online=1 of 1  failed=0
guard: aps=0  windows=253
mod: recheck crc=0x5d92f65d  same=1
kernel: done
--- Beendigungscode 21 ---
```

**The four lines that matter:**

* `mod: … crc=0x5d92f65d want=0x5d92f65d ok=1` — the kernel found the
  boot module and **recomputed for itself** that it arrived intact. The
  host wrote the same checksum into the command line.
* `osum: from module … blocks=6144` — the **root disk is the module**. An
  ISO has no disk; what a Multiboot loader can pass on is a module. That
  is why this product has a userland at all.
* `guard: cr4=0x300020 smep=1 smap=1` — bit 20 and bit 21 in CR4, **read
  back** from the register. Ring 0 executes no user code and touches user
  data only in the `stac` window (`windows=253` says how often it
  opened).
* `mod: recheck … same=1` — after the whole run the module is still the
  same. Had the frame allocator given its memory away, a different
  checksum would stand here.

---

## How the two repositories fit together

```
osum (own repo, Firn)                    orientos (this repo)
├── kernel/*.fi   the kernel     ──┐     ├── vendor/osum/COMMIT   ← pinned
├── kernel/user/  shell, 25 tools  │     ├── brands/*.toml        brands
├── lib/libc/     libc in Firn     └──►  ├── userland/PROGRAMME   what goes into the ISO
├── vendor/firn/COMMIT (its own          ├── userland/dateien/    what OrientOS contributes
│                       compiler)        ├── PACKAGING.md         packages
└── test.sh       15 sections,           ├── ASSISTENT.md         interface
                  >1100 checks           ├── build.sh             kernel + userland → ISO
                                         └── test.sh              acceptance of the system
```

**Only the hash is checked in.** No kernel image, no program, no
`mkfs.py` — `vendor/osum/hole-osum.sh` builds everything from the named
commit, and `test.sh` checks that nothing built really lies in the
repository. The two repositories pin **different** Firn commits, and it
is meant to stay that way: two projects, two pins, no quiet mixing.

---

## One source tree, two products

`brands/*.toml` describes everything about the product that is
exchangeable; `brands/STANDARD` names the default brand. The kernel is
called `osum` in **every** brand — the way NT is called NT in every
Windows edition.

```sh
./build.sh                 # build/orientos.iso, boot menu "/OrientOS"
./build.sh --brand xoffi   # build/xoffi.iso,    boot menu "/XoffiOS"
```

`test.sh` really builds the second brand and reads back that its boot
menu carries the other name — and that an **invented** brand aborts
instead of quietly falling back to the default brand. Details:
[BRANDING.md](BRANDING.md).

---

## What is still missing

Honestly and completely in [KERNELWECHSEL.md § 4](KERNELWECHSEL.md) and
[ROADMAP.md](ROADMAP.md). The short version:

* **No architecture boundary.** Osum is x86-64 only, although Firn can do
  aarch64 as well. The Rust kernel had this boundary as traits together
  with a test step that enforced it; that is **not** ported, because it
  would be a rebuild of every module. The template lies in the tree as
  `vorlage/arch_iface.rs` — not built, with the reasoning in its header.
* **Channels, ports, namespaces, `ProcessSpawn`, memory objects** of the
  native ABI answer `NotSupported` (−9). What is ported is the handle
  model underneath, not the objects that hang off it.
* **Of the distribution model** ([PACKAGING.md](PACKAGING.md)) nothing is
  built: the root file system of the ISO is a fixed list, not a store
  with content hashes.
* **The module is a RAM disk.** Changes do not survive the run. For a
  system that installs itself, a write path to a real disk and a
  partition table reader are missing.
* No window system, no name resolution, no USB, no SATA/AHCI, no
  journal, no users/permissions, no dynamic linking.
* **Testing happens in QEMU**, not on real hardware.

---

## Documentation

| file | content |
|---|---|
| [KERNELWECHSEL.md](KERNELWECHSEL.md) | **What holds.** The comparison module by module, what was ported, what is open, what was deleted |
| [ARCHITECTURE.md](ARCHITECTURE.md) | the architecture of the system: who does what, and where the boundary lies |
| [ROADMAP.md](ROADMAP.md) | where this is going, sorted by dependency |
| [LANGUAGE.md](LANGUAGE.md) | the logbook of the friction points between Rust and a kernel — history, and still a list of requirements for Firn |
| [BRANDING.md](BRANDING.md) · [RENAME.md](RENAME.md) · [NAMEN.md](NAMEN.md) | brands, renaming, finding names |
| [PACKAGING.md](PACKAGING.md) · [FILESYSTEM.md](FILESYSTEM.md) | distribution model and file system design |
| [ASSISTENT.md](ASSISTENT.md) | the interface for an assistant (instead of screenshots) |
| [NETZWERK.md](NETZWERK.md) | redirection layer, kernel/userspace boundary, Tor/WireGuard |
| [userland/README.md](userland/README.md) | how the userland gets into the ISO and what is to become of it |
| [tests/README.md](tests/README.md) | how the acceptance suite is put together |
