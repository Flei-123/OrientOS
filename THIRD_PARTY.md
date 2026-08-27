# THIRD_PARTY.md -- OrientOS (the system and the package manager)

What in this repository is NOT written here. Round INVENTORY,
27 August 2026.

**Base of every number in this file:** branch `plan2`, commit `9fab13f`.
`plan2` and not `main`, because `pkg/` -- the package manager, and the
subject of half this file -- does not exist on `main` yet. Counted with
`git ls-tree -r plan2` and `wc -l`.

**The one distinction that matters** is marked on every entry:

* **RUNTIME** -- the foreign part is on the ISO and executes on the machine.
  These are the only entries that limit independence.
* **BUILD** -- needed to produce the ISO, gone afterwards.
* **TEST** -- needed only to measure. Never shipped.

The overview across all four repositories is the companion file
`THIRD_PARTY_OVERVIEW.md` in this directory.

---

## 0. Summary in numbers

OrientOS compiles nothing. Since the kernel change (`KERNELWECHSEL.md`,
26 August 2026) `build.sh` **assembles**: it fetches the pinned Osum kernel,
builds a root filesystem out of the pinned userland, and packs both into an
ISO with Limine.

| | files | lines / octets |
|---|---:|---|
| Python here (`pkg/`, `assets/marke/logo_build.py`, `tests/firn-elf/faelle.py`) | 6 | 4,493 lines |
| shell here (`build.sh`, `test.sh`, `tests/step-*.sh`, ...) | 21 | 4,085 lines |
| documentation | 29 | 7,252 lines |
| **foreign code, tracked** | **16** | **4,610,523 octets (Limine)** |
| **foreign code that boots the machine** | **4** | **3,437,860 octets** |

**The whole third-party surface of this repository is Limine.** There is no
other foreign source, no foreign binary, and -- this is the answer to the
question the round asked -- **no required foreign Python package**.

### What to replace first

1. **Write a `LICENSE` file.** Section 6. Costs five minutes, and until it
   exists the repository is legally unusable by anyone, including its author
   later.
2. **Ship Limine's licence on the ISO.** Section 2. Limine is 2-clause BSD
   and the binary-form clause requires the notice to travel with the binary.
   `build.sh:239-242` copies four Limine binaries into the ISO root and does
   not copy `vendor/limine/LICENSE`. One `cp`.
3. **Then stop.** Replacing Limine itself means writing a BIOS and a UEFI
   bootloader, which is a project of its own size and buys independence only
   for the first 200 ms of the boot. Worth doing one day, worth doing after
   everything above the bootloader is own work -- not before.
4. `xorriso`, `qemu`, `OVMF`, `python3`, `mtools`, `binutils` are development
   tools. Do not replace.

---

## 1. Rust crates

**None.** There is no `Cargo.toml` in this repository. `build.sh:5-8` records
why: "Bis zum Kernelwechsel hat es einen Rust-Kernel uebersetzt (cargo,
build-std, eigenes Target ...). Dieser Kernel ist geloescht."

`README.md:151-152` says the same to the reader: rustc/cargo are still needed
on the machine because Osum's Firn compiler (stage 0) is a Rust program --
"In DIESEM Repo wird kein Rust uebersetzt."

**Stage:** BUILD, and only indirectly, through the Osum repository.

Note: `RUN.md:31-33` still tells the reader to install a Rust nightly
toolchain with `rust-src` and `llvm-tools`, and `RUN.md:36` still mentions
`nasm`. Both are leftovers from before the kernel change and are no longer
true. **Stale documentation, worth fixing.**

---

## 2. Limine -- the bootloader. The one foreign thing that runs.

* **What / where:** `vendor/limine/`, **16 tracked files, 4,610,523
  octets**. (`limine` and `limine.exe`, the host tools, are built locally and
  are excluded by `vendor/limine/.gitignore`.)

  | file | octets | role |
  |---|---:|---|
  | `limine-uefi-cd.bin` | 2,949,120 | El Torito UEFI boot image -- **on the ISO** |
  | `BOOTX64.EFI` | 253,952 | UEFI loader at `/EFI/BOOT/` -- **on the ISO** |
  | `limine-bios.sys` | 212,260 | BIOS stage 2 -- **on the ISO** |
  | `limine-bios-cd.bin` | 22,528 | El Torito BIOS boot image -- **on the ISO** |
  | `BOOTAA64.EFI` | 229,376 | AArch64, unused today |
  | `BOOTIA32.EFI` | 258,048 | ia32, unused today |
  | `BOOTLOONGARCH64.EFI` | 245,760 | unused today |
  | `BOOTRISCV64.EFI` | 212,992 | unused today |
  | `limine.c` | 49,092 | source of the host install tool |
  | `limine.h` | 20,093 | the Limine boot protocol header |
  | `limine-bios-hdd.h` | 120,366 | the BIOS MBR stage as a C array |
  | `limine-bios-pxe.bin` | 18,866 | network boot, unused today |
  | `install-sh`, `Makefile`, `LICENSE`, `.gitignore` | 38,070 | |

  **Shipped onto the ISO: four files, 3,437,860 octets.** They execute on
  every single boot, before one line of Osum runs.

