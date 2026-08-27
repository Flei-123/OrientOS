# THIRD_PARTY_OVERVIEW.md -- all four repositories at once

Round INVENTORY, 27 August 2026. The one page that answers: **what in this
system is not mine, and what would it cost to make it mine.**

Per-repository detail:

| repository | file | branch / commit inspected |
|---|---|---|
| Firn (language, compiler, standard library, Certus) | `firn/THIRD_PARTY.md` | `main`, `ce28104bf` |
| Osum (kernel) | `osum/THIRD_PARTY.md` | `main`, `3389fbd` |
| OrientOS (system, packaging) | `orientos/THIRD_PARTY.md` | `plan2`, `9fab13f` |
| orientos-browser (requirements only) | `orientos-browser/THIRD_PARTY.md` | no git repository |

Every claim in those four files carries a path and a line number or an octet
count. Anything that could not be proven is in an "open items" section and
is neither counted as own nor as foreign.

---

## 1. The numbers

### 1.1 Own code

Regular files only; symbolic links are counted once, at their target
(Firn's `main` has 43 of them, mostly `bin/*.fi` -> `lib/firnc1/*.fi`).

| repository | what | files | lines |
|---|---|---:|---:|
| Firn | `lib/**.fi` -- standard library, browser, TLS, JS, compiler stage 1 | 200 | **162,113** |
| Firn | all `.fi` outside tests and test data | 373 | 189,174 |
| Firn | `compiler/**.rs` -- bootstrap compiler, stage 0 | 65 | 52,865 |
| Osum | `kernel/**.fi` -- kernel and userland | 140 | **75,280** |
| Osum | `kernel/*.s`, `kernel/user/crt.s` | 7 | 1,884 |
| Osum | `lib/libc/*.fi` | 8 | 1,690 |
| Osum | `tools/**` -- host test tools | 51 | 16,837 |
| OrientOS | `pkg/**.py` and the other Python here | 6 | 4,493 |
| OrientOS | shell -- `build.sh`, `test.sh`, `tests/step-*.sh` | 21 | 4,085 |

### 1.2 Foreign SOURCE CODE in the shipped system

**Zero lines.** In all three code repositories.

* Firn: `compiler/Cargo.toml` declares no dependency; no `extern crate`; a
  search for `SPDX-License` / `Licensed under` / `Copyright (c)` over `lib/`
  and `compiler/src/` returns nothing.
* Osum: same search over `kernel/`, `lib/`, `tools/` returns nothing. No
  `Cargo.toml` at all.
* OrientOS: compiles nothing; no `Cargo.toml`.

### 1.3 Foreign material that IS in the running system

Four items, and only four.

| # | what | where | size | licence | notice present? |
|---|---|---|---:|---|---|
| 1 | **Limine** bootloader, four files on the ISO | `orientos/vendor/limine/` -> `build/isoroot/` | **3,437,860 octets** | 2-clause BSD, present at `vendor/limine/LICENSE` | **NO -- not copied onto the ISO** |
| 2 | **DejaVu Sans** outlines, subsetted | `osum/assets/osum-sans.ttf` | 18,676 octets | Bitstream Vera / DejaVu | **NO -- no licence file, and the `name` table is stripped** |
| 3 | **DejaVu Sans Mono** outlines, subsetted | `osum/assets/osum-mono.ttf` | 14,604 octets | Bitstream Vera / DejaVu | **NO -- same** |
| 4 | **DejaVu Sans Mono** rasterised to 8x16 bitmaps | inside `osum/kernel/font.fi` | 1,520 octets | Bitstream Vera / DejaVu | **NO** |

**Total foreign material in the shipped system: 3,472,660 octets, of which
3,437,860 is Limine and 34,800 is glyph shapes.** Nothing else.

### 1.4 Foreign DATA that becomes own Firn tables at build time

Not code. Adopting it is what `orientos-browser/DECISION.md` section 7.1
declares binding (interpretation B1).

| generated file | lines | generated from | licence notice present? |
|---|---:|---|---|
| `firn/lib/generated/unicode_tables.fi` | 3,020 | Unicode Character Database 17.0.0, pinned by sha256 in `firn/tools/ucd/UCD.sha256` | **NO -- only a URL in `SOURCE.md`** |
| `firn/lib/html/entities_data.fi` | 4,677 | WHATWG named character references, 2,231 entries, via CPython `html.entities.html5` | **NO** |
| `firn/lib/browser/quirks_data.fi` | 70 | WHATWG HTML 13.2.6.4.1 DOCTYPE lists | **NO** |

### 1.5 Foreign material that is TEST or BUILD only

Never shipped. Listed so the picture is complete.

| what | where | size | licence |
|---|---|---:|---|
| Web Platform Tests (4 corpora) | `firn/tests/data/{wpt-ref,wpt-css,wpt-dom,html5lib}/` | 1,737 files | BSD-3-Clause; **no LICENSE file in the tree** |
| test262 | `firn/testdata/test262/` | 4.3 MiB, 32,893 files | BSD-3-Clause; **not in the archive either** |
| html5lib tokenizer tests | `firn/testdata/html5lib-tokenizer/` | 1.7 MB | **not stated anywhere** |
| JSONTestSuite | `firn/testdata/json/` | 11 KiB | MIT; **not in the archive** |
| NIST CAVP vectors | `firn/testdata/crypto/` | 960 KB | **no statement** |
| css-parsing-tests | `firn/testdata/css-parsing-tests/` | 76 KB | **CC0, LICENSE present** -- the only one done right |
| NBT `bigtest` | `firn/testdata/nbt/` | 507 octets | **not stated** |
| eight real web pages, 4 of them Wikipedia | `firn/testdata/realweb/` | 4,931,812 octets | **none named; Wikipedia is CC BY-SA 4.0** |
| six real TLS chains | `firn/tests/data/tls-chains/` | 6 files | none needed (public credentials) |
| `Ahem.ttf` | `firn/tests/data/fonts/` | 21,768 octets | public domain (Todd Fahrner 1995) |
| `FirnSans.ttf` -- DejaVu Sans subset | `firn/tests/data/fonts/` | 14,396 octets | Bitstream Vera / DejaVu; **no text in the tree** |
| Unicode `.txt` inputs | `firn/tools/ucd/` | 3,332,992 octets | Unicode terms; **no text in the tree** |
| `html5ever` + 37 transitive crates | `firn/bench/tokenizer/Cargo.lock` | fetched from crates.io | **unrecorded** |
| ~100 npm packages | `firn/tools/mcserver/node_modules/` | not tracked (`.gitignore:116`) | each carries its own LICENSE, mostly MIT |
| Python: numpy, fontTools, Pillow, html5lib, cssselect2, xxhash | `firn/tools/**` | from PyPI | **unrecorded**; three of the six optional |
| Python: `cryptography` | `orientos/pkg/opk.py:222-236` | from PyPI | **unrecorded**; **optional** -- see section 3 |

**Total foreign test and build material: 21,456,955 octets in Firn's
`testdata/` and `tests/data/` alone.** All of it inert.

---

## 2. The five findings, in order of how much they matter

### 1. OrientOS has no licence at all

`git ls-tree --name-only main | grep -i licen` and the same on `plan2`:
nothing. Firn has `LICENSE` (MIT, 21 lines). Osum has `LICENSE` (MIT). The
repository that assembles the actual product does not.

Without one, the default is all rights reserved -- nobody may copy, modify
or redistribute it, and that "nobody" includes a future arrangement with a
second contributor or a company. **Cost to fix: minutes.** It is the highest
value-per-effort item in this entire inventory.

### 2. Three fonts ship with the notice removed

The two `.ttf` files and the bitmap table in `kernel/font.fi` are DejaVu
glyph outlines. The Bitstream Vera / DejaVu licence permits everything that
was done to them -- subsetting, rasterising, redistribution -- on one
condition: the copyright notice travels with the font.

`osum/tools/ttf/schnitt.py` keeps seven TrueType tables and `name` is not
one of them. Reading the two shipped files back confirms it: **no `name`
table at all.** The notice was not omitted from a README, it was deleted out
of the file. And there is no licence text anywhere near them.

**Cost to fix the compliance defect: one file,
`osum/assets/FONT-LICENSE.txt`, today.**
**Cost to remove the dependency: draw 95 monospace and 95 proportional
glyphs.** Bounded, needs no new technology (`schnitt.py` and `kernel/ttf.fi`
handle whatever is fed in), and it is the item that would leave the shipped
system entirely own work apart from the bootloader.

### 3. Limine is unpinned, and its notice is not on the ISO

`orientos/vendor/limine/` carries **prebuilt binaries** dated 13 August 2026
with **no version, no tag, no commit hash and no checksum recorded
anywhere**. That is remarkable against the rest of this project family,
which pins everything: `vendor/firn/COMMIT`, `vendor/osum/COMMIT`,
`vendor/net/BLOBS` (blob hashes, re-checked by `test.sh`),
`firn/tools/ucd/UCD.sha256`, `firn/testdata/test262/subset.sha256` (a sum per
file, 32,893 of them). The one foreign binary that actually boots the machine
is the one thing not pinned.

Separately, `build.sh:239-242` copies four Limine binaries into the ISO and
does not copy `vendor/limine/LICENSE`, which the 2-clause BSD binary-form
condition asks for.

**Cost to fix: a `VERSION` file with sha256 sums, and one `cp` line in
`build.sh`.**

### 4. `testdata/realweb/` redistributes CC BY-SA content inside an MIT repository

Four full Wikipedia article HTML files (3,016,223 octets of the 4,931,812),
plus a Hacker News page that carries no free licence at all. The MANIFEST
records the URL of every page and the licence of none.

Wikipedia is CC BY-SA 4.0 -- attribution and share-alike. The Firn repository
is MIT. That combination is the clearest licence defect on the Firn side.

The corpus needs pages that are big, messy and real; it does not need these
particular pages. **Cheapest fix: fetch them at measuring time instead of
storing them, or swap them for freely redistributable pages.** Alternative:
keep them and add the attribution CC BY-SA requires.

### 5. Every borrowed test corpus names its licence and ships none of them

WPT (BSD-3-Clause, 1,737 files), test262 (BSD-3-Clause, 32,893 files),
JSONTestSuite (MIT), the Unicode database (Unicode terms), `FirnSans.ttf`
(Bitstream Vera). Every one of them is named correctly in a `PROVENANCE.md`
or `MANIFEST.md` -- which is more care than most projects take -- and not one
of them carries the upstream licence text. Two corpora
(`html5lib-tokenizer`, `nbt`) and the NIST vectors have no licence statement
at all and go under "must be checked".

`testdata/css-parsing-tests/LICENSE` (CC0, Simon Sapin 2013) is the single
counter-example and shows what the others should look like.

**Cost to fix: an afternoon and a `THIRD_PARTY_LICENSES/` directory.**

---

## 3. The question that was asked directly: Ed25519 in `opk.py`

`orientos/pkg/opk.py` implements Ed25519 twice.

* **Own implementation:** `pkg/opk.py:115-217`, RFC 8032, roughly 100 lines
  of pure Python integer arithmetic on top of `hashlib.sha512`. No
  dependency.
* **The host library:** `pkg/opk.py:222-236`, function
  `ed25519_verify_bibliothek`. **The library is
  `cryptography`** (pyca) -- specifically
  `cryptography.hazmat.primitives.asymmetric.ed25519.Ed25519PublicKey`.

**Is it required? No.** The import sits inside the function inside a
`try`/`except` that returns `None` when the package is absent. The three call
sites (`opk.py:1621-1622`, `1646-1647`, `2354-2357`) compute both and abort
**only when the two disagree**; `None` is not a disagreement. Verification
runs on the own implementation alone on a host without the package.

The reason it is there is written in the file at `pkg/opk.py:115-124`: a
signature check that always answers "yes" fails on no valid package and
therefore fails silently. Two implementations catch that; one cannot.

**Verdict:** keep it optional, do not make it required, do not remove it.
Removing it would make the check weaker, not the system more independent --
`opk.py` is a host tool and there is no Python on the ISO.

---

## 4. What to replace, in order

| # | item | stage | effort | worth doing? |
|---|---|---|---|---|
| 1 | Write `orientos/LICENSE` | -- | minutes | **yes, now** |
| 2 | `osum/assets/FONT-LICENSE.txt` + an `EXCEPTIONS` note in `osum/LICENSE` | -- | minutes | **yes, now** |
| 3 | `cp vendor/limine/LICENSE` into the ISO; pin Limine by version + sha256 | BUILD | an hour | **yes, now** |
| 4 | `THIRD_PARTY_LICENSES/` in Firn for WPT, test262, JSONTestSuite, UCD, DejaVu | TEST | an afternoon | **yes** |
| 5 | Fix `testdata/realweb/` -- fetch at measuring time, or swap the pages | TEST | a day | **yes** |
| 6 | **Draw own glyphs** -- 95 mono + 95 proportional + the 8x16 console bitmaps | **RUNTIME** | real design work, bounded | **yes** -- this is the one that removes foreign material from the running system |
| 7 | Own bootloader, BIOS + UEFI, to replace Limine | **RUNTIME** | a project of its own | **later.** Only once everything above the bootloader is own work. It runs for a fraction of a second and never again |

Not on the list, on purpose: the Unicode database, the WHATWG entity list,
the DOCTYPE quirks lists. Those are normative DATA and
`orientos-browser/DECISION.md` 7.1 already calls maintaining your own
Unicode "Unsinnig" -- it means maintaining Unicode.

---

## 5. Where independence makes no sense, said plainly

**Building your own assembler, your own linker, your own emulator or your own
firmware is not a goal while they are only used during development.**

* `as` and `ld` (GNU binutils, GPL-3.0) are called by `firnc` on every single
  compile -- `firn/compiler/src/target.rs:41-60`,
  `firn/compiler/src/main.rs:1039` and `:1080`. They are the one build
  dependency without which the project has no artifact at all. That is worth
  knowing. It is not worth fixing: the GPL applies to the tools, not to what
  they assemble, and the finished ISO contains no binutils.
* `qemu-system-x86_64` runs 86 times in Osum's scripts and 5 in Firn's. An
  emulator written by the same person who wrote the kernel will agree with
  the kernel about the bugs they share. **QEMU's independence is its entire
  value.** The same argument applies to OVMF and SeaBIOS: they are firmware,
  and firmware is precisely the layer a real machine brings itself.
* The same holds for every measuring instrument in the tree, and there are a
  lot of them, on purpose: `html5ever` for tokenizer throughput, Node's V8
  for the JavaScript engine, `html5lib` for tree construction, `cssselect2`
  for selectors, `fontTools` for font metrics, Pillow for PNG, `numpy` for
  pixel comparison, `python-xxhash` for hash vectors, `cryptography` for
  Ed25519 and for TLS primitives, `openssl s_server` for the TLS handshake,
  `libjpeg` for JPEG, Chromium for layout, the Linux kernel over `veth` for
  TCP/IP, `gcc`/glibc `strtod` for decimal parsing, `mkfs.vfat` for FAT.
  **Every one of these exists to disagree with the project's own code.**
  Replacing any of them with something written here would not increase
  independence; it would delete the measurement.

`orientos-browser/DECISION.md:380-383` already says this and says it well:
running foreign implementations as test oracles "ist kein Verstoss gegen B1
und spart bei der Fehlersuche Monate."

The line to hold is the one that is already drawn: **foreign code may measure
this system, and it may help build it. It may not be in it.** Today that line
is crossed by exactly four things -- one bootloader and three fonts -- and
both of those are fixable.

---

## 6. Licence compatibility against GPL-2.0-only (added 27.08.2026)

On 27 August 2026 Justin relicensed all four projects: **GPL-2.0-only** for
the Osum kernel, OrientOS, the Firn compiler and Certus; **MIT** for Firn
runtime and standard library and for Osum Ring 3 libraries. The reasoning is
in `orientos/LICENSING.md`, `osum/LICENSING.md` and `firn/LICENSING.md`.

Every third-party entry in section 1.3 to 1.5 above was checked against
GPL-2.0-only. **No blocking conflict exists.**

| foreign part | its licence | GPL-2.0-only? | note |
|---|---|---|---|
| Limine (on the ISO) | 2-clause BSD | **compatible** | its notice is still not copied onto the ISO. Unchanged defect |
| DejaVu glyphs (2 TTF + `kernel/font.fi`) | Bitstream Vera / DejaVu | **compatible** | licence text still absent, `name` table still stripped. Unchanged defect, now more visible |
| Lucide icon font (round ICONS, branch `icons`) | **ISC** | **compatible** | checked deliberately -- ISC, not Apache-2.0. `assets/icons/LICENSE.lucide` is present and `tools/icons/run.sh:108-113` fails the build without it |
| `cryptography` (optional, `pkg/opk.py`) | Apache-2.0 **OR** BSD-3-Clause | **compatible via the BSD branch** | Apache-2.0 alone would NOT be. The dual licence saves it, and it is optional and not redistributed |
| the 37 crates behind `html5ever` | 27x MIT OR Apache-2.0, 7x MIT, 1x (MIT OR Apache-2.0) AND Unicode-3.0 | **compatible** | **none is Apache-2.0-only.** `bench/tokenizer` is MIT anyway, so the question does not arise |
| Web Platform Tests, test262 | BSD-3-Clause | compatible | notice still missing from the tree |
| JSONTestSuite | MIT | compatible | |
| css-parsing-tests | CC0 | compatible | |
| Unicode Character Database | Unicode licence | compatible | permissive; the derived table is MIT |
| `Ahem.ttf` | public domain | compatible | |
| NIST CAVP vectors | not stated | not a licence question | data, not code, not shipped |
| `testdata/realweb/` (4x Wikipedia) | CC BY-SA 4.0 | **not a GPL question, but still wrong** | attribution and share-alike notice missing. Was a defect under MIT, is a defect under GPLv2 |
| binutils, gcc, qemu, xorriso, mtools | GPL-2.0 / GPL-3.0 | not a licence question | the GPL binds distribution of a TOOL, not of the tool output |
| OVMF | BSD-2-Clause-Patent | compatible, and irrelevant | firmware for a test, never shipped |
| SeaBIOS | LGPL-3.0 | irrelevant | inside QEMU, never shipped |
| rustc / cargo | Apache-2.0 OR MIT | not a licence question | a compiler, not a component |

**The single rule that follows from the decision:** GPL-2.0-only cannot
absorb **Apache-2.0** (patent-termination clause is a "further restriction"
that GPLv2 clause 6 forbids) and cannot absorb **GPLv3**. Nothing in the
tree is either, today. Before anything Apache-2.0-licensed is ever pulled
in, look for an MIT, BSD or ISC alternative first -- Lucide (ISC) is the
example of how round ICONS already did it right.

**One item moves to the top of the list because of this change:** the
DejaVu fonts. Both `LICENSE` files now make a repository-wide statement,
and neither statement has ever been true of those three files. Adding
`osum/assets/FONT-LICENSE.txt` is still minutes of work and is now overdue.
