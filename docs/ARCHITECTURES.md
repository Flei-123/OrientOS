# Two machines

*Round PLAN2, second addendum, 27.08.2026. Everything here that is a
number comes from a run; the runs are named where they are quoted.*

Firn, Osum and OrientOS are to run on x86-64 **and** on AArch64. Firn's
round 80 already compiles the same source for both — 290 of 294 cases
behave identically — and round OSUM-ARM is putting an architecture seam
through the kernel. That leaves one question for the package manager,
and it is the only one that matters:

> **What does a hash mean now?**

It means what it meant. A package is identified by the SHA-256 of its
content. Machine code for another machine is other content, so it is
another package with another hash. Nothing had to be invented for that,
and the sections below are not about inventing it. They are about making
it **visible** and **refusable**, because the bad outcome is not a
refusal — it is a device that installs the wrong binaries quietly and
stops at the first instruction with nothing to read.

---

## 1. `.opk` and `.prog`, since the question came up

The short form first, because it is the part worth remembering:

> **`.opk` is what you hold. `.prog` is what runs.**

They are different kinds of thing, not two names for one thing.

| | `.opk` | `/apps/<name>.prog/` |
|---|---|---|
| **What it is** | a FILE | a DIRECTORY |
| **Purpose** | transport and archive | the activated view of the current generation |
| **Identified by** | the SHA-256 of its content | its name, and nothing else |
| **Contains** | a 64-octet header, metadata, a deterministic archive | `start`, `INFO`, `symbol`, `daten/` — as second names |
| **Costs disk** | yes, once, in `store/<20 hex>/` | **no** — every file is a hard link into the store |
| **Who reads it** | `pkg/opk.py` | Osum: `kernel/user/appdir.fi`, `starter.fi`, `explorer.fi` |
| **Rebuilt** | never — a package is immutable by definition | **completely**, on every activation |

**The `.opk` file.** `pkg/opk.py`, `paket_packen`. Eight octets of
magic (`OPKG0001`), the two lengths, and the SHA-256 of metadata plus
data, in a header of 64 octets. `paket_lesen` recomputes that hash and
is *the only way into the system* — there is deliberately no second path
that could forget to check. The archive inside is not `tar`: tar carries
timestamps, owners and padding, so two runs over one tree give different
octets, and then "same hash = same content" is no longer true and the
whole design falls over. `archiv_bauen` sorts by the octets of the name
and takes the file mode from a rule rather than from the host's umask.

**The `.prog` bundle.** `Wurzel.aktivieren` deletes `apps/` and rebuilds
it from the plan, hard-linking every file out of the store
(`_verweisen`). The metadata file `PAKET` stays behind in the store: a
bundle is what a *program* needs, not what the package manager needs.
Osum finds these directories by their suffix (`appdir.fi`,
`endet_auf_prog`), reads `INFO` out of the bundle, and starts
`<bundle>/start` — `starter.fi` line 47 has `/apps/suchen.prog/start`
written out in full. The form is Apple's: one program is one directory,
installing is copying, removing is deleting, and there is no second place
to forget. The suffix is `.prog` and not `.app` because a system that
borrows the shape does not have to borrow the name too.

**Why this matters for two machines.** The bundle contract is
`<bundle>/start` — one path, no variants. Anything that put two
architectures into one bundle would have to break that contract or hide
a chooser behind it. See §3.

---

## 2. Where the architecture lives

Three places were possible. Two of them would have been copies of the
third, and this repository's recurring rule is that a second place where
a name lives is the place that stops being true.

### Rejected: in the package name (`explorer-x86_64`)

The name is what a person types, what `braucht=` refers to, and what
`/apps/<name>.prog` is called. Putting the machine in it breaks three
things at once:

* **Dependencies become unwritable.** `braucht=explorer` would have to
  say *which* explorer, in a file that cannot know where it will be
  installed. Every library would need one recipe per machine for no
  reason other than its own name.
* **Plans stop being comparable.** Two systems that should differ in one
  line would differ in every line.
* **It leaks.** `x86_64` would appear in the launcher, in the file
  explorer and on the desktop of somebody who does not care.

### Chosen: in the package metadata, `arch=`

This is the **truth**, and it is the right place for exactly one reason:
the metadata is *inside the hash*. `paket_packen` hashes `meta + daten`.
A package cannot change what it says it is for without becoming a
different package. Nothing else on this page has that property.

### Chosen as well: in the plan, one `arch` line

This says a **different fact**: not what a package is for, but what
*this machine is*. One line, exactly like `setting display.mode`:

```
arch	x86_64
```