* **Origin:** the Limine bootloader, `github.com/limine-bootloader/limine`.
  The tree carries **prebuilt binaries**, not a source checkout -- files
  dated 13 August 2026. `RUN.md:40`: "Limine liegt vorgebaut in
  `vendor/limine/`." The exact upstream version/tag is **not recorded
  anywhere in this repository** -- there is no `COMMIT` file, no version
  string in a manifest, and `limine.h` carries no version macro. See
  section 7.

* **Licence:** **present** -- `vendor/limine/LICENSE`, 22 lines,
  "Copyright (C) 2019-2025 Mintsuki and contributors", a **2-clause BSD**
  licence (redistribution in source and in binary form, two conditions and
  the standard disclaimer).

  **Finding: the ISO does not carry the notice.** Clause 2 requires that
  redistributions in binary form "reproduce the above copyright notice ... in
  the documentation and/or other materials provided with the distribution".
  `build.sh:239-242` copies `limine-bios.sys`, `limine-bios-cd.bin`,
  `limine-uefi-cd.bin` and `BOOTX64.EFI` into `build/isoroot/` and copies
  nothing else. `build/<slug>.iso` therefore ships 3.4 MB of BSD-licensed
  binary without the notice that licence asks for. It is a one-line fix in
  `build.sh` and it should be made.

  Second point, smaller: this repository has no `LICENSE` of its own
  (section 6), so a reader of `vendor/limine/LICENSE` currently sees the only
  licence in the tree and could reasonably conclude it covers everything.

* **What it is for:** `build.sh:70`, `237-255` and `ARCHITECTURE.md:67-70`
  describe the whole job:
  * writing `boot/limine/limine.conf` with `protocol: multiboot1`,
  * booting the Osum image over **both** BIOS (SeaBIOS) and **UEFI** from the
    same ISO -- `tests/step-30-boot.sh:13` measures exactly that,
  * handing the OFS root filesystem to the kernel as a **boot module**
    (`module_path:` in `limine.conf`, `ARCHITECTURE.md:104`). This is what
    gives an ISO a userland at all: an ISO has no disk, and what a Multiboot
    loader can put next to the kernel is a module. Osum's round K10 takes it,
    checks its CRC32 and mounts it as root (`kernel/bootmod.fi` over there).
  * `vendor/limine/limine bios-install build/<slug>.iso` (`build.sh:255`) --
    the host tool that writes the BIOS stage into the ISO. Built on demand by
    `make -s -C vendor/limine` (`build.sh:252-254`). **That one is BUILD
    only.**

* **Cost of replacing, and whether it makes sense:** replacing Limine means
  writing two bootloaders, not one -- a BIOS MBR/stage-2 chain in 16-bit and
  32-bit real/protected mode, and a UEFI application that speaks the UEFI
  boot services, gets the memory map, sets a framebuffer through GOP and
  exits boot services cleanly. Plus the El Torito plumbing so one ISO boots
  both ways, plus module loading. That is a project of its own, and every
  bug in it looks like a kernel bug.

  **Is it worth it?** Not now, and the reason is proportion: Limine runs for
  a fraction of a second and then never again, and the parts of the system
  that are actually used -- kernel, filesystem, network stack, window server,
  shell, browser -- are already entirely own work. The honest ranking puts
  the DejaVu glyph outlines in Osum (`osum/THIRD_PARTY.md` section 4) ahead
  of this, because those are foreign material that is on screen the whole
  time the machine is running.

  **It is worth it eventually**, and there is a clear trigger for it: the day
  "no foreign code executes on this machine" becomes a stated goal rather
  than a direction, Limine is the last thing standing between the firmware
  and Osum.

* **Stage:** **RUNTIME** for the four ISO files, BUILD for the host tool.

---

