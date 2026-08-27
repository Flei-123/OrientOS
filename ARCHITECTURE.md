# Architecture of OrientOS

This document describes **the built state**, not intentions. Everything
that stands here is verifiable in the code or in the test run; where
something is still missing, it says so explicitly as "not yet".

> **This version is from 2026-08-26 and describes something different
> from the previous one.** Until then, ten sections about a Rust kernel
> stood here: layers, arch traits, frame allocator, scheduler, ring 3,
> two ABIs. That kernel is **deleted**
> ([KERNELWECHSEL.md](KERNELWECHSEL.md) § 7). It is not gone without a
> trace — the old version of this document is complete in the history:
>
> ```sh
> git show 91182a0:ARCHITECTURE.md
> ```
>
> What of it lives on in substance is in section 6.

---

## 0. The division: kernel and system

**OrientOS does not write its kernel.**

| | repository | language | role |
|---|---|---|---|
| **Osum** | its own repository, embedded here as `vendor/osum` (pinned through `vendor/osum/COMMIT`) | Firn + assembly | **The kernel and the programs on it.** Boot, memory, address spaces, processes, file system, drivers, network, signals, screen, both ABIs, shell and 25 tools. |
| **OrientOS** | this repository | shell, TOML, Python, documentation | **The system around it.** Brands, assembly of the userland, package format, assistant interface, the packing into a bootable product and the **acceptance suite of the whole system**. |

The rule between them is the same one this repository already uses for
the Firn compiler: **a pinned commit, no checked-in image, no copying
without provenance.** `vendor/osum/hole-osum.sh` builds the kernel **and**
the programs from the named commit — with **Osum's own** Firn compiler,
not with this repository's.

`test.sh` step 1 checks that: the commit is a hash, the built image comes
from it, and there is neither a kernel image nor a program nor `mkfs.py`
in the repository.

---

## 1. What this repository does, in four steps

```
./build.sh
  │
  │  1. PROVENANCE
  ├─ vendor/osum/hole-osum.sh
  │     git archive <COMMIT> → /tmp/osum-pin-<hash>
  │     build Osum's own Firn compiler there
  │     there tools/build-kernel.sh  → vendor/osum/osum.mb
  │     there every kernel/user/*.fi → vendor/osum/bin/*
  │
  │  2. ASSEMBLY
  ├─ userland/PROGRAMME   which programs, which directories,
  │                       which files from userland/dateien/
  ├─ vendor/osum/mkfs.py  → build/<slug>-userland.img  (OFS, 512-B blocks)
  ├─ zlib.crc32           → the checksum the kernel will recompute
  │
  │  3. BRAND
  ├─ brand.sh + brands/<brand>.toml   → OS_NAME, SLUG
  │
  │  4. PRODUCT
  ├─ build/isoroot/boot/osum                     the kernel
  ├─ build/isoroot/boot/<slug>-userland.img      the module
  ├─ build/isoroot/boot/limine/limine.conf       protocol: multiboot1
  │                                              path, module_path,
  │                                              cmdline "… modfs modcrc=…"
  └─ xorriso + limine bios-install → build/<slug>.iso
```

No compiler runs in this repository while that happens. That is
deliberate: everything that gets compiled belongs to Osum, and Osum pins
its own Firn commit for it.

---

## 2. The boot path, and why it goes through Multiboot

**Limine with `protocol: multiboot1`**, not with the Limine protocol.
The reason lies with the kernel: Osum boots through Multiboot 1
(`kernel/boot.s`), and a loader has to start the kernel the way the
kernel expects.

The path has two places where it used to snag, and both are measured:

**a) UEFI.** A Multiboot loader under UEFI has to hand the kernel a
screen before it leaves the firmware. If the Multiboot header contains no
screen request, it assumes **text mode** — and that does not exist under
UEFI:

```
PANIC: multiboot1: Cannot use text mode with UEFI.
```

Since commit `c4427fa`, Osum's header demands a **linear framebuffer**
with **flag bit 2** and `mode_type = 0`. With that the same image boots
through SeaBIOS **and** through OVMF. `tests/step-20-produkt.sh` reads
the header out of the built image and checks the checksum, bit 2 and
`mode_type`; `tests/step-30-boot.sh` really drives both paths.

**b) The boot module.** An ISO has no disk. What a Multiboot loader can
pass on is a **module** — `module_path:` in `limine.conf`, and at the
kernel it arrives as an entry in the module table (Multiboot 1,
information structure, flag bit 3). Osum's `kernel/bootmod.fi` reads it,
computes **CRC32** over the whole module, compares it with `modcrc=` on
the command line and hands it to `blk.ram_init`. For the file system
nothing changes in the process: the same 512 bytes per block, only out of
a different device.

**The proof that these were two real boots** and not the same one twice:
the memory map comes from the firmware, and SeaBIOS and OVMF deliver
different numbers of entries. `tests/step-30-boot.sh` compares the two
numbers and fails when they are equal.

---

## 3. The command line is the interface

What the kernel does is on its command line, and that line is written
into `limine.conf` **at build time** — not at start time. That is why
`run-osum.sh` builds first.