Without it, a plan handed to a foreign machine could be rebuilt into a
tree of binaries that cannot run, and nothing would have said so. With
it, the rebuild either works or is refused by name. It is the answer to
the requirement that a plan must be *either* cleanly rebuildable on a
foreign machine *or* clearly refused.

So: two facts, in two places, each written down once. `arch=` in a
package says what it is for; `arch` in a plan says what the machine is.
Neither is derivable from the other and neither is stored twice.

### And it is measured, not declared

A recipe does **not** have to say `arch=`. `bauen` reads the ELF header
of every file it packs — `e_machine`, two octets at offset 18 — and
takes the machine from there. Measured in `tests/step-92-arch.sh`:

> the two recipes for the two builds are **identical except for the path
> of the binary**, and the packages come out `x86_64` and `aarch64`.

A recipe *may* declare it, and then the declaration is checked against
the measurement and loses if they disagree:

```
opk: liar says arch=aarch64, but the file start is EM_X86_64.
The recipe and the octets disagree, and the octets are the ones
that will run.
```

**An exception table that the measurement made unnecessary.** The first
version of this had `ARCH_EXCEPTIONS`, so that a recipe could declare
`arch=x86_64` and be forgiven for a 32-bit i386 file — the multiboot
kernel image is EM_386 because the firmware enters it in protected mode
and the image switches to long mode itself (measured: `osum.mb` is
EM_386 class 1, `osum.mb.elf` is EM_X86_64 class 2, same kernel).
Building the 73 packages showed the table **never fired once**: with
EM_386 mapped to `x86_64` in `ELF_MACHINE`, the declaration and the
measurement already agree. It was code that could not be reached, and it
is gone. The judgement now sits on one line of a lookup table with the
reason next to it. If OrientOS ever supports a machine that is really
32-bit only, that line becomes wrong and the fix is a third name — not
an exception to a rule.

### The acceptance rule, whole

| plan says | package says | result |
|---|---|---|
| nothing | anything | **accept** — an old plan makes no claim, so there is nothing to check against. The backward-compatible door, and the only quiet one. |
| a machine | `any` | **accept** — no machine code, runs anywhere |
| a machine | the same | **accept** |
| a machine | another | **REFUSE** |
| a machine | nothing | **REFUSE** — an unlabelled binary on a machine that knows what it is *is* the silent mis-installation this exists to prevent |

The last row is the one that costs something: every package built before
this round is unlabelled and will be refused by a system that states its
architecture. That is intended, it is loud, and the fix is one command —
`pkg/bauen.sh` rebuilds all 73 and stamps what it measures.

---

## 3. One package per machine, not one fat package

**Confirmed, and the owner's reasoning is the right one:** separate
packages, because then a hash still names exactly one thing.

Three further reasons, in order of how much they cost:

1. **A fat package moves a decision out of the plan.** Which half gets
   activated would be decided by the unpacker, at activation time, from
   something that is not in the text file. `rebuild` would stop being a
   function of the plan — and `rebuild` being a function of the plan is
   the entire measurement of round PLAN2.
2. **It breaks the bundle contract.** Osum starts `<bundle>/start`
   (§1). A fat bundle needs `start/x86_64` and `start/aarch64`, or a
   shim at `start` that chooses — which is a chooser in every single
   program, on a system whose kernel is being ported right now.
3. **It ships the other machine's octets to every device.** Measured on
   the real 73-package source: 5 588 377 octets of machine code. A fat
   world puts all of it on both devices; separate packages put it on
   one.

**The honest counter-argument** is that a fat package is *one file you
can hand somebody*, and separate packages are two. That is true, and it
is already solved one level up: the signed `INDEX` of a source is one
directory that serves both machines, and it says which entry is which
(§5). The convenience exists; it just does not have to live inside the
hash.

`arch_decide` refuses a fat package at build time, and the message says
why rather than what:

```
opk: fat holds machine code for 2 different machines
(start=EM_X86_64, start2=EM_AARCH64). A `fat` package is refused
on purpose: its SHA-256 would name two things at once, and `which
of the two is installed here` would then be a decision taken
outside the plan.
```

---

## 4. `arch=any` — the part that is a real advantage

A package with no machine code in it gets `arch=any`, and **nobody
decides that either**: there was no ELF header to read. `any` is the
same three octets on both machines, so the metadata is the same, so the
**hash is the same**, so the store holds it once and both machines name
it.

This is not a feature that was added. It is what content addressing does
when it is not fought.

Measured (`tests/step-92-arch.sh`, and the run in
`docs/ROUND-PLAN2.md`):

> a wallpaper packed on two build hosts is **identical, 173 018 octets**,
> and comes out `arch=any` — one store entry serves both machines