## 3. Python -- and the Ed25519 question

`pkg/` is 4,032 lines of Python: `opk.py` 3,648, `recipes.py` 180,
`osym.py` 124, `mkfs-spec.py` 80. Plus `assets/marke/logo_build.py` (117) and
`tests/firn-elf/faelle.py`.

**Everything is standard library** -- `argparse`, `hashlib`, `os`, `shutil`,
`struct`, `sys`, `json` -- **with exactly one exception.**

### 3.1 `cryptography` -- optional, and deliberately so

* **What / where:** `pkg/opk.py:222-236`, the whole of function
  `ed25519_verify_bibliothek`:

  ```python
  def ed25519_verify_bibliothek(pk, msg, sig):
      """Die zweite Meinung. Gibt None, wenn der Wirt keine Bibliothek hat."""
      try:
          from cryptography.hazmat.primitives.asymmetric.ed25519 \
              import Ed25519PublicKey
          from cryptography.exceptions import InvalidSignature
      except Exception:
          return None
      ...
  ```

  That is the only place in the repository where a non-standard-library
  Python package is named. The import sits **inside** the function and
  **inside** a `try`/`except` that returns `None` when the package is
  missing.

* **The other implementation, in the same file:** `pkg/opk.py:115-217` is a
  complete **Ed25519 after RFC 8032 written here** -- `_xrecover`,
  `_edwards`, `_scalarmult`, `_encodepoint`, `_decodepoint`,
  `ed25519_public`, `ed25519_sign`, `ed25519_verify`, on top of
  `hashlib.sha512`. Roughly 100 lines of pure Python integer arithmetic, no
  dependency at all.

* **Is the library required?** **No.** The comment at `pkg/opk.py:115-124`
  states the design: "Signaturen (Roadmap 6.2) sind der Punkt, an dem eine
  einzige Umsetzung am wenigsten wert ist: eine Pruefung, die IMMER 'ja'
  sagt, faellt bei keinem gueltigen Paket auf. Deshalb steht hier eine
  eigene, kurze Umsetzung von Ed25519 (RFC 8032) NEBEN der Bibliothek des
  Wirts."

  The three call sites -- `opk.py:1621-1622`, `1646-1647`, `2354-2357` --
  compute `eigen` and `fremd` and abort **only if they disagree**:
  "opk: the two Ed25519 implementations disagree". `None` from the library
  side is not a disagreement. Verification runs on the own implementation
  alone when the package is absent; the library only ever removes doubt, it
  never grants trust. `PAKETE.md:263-264` and `ROADMAP.md:224` describe the
  same arrangement.

* **Origin and licence:** PyPI package `cryptography` (pyca), which wraps
  OpenSSL/Rust primitives. Upstream it is dual-licensed Apache-2.0 OR
  BSD-3-Clause. **No copy of either text is in this repository, the package
  is not vendored, and no version of it is pinned anywhere.** Since it is
  optional the exposure is small, but the licence is genuinely unverified
  here.

* **What it is for:** a second opinion on Ed25519 signature verification for
  package sources -- and, through that, on the own RFC 8032 code.

* **Cost of replacing:** zero. It is already replaced; the own implementation
  is the primary one. Removing the library would only make the check weaker,
  because the cross-check disappears with it. **Keep it optional. Do not make
  it required. Do not remove it.**

* **Stage:** **BUILD/TEST only**, and optional even there. `opk.py` runs on
  the host; nothing Python is on the ISO. There is no Python in the shipped
  system at all.

### 3.2 Everything else in `pkg/`

`mkfs-spec.py` (80 lines, `sys`/`os`), `osym.py` (124), `recipes.py` (180,
imports `PLAN_TYPES` from `opk`). Standard library only. **BUILD.**

---

## 4. Brand assets -- own work, foreign tool

* **What / where:** `assets/marke/`, 35 tracked files -- 12 SVG, 18 PNG, one
  `.ico`, `osum-vorlage-justin.jpg`, `osum-master.json`, `README.md`, and
  `logo_build.py` (117 lines, `json` and `os` only).
* **Origin:** `assets/marke/README.md:38-41` -- "Grundlage ist Justins
  Vorlage `osum-vorlage-justin.jpg`. Die wurde nicht nachgebaut, sondern mit
  **potrace vektorisiert**."
* **The distinction that matters here:** the input is Justin's own drawing
  and the output is Justin's own artwork. **potrace** is a tool that was run
  over it once -- like running a compiler over your own source. potrace is
  GPL-2.0, it is **not in this repository**, and it is **not needed to build
  the product**: the SVGs and PNGs are checked in finished.
