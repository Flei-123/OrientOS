# LICENSING.md -- why this repository is GPL-2.0-only

Decision of 27 August 2026. Justin (Flei123) is the sole author of
OrientOS, Osum, Firn and the browser requirements, so the change needed
nobody else's agreement.

**Until this date this repository had no licence file at all.** Not on
`main`, not on `plan2`. The only `LICENSE` anywhere in the tree was
Limine's, which covers Limine. Round INVENTORY found it and called it the
highest value-per-effort item in the whole project; this is that fix.

The machine readable answer for any single file is its
`SPDX-License-Identifier:` line. This document says why the line says what
it says. The same reasoning, in the same words, is in `firn/LICENSING.md`
and `osum/LICENSING.md`.

---

## 0. Why "no licence" was not a neutral state

Without a licence, copyright law's default applies: **all rights
reserved**. Nobody may copy, modify or redistribute the work. "I wrote it
so I can do what I like with it" holds only while there has never been a
second contributor and never a redistributed build. The moment either
happens, the absence has to be sorted out retroactively -- and retroactive
relicensing needs the agreement of everyone who ever committed.

It also blocked the ordinary things: nobody could legally accept a patch,
nobody could legally hand somebody an ISO, and a later commercial
arrangement would have had a hole in the middle of it.

## 1. Why GPL and not MIT

MIT lets anybody take the work, close it and sell it without giving
anything back. For a library that is often the right trade. For an
operating system it is not: the point of building one is that it stays
open. **Whoever ships a changed OrientOS has to ship the changes.**

## 2. Why version 2 ONLY, and not version 3, and not "or later"

**GPLv3 section 6 requires "Installation Information" for User Products:**
ship GPLv3 software inside a consumer device and you must also ship
whatever a user needs to install their own modified version on it --
including the signing keys.

That would make one specific thing legally impossible: **binding firmware
to the machine it came with, as a theft deterrent.** A device that only
runs software signed for it is worthless to a thief. Under GPLv3 section 6
the manufacturer would have to hand out the keys that defeat exactly that.

**Linux is GPLv2-only for this reason**, stated publicly by Linus Torvalds
when GPLv3 was drafted; Android carries GPLv2-only for the kernel and
permissive licences above it for the same practical reason. OrientOS
follows that path deliberately.

**"or any later version" is left out on purpose.** With that clause the
Free Software Foundation could effectively relicense this work later by
publishing a GPLv4 -- a decision that would be out of Justin's hands. Every
SPDX line here says `GPL-2.0-only`, never `GPL-2.0-or-later`.

**The cost, stated honestly:** GPL-2.0-only is **incompatible with
Apache-2.0** (Apache's patent-termination clause is a "further restriction"
that GPLv2 clause 6 forbids) and with GPLv3. This repository can never
absorb Apache-2.0 or GPLv3 source. Section 5 checks what that costs today.
The answer is: nothing.

## 3. Why there is no MIT part here, and why that is not a problem

Firn and Osum each carry an MIT carve-out, because both contain code that
gets **linked into a program the user compiles** -- Firn's runtime and
standard library, Osum's Ring 3 libraries. If those were GPL, every program
ever written for this system would be GPL and nobody would write one.

**Nothing in OrientOS is linked into anything.** `build.sh` assembles,
`pkg/opk.py` packages, `tests/step-*.sh` measures. All of it runs on the
host and none of it ends up inside a user's binary. There is therefore
nothing to carve out, and the whole repository can be GPL-2.0-only without
making the system unusable.

**A program for OrientOS may still be closed source.** It links Firn's
runtime (MIT) and Osum's Ring 3 libraries (MIT), and neither of those is
affected by this file. That is the arrangement, and it is deliberate:

> The system is open and stays open. What you write **for** it is yours.

---

## 4. The boundary, file by file

There is only one side. For completeness:

### GPL-2.0-only -- all of it

| path | files / lines | what it is |
|---|---|---|
| `pkg/opk.py` | 3,648 lines | the package manager: `.opk` format, content-addressed store, generations, rollback, Ed25519 signatures |
| `pkg/mkfs-spec.py`, `pkg/osym.py`, `pkg/recipes.py` | 384 lines | the OFS build spec, the OSYM icon format, the recipe table |
| `pkg/bauen.sh` | | the package build driver |
| `build.sh` | | assembles the ISO out of the pinned kernel, the userland and Limine |
| `brand.sh`, `run-osum.sh`, `rename.sh`, `test.sh` | | |
| `tests/step-*.sh`, `tests/firn-elf/faelle.py` | 21 shell + 1 Python | the acceptance suite |
| `assets/marke/logo_build.py` | 117 lines | the logo generator |
| `assets/marke/*.svg`, `*.png`, `*.ico`, `osum-master.json` | 34 files | the brand artwork. Justin's own drawing, vectorised once with potrace |
| `vorlage/`, `brands/`, `userland/` | | templates, brand definitions, theme and wallpaper text |
| documentation (`*.md`, `docs/**`) | 29 files, 7,252 lines | |