On the real source of 73 packages:

| | packages | octets |
|---|---|---|
| `x86_64` | 69 | 5 588 377 |
| `any` | 4 | 346 977 |
| **total** | **73** | **5 935 354** |

A source serving both machines therefore holds `69 × 2 + 4 = 142`
entries and **11 523 731** octets, instead of `73 × 2 = 146` entries and
11 870 708 octets if everything were stamped per machine. **Saved:
346 977 octets, 2.9 %.**

**That number is small today, and pretending otherwise would be
dishonest.** It is small because only 4 of 73 packages (5.5 %) are
currently machine-independent — two colour schemes and two wallpapers.
The category that will move it is the one that does not exist here yet.
Measured in the neighbouring repository rather than guessed:

* `osum/assets/osum-mono.ttf` 14 604 and `osum-sans.ttf` 18 676 octets —
  fonts, `any` by construction
* branch `i18n`, `locale/de/messages` 4 897 and `locale/en/messages`
  4 868 octets, and that is two languages of an unfinished round

Every one of those is `arch=any` the moment it is packaged, with no
further work, because it contains no ELF header. The mechanism is in
place; what is missing is content, and content is round I18N's.

**An asset cannot smuggle code past this.** `asset_bauen` runs the same
measurement and refuses:

```
opk: sneak holds machine code (blob is EM_X86_64) and would be an
asset -- refused. An asset is a colour scheme or an image; if this
is a program it belongs in a program package, where its
architecture is checked against the machine.
```

---

## 5. Dependencies, and a store that holds both

**Dependencies are resolved per machine.** `braucht=libfoo` names a
name, and on a store that holds two machines a name is not enough. Both
`installieren` and `verify` check it:

```
opk: user is aarch64 and needs lib, but the lib installed here is
x86_64. A dependency is resolved by name, and on a machine with
two architectures a name alone is not enough.
```

Two `any` packages are always compatible — that is what `any` means —
and a same-machine dependency resolves exactly as it always did.

**A store can hold both machines**, and it needs no mechanism to do so:
entries are named by the SHA-256 of their content, and two machines
never produce the same content. Measured — two rebuilds, two plans, one
root:

```
store /tmp/.../both/store, this system is aarch64
  aarch64     1 entry     4246 octets  twin
  any         1 entry   172934 octets  deepw
  x86_64      1 entry     3941 octets  twin
  2 machines side by side in one store; `any` is 1 entry and
  172934 octets, held ONCE instead of 2 times
```

and the activated view names exactly **one** of them, because a device
is one machine.

**A source can hold both**, and this needed one thing: the `INDEX` is
keyed by name, and two packages called `cat` would have collided. It has
a sixth column now:

```
name	fassung	sha256	size	file	arch
```

Five-column indexes from before this round are still read
(`_index_zeile`); their entries simply say nothing about machines. When
a source offers a choice and the asking system has not said what it is,
**nothing is picked**:

```
opk: the source has 2 builds of twin (aarch64, x86_64) and nothing
said which machine this is for -- set the architecture of the
system (`opk.py arch <name>`) or the choice would be made by
accident.
```

---

## 6. The commands

| | |
|---|---|
| `opk.py arch --root R` | what this system is, and what machine each installed package is for |
| `opk.py arch --root R <name>` | set it — a **new generation**, so it can be rolled back |
| `opk.py archs --root R` | what the **store** holds, per machine, with what `any` saves. The build server's view. |

`arch <name>` refuses to make a wrong tree look right:

```
opk: this system cannot be called aarch64: 1 installed package(s)
are not for it (twin=x86_64). Changing the word would not change
the octets -- install the aarch64 builds first, then say so.
```

---

## 7. What is not true yet

* **Osum does not run on AArch64.** Round OSUM-ARM is putting the seam
  in. Everything measured here about *two real machines* used Firn as
  the compiler (`firnc --target=`), and the AArch64 binary is a real ELF
  that really runs — under `qemu-aarch64`, printing the same output as
  the x86-64 build, both exiting 0. It is **not** an Osum program,
  because there are none yet. The package manager is ready for them; it
  has not seen one.
* **All 69 code packages are `x86_64`.** The aarch64 half of the real
  source does not exist and cannot until OSUM-ARM lands. The 2.9 %
  sharing figure in §4 is arithmetic over measured sizes, and it is
  labelled as such.
* **There is no `arch` in the product ISO.** `build.sh` builds one
  image, for one machine, and does not ask. That is round FORMAT's and
  round INSTALL's ground.
* **`ELF_MACHINE` has three entries.** Anything else is refused by name
  rather than guessed at, which is right, but it does mean a fourth
  machine is a change to this program.