| word | effect (Osum) |
|---|---|
| `osum` | start `/bin/sh` from the root disk |
| `modfs` | take the boot module as the root disk |
| `modcrc=<8 hex>` | the demanded checksum of the module |
| `script=…` | a script that goes through the same line discipline as the keyboard (`;` is a line break) |
| `caps` | the capability round in ring 3 |
| `nosmep`, `nosmap` | counter-checks: leave out the respective protection bit |
| `smapraw`, `smepraw` | counter-checks: an access that **has** to fail |
| `nokbd`, `nosched`, `noproc`, `nofs`, `noring3` | leave out stages of the kernel |

**One ordering is not arbitrary:** `script=` reads to the end of the
line. A kernel word after it would be a shell command. `build.sh`
therefore pushes `modfs modcrc=…` **before** `script=` — exactly that
went wrong once and stood in the log as `osum$ exit modfs modcrc=…`.

---

## 4. Brands: one source tree, two products

The only role that **cannot** be pushed into the kernel.

* `brands/<brand>.toml` — `os-name`, `slug`, `publisher`, `web`, `feed`.
* `brands/STANDARD` — which brand applies when none is named. Until the
  kernel switch this was in `kernel/Cargo.toml`; that no longer exists.
* `brand.sh` — resolves it and **aborts** on an unknown name instead of
  quietly falling back to the default brand.
* The kernel is called `osum` in every brand. A brand that wants it
  otherwise sets `kernel-name` — that then only renames the file in the
  image.

What the test run measures of this is not the existence of the files but
the **effect**: the second brand is built, a second ISO comes into being
under a different name, its boot menu carries the other product name, and
an invented brand aborts.

---

## 5. The acceptance suite

`test.sh` gathers the steps from `tests/step-*.sh`. Every single check is
counted and printed individually; a summary number on its own would be
too easy to get green.

| step | what it measures |
|---|---|
| provenance | the commit is a hash, the image comes from it, nothing built lies in the repository |
| no Rust | 0 lines of Rust apart from the template, no `Cargo.toml`, the history still has everything |
| brands | two products, really built; an invented brand aborts |
| product | the ISO contains exactly the built image and exactly the built module, `limine.conf` names the right CRC32, the Multiboot header is right |
| boot | BIOS **and** UEFI, different memory maps |
| userland | `/bin/sh` out of the module, `ls`/`uname`/`cat`/`wc` in ring 3, no frame is left behind, the module is unchanged at the end |
| counter-checks | without the module and with a wrong checksum **no** shell runs |
| capabilities | 18 checks from ring 3, read back individually; without `caps` nobody reports anything |
| protection bits | CR4 read back; `smapraw` has to give a `#PF`, with `nosmap` it must not |
| ELF corpus | 53 broken ELF headers through the loader, in **one** boot, the kernel survives all of them |
| documentation | every path named exists; what is open stands there as open |

**Every check has a counter-check.** A property without a counter-check
is a claim.

---

## 6. What lives on from the Rust kernel in substance

Four things, and they are the reason why the switch was not a loss:

1. **The capability model.** Handle = slot + generation + per-process
   random value, 10 rights bits, 8 object kinds, 9 error values — the
   same numbers, in Firn: Osum `kernel/cap.fi`. The difference from POSIX
   in one line: a valid handle without the right → `RightsDenied` (−2), a
   forged one → `BadHandle` (−1). POSIX has only `-EBADF` for both.
2. **SMEP and SMAP.** Osum `kernel/guard.fi`, ported from
   `arch/x86_64/user.rs`. Both bits in CR4, on every core, with the
   `stac` window in exactly four places.
3. **Boot modules with a checksum.** Osum `kernel/bootmod.fi`, ported
   from `kcore/initramfs.rs` — not the archive format but the point: what
   comes in from outside is recomputed before it is used.
4. **The ELF test corpus.** `tests/firn-elf/faelle/` — 53 hand-built
   headers, each with exactly one defect. The code that checked them is
   gone; the cases now run against Osum's loader, in the product ISO,
   from ring 3.

**What does not live on** and why is in
[KERNELWECHSEL.md § 4](KERNELWECHSEL.md): the architecture boundary
(stays as `vorlage/arch_iface.rs`, not built) and the objects of the
native ABI (channels, ports, namespaces — `NotSupported`).

---

## 7. What is not there yet

* **No architecture boundary in the kernel.** Osum is x86-64 only,
  although Firn can do aarch64 as well. The requirement is in
  `vorlage/arch_iface.rs`; the real content is not the trait text but the
  rule: no x86 term outside of `arch/`, and a test step that enforces it.
* **No package format in the product.** The root file system is a fixed
  list (`userland/PROGRAMME`), not a store with content hashes. The way
  there is in [PACKAGING.md](PACKAGING.md) and
  [userland/README.md](userland/README.md).
* **No brand-dependent userlands.** Two brands differ only in the name so
  far. `userland/PROGRAMME.<brand>` is the next step.
* **No write path to a real disk.** The module is a RAM disk; changes do
  not survive the run. Osum can do ATA and NVMe, but there is no
  partition table reader and no installation path.
* **No assistant.** [ASSISTENT.md](ASSISTENT.md) describes the interface;
  none of it is built.
* **Testing happens in QEMU**, not on real hardware.