### NOT covered -- third party, keeps its own terms

| path | terms |
|---|---|
| `vendor/limine/**` | 2-clause BSD, `vendor/limine/LICENSE` |
| `vendor/osum/COMMIT`, `hole-osum.sh`, `patches/*` | Justin's own, Osum's licences apply to what they fetch |
| `vendor/firn/COMMIT`, `hole-firnc.sh` | Justin's own, Firn's licences apply to what they fetch |

**One remark on the brand artwork.** It is licensed GPL-2.0-only with
everything else, which is the consistent choice but probably not the
intended one long term: a logo usually wants to be a **trademark** -- free
to reproduce when talking about the project, not free to slap on somebody
else's fork. If that is what you want, `assets/marke/` needs a sentence of
its own. Flagged, not decided.

---

## 5. Does GPL-2.0-only conflict with anything already in the tree?

Checked against every entry in `THIRD_PARTY.md`.

### 5.1 Limine -- 2-clause BSD, compatible

`vendor/limine/`, 16 tracked files, 4,610,523 octets, of which 3,437,860
go onto the ISO and execute on every boot.

**2-clause BSD is GPL-2.0-compatible.** A GPL-2.0-only ISO may carry a BSD
bootloader; the combination is exactly what every Linux distribution ships.
**No conflict.**

Two things that were true before this change and are still true:

1. **The notice is not on the ISO.** BSD clause 2 requires binary
   redistributions to reproduce the copyright notice "in the documentation
   and/or other materials provided with the distribution".
   `build.sh:239-242` copies four Limine binaries into `build/isoroot/` and
   does not copy `vendor/limine/LICENSE`. One `cp` line fixes it, and it
   should be added in the same change that adds this file.
2. **Limine is not pinned.** No version, no tag, no checksum -- the only
   foreign dependency in the whole project family without one. See
   `THIRD_PARTY.md` section 9.

### 5.2 `cryptography` in `pkg/opk.py` -- optional, and compatible either way

Imported only inside `ed25519_verify_bibliothek` (`pkg/opk.py:222-236`), in
a `try`/`except` that returns `None` if the package is absent. Upstream it
is **Apache-2.0 OR BSD-3-Clause**.

Apache-2.0 is the incompatible one -- **but the BSD-3-Clause branch of the
dual licence is available and is GPL-2.0-compatible**, so there is no
conflict. And it would not matter anyway: `opk.py` does not link the
package, it imports it at runtime on the host, it works without it, and
nothing of it is redistributed here.

**Recommendation, one line:** put a comment next to the import saying that
the BSD-3-Clause branch of `cryptography` is the one relied upon. Costs
nothing and removes the question for the next reader.

### 5.3 Build and test tools

`xorriso` (GPL-3.0), `make`, `gcc`, `qemu` (GPL-2.0), `mtools` (GPL-3.0),
`python3` (PSF), `git`, **OVMF** (BSD-2-Clause-Patent), **SeaBIOS**
(LGPL-3.0), binutils (GPL-3.0).

None of these is redistributed by this repository and none is linked into
anything. **The GPL binds distribution of a tool, not of the tool's
output.** A GPL-3.0 `xorriso` producing a GPL-2.0-only ISO is not a licence
event. **No conflict.**

Worth naming precisely because it looks like one: **OVMF is
BSD-2-Clause-Patent**, which the FSF does consider GPL-compatible -- and it
does not matter here, because OVMF is firmware supplied by the host for a
test, never shipped.

### 5.4 The Osum kernel and the Firn compiler this repository pulls in

`vendor/osum/COMMIT` fetches a kernel that is **GPL-2.0-only** and Ring 3
libraries that are **MIT**; `vendor/firn/COMMIT` fetches a compiler that is
**GPL-2.0-only** and a runtime that is **MIT**. All four combine with a
GPL-2.0-only assembly repository without friction.

**Result: nothing has to be removed or replaced because of this licence
change.**

---

## 6. What this changes for somebody using OrientOS

| you want to ... | may you? |
|---|---|
| write a program for OrientOS and sell it closed | **yes.** The runtime you link is MIT (Firn) and MIT (Osum Ring 3) |
| ship a changed OrientOS | not closed. GPL-2.0-only -- publish your changes |
| use `opk` in your own product | yes, under GPL-2.0-only |
| ship a device that runs OrientOS and only boots signed firmware | **yes.** That is the entire reason for GPLv2 instead of GPLv3 |
| take Apache-2.0 code into this repository | **no.** Incompatible. Look for MIT, BSD or ISC |