* **Licence of the assets:** own work. Which means they are covered by
  whatever this repository's `LICENSE` says -- and there is none (section 6).
* **Stage:** the brand files are BUILD input (`brand.sh`, `brands/*.toml`);
  potrace is neither BUILD nor TEST, it was used once by hand.

---

## 5. The toolchain

Everything here is BUILD or TEST. `README.md:150` states the list:

```
sudo apt install qemu-system-x86 xorriso mtools ovmf python3 binutils build-essential git
```

| tool | where | what for | licence | stage |
|---|---|---|---|---|
| **xorriso** | `build.sh:244` (`xorriso -as mkisofs -quiet -R -r -J`) | builds the hybrid BIOS+UEFI ISO | GPL-3.0 | BUILD |
| **make + a C compiler** | `build.sh:252-254` (`make -s -C vendor/limine`) | builds Limine's host install tool from `limine.c` | GPL-3.0 (GNU make, GCC) | BUILD, and only on first run |
| **python3** | `build.sh:161`, `183`, `184`; every `tests/step-*.sh` | `pkg/mkfs-spec.py`, `vendor/osum/mkfs.py`, the CRC32 of the image (`zlib.crc32`) | PSF | BUILD |
| **qemu-system-x86_64** | `run-osum.sh`, `tests/step-30-boot.sh`, `step-50-schutz.sh`, `step-60-elf-korpus.sh:90` | boots the product | GPL-2.0 | TEST |
| **OVMF (EDK II)** | `run-osum.sh:101` (`/usr/share/OVMF/OVMF_CODE*.fd`), `tests/step-30-boot.sh:51-56` | the **UEFI firmware** for the UEFI half of the boot test. Absent -> the step is skipped and says so (`step-30-boot.sh:83`) | BSD-2-Clause-Patent | TEST |
| **SeaBIOS** | inside QEMU, `tests/step-30-boot.sh:12` | the BIOS half of the same test | LGPL-3.0 | TEST |
| **qemu-aarch64** | `tests/step-92-arch.sh:57-60` | runs an AArch64 binary to compare behaviour. Absent -> "SKIPPED: no qemu-aarch64 on this host" | GPL-2.0 | TEST |
| **mtools** | `README.md:150` | FAT handling for the UEFI part | GPL-3.0 | BUILD |
| **binutils** | via the Osum/Firn build | `as`, `ld` | GPL-3.0 | BUILD |
| **git** | `vendor/*/hole-*.sh` | fetching the pinned commits | GPL-2.0 | BUILD |

**None of these has a licence file in this repository**, and none needs one:
they are host programs, not redistributed material. Named for completeness.

**Note on OVMF and SeaBIOS:** these are *firmware*, and firmware is exactly
the layer the product does not ship -- a real machine brings its own. They
stand in for it while testing. They are not a dependency of the product and
replacing them would be meaningless.

---

## 6. `vendor/osum/` and `vendor/firn/` -- pinned, NOT third party

Both point at Justin's own repositories. Named here so a `vendor/` directory
is not mistaken for foreign code.

* **`vendor/osum/`** -- tracked: `COMMIT`, `hole-osum.sh`, and three patches
  (`patches/0001-kmain-merge-klammer.patch`,
  `0002-sys-wig-naht-in-dispatch.patch`, `0003-mkfs-load-inodezahl.patch`).
  Everything else it produces -- `osum.mb`, `bin/*`, `mkfs.py`, `apps/*` --
  is built from that commit and is gitignored. `hole-osum.sh` explains the
  arrangement at length: one pinned commit, pulled forward only when
  `./test.sh` is green. **Own code, MIT (Osum's `LICENSE`).**
* **`vendor/firn/`** -- tracked: `COMMIT` and `hole-firnc.sh`. **Own code,
  MIT (Firn's `LICENSE`).** Note that Osum pins its own, different Firn
  commit; `hole-osum.sh` is explicit that the kernel is built with the
  compiler ITS repository names, not this one's. Two projects, two pins, no
  silent mixing.
* **`tests/firn-elf/faelle/*.elf`** -- 53 tracked ELF files. **Generated
  here** by `tests/firn-elf/faelle.py`; they are deliberately malformed ELF
  headers for the loader's error paths. Own work.

---

## 7. This repository has NO LICENCE

**Finding, and the most important one in this file.**

```
$ git ls-tree --name-only main   | grep -iE 'licen|copying|notice'
$ git ls-tree --name-only plan2  | grep -iE 'licen|copying|notice'
$
```

Nothing on either branch. The only licence file anywhere in the tree is
`vendor/limine/LICENSE`, which covers Limine and nothing else.

Firn has one (`firn/LICENSE`, MIT). Osum has one (`osum/LICENSE`, MIT).
OrientOS does not.

**Why that is not a formality.** Without a licence the default applies:
all rights reserved. Nobody may copy, modify or redistribute the repository
-- not a contributor, not a user, and not a future company that wants to put
the code under a different arrangement, because "I wrote it, so I can do what
I like" only holds cleanly while there has never been a second contributor
and never a redistributed build. The moment anyone else touches it, or the
moment an ISO goes out the door, the absence of a licence is the thing that
has to be sorted out retroactively, and retroactive relicensing needs the
agreement of everyone who ever committed.

**The fix, in full:**

1. Add `LICENSE` -- MIT, `Copyright (c) 2026 Justin (Flei123)`, identical to
   `firn/LICENSE` and `osum/LICENSE`, so all three repositories say the same
   thing.
2. Add an `EXCEPTIONS` paragraph naming `vendor/limine/` as 2-clause BSD,
   Copyright (C) 2019-2025 Mintsuki and contributors.
3. Add the `cp vendor/limine/LICENSE build/isoroot/boot/limine/` line to
   `build.sh` (section 2).

Time required: minutes. Effect: the repository becomes usable.

---

## 8. Built here from somebody else's specification

**Not foreign code.** Implementing a written specification is own work. Kept
short here because the substance lives in the Osum and Firn repositories.

| what | where | specification |
|---|---|---|
| The `.opk` package format | `pkg/opk.py`, `PAKETE.md` section 2 | **own design** (the store-by-hash idea is Nix's; the format is not) |
| Ed25519 signing and verification | `pkg/opk.py:115-217` | **RFC 8032** |
| SHA-256 content addressing | `pkg/opk.py` (`hashlib`) | FIPS 180-4 |
| The typed PLAN, generations, rollback | `pkg/opk.py`, `PACKAGING.md` | **own design** |
| The OFS build spec | `pkg/mkfs-spec.py` | own format (Osum's OFS) |
| The OSYM icon format | `pkg/osym.py` | **own design** |
| ELF header validation corpus | `tests/firn-elf/faelle.py` | System V gABI, ELF-64 |
| Multiboot1 handover, boot module | `build.sh`, `limine.conf`, `ARCHITECTURE.md:104` | GNU Multiboot Specification |
| CRC32 over the boot module | `build.sh:184` | ITU-T V.42 / zlib |
| El Torito hybrid ISO | `build.sh:244-255` | El Torito / ISO 9660 (via xorriso) |

The comparison worth drawing: `pkg/opk.py` implements the **idea** of a
content-addressed store with generations, which Nix had first
(`pkg/opk.py:88-96` says so about the truncation to 20 hex digits). Not one
line of Nix is in it.

---

## 9. Open items -- things this round could not settle

1. **Which Limine version this is.** No `COMMIT`, no tag, no version string
   recorded anywhere. The binaries are dated 13 August 2026. Without a pin
   there is no way to fetch the same bootloader again, no way to check it
   against upstream, and no way to know which CVEs apply. **Every other
   foreign dependency in this project family is pinned by hash
   (`vendor/firn/COMMIT`, `vendor/osum/COMMIT`, `vendor/net/BLOBS`,
   `tools/ucd/UCD.sha256` in Firn) -- Limine is the one that is not.** Fix:
   a `vendor/limine/VERSION` with the upstream tag and a `sha256` per file.
2. **The exact SPDX identifier of the Limine licence.** The text in
   `vendor/limine/LICENSE` is the 2-clause BSD wording; whether upstream
   declares it as `BSD-2-Clause` is not verified from this tree.
3. **The licence of `cryptography`** as actually installed on the build host
   -- Apache-2.0 or BSD-3-Clause -- is not recorded here (section 3.1).
4. **`RUN.md` is stale** (section 1): it still requires a Rust nightly with
   `rust-src` and `llvm-tools`, and `nasm`. Neither is used any more.
5. **The brand assets have no licence statement of their own** and will
   inherit whatever `LICENSE` ends up saying (section 4). If the logo is ever
   meant to be a trademark rather than MIT-licensed art, that needs its own
   sentence.
