# Round PLAN2 — the plan that carries a whole machine

**Date:** 27.08.2026 · branch `plan2` in OrientOS · kernel pinned at Osum
`c5fe12f` · format: [PLAN-FORMAT.md](PLAN-FORMAT.md) · backup:
[BACKUP.md](BACKUP.md) · test: `tests/step-90-plan.sh`

**Acceptance:** `./test.sh` → **ALL 15 STEPS PASSED, 234 assertions**,
0 failures. Before this round: 14 steps, 201 assertions. The 33 that came
in are exactly `step-90-plan.sh`; **not one existing assertion was lost**,
and the product ISO still boots over BIOS and over UEFI with the same
userland it had before.

Every number in this file comes from a run on this machine. Where a
number was not measured, it is not here.

---

## 1. What the round was for

`/system/generations/<n>/PLAN` used to be three lines and about 200
octets, and it described the installed **applications**. It was called
“the whole state of the system”, and it was not: it did not say which
kernel those applications run on, it did not say where the hashes could
be fetched from, and it did not say what the machine was configured to
be. A device could not be rebuilt from it.

After this round a plan looks like this — a whole machine, 13 lines,
**745 octets**:

```
account	justin	1000	1000	/users/justin	/bin/sh	52d94b3107c08a8b…
app	hallo	73d3f6f3c4e8256de0b444fd191c3cd6f14f5218a80052029b7bed70971fa6a0
app	ls	a52a49084a96b9930a510d927f8e237a41d181dc7e88cec26414e198b38fc313
app	wc	d5f76847e9a83f889023529e840d12ea686dddb82e46c5a2b02a4e7d5896096b
kernel	32f1a6edc3f4e1fd6d8430a03b0b64f609d9d4c69b1dabf8fd24f4f17caa6a12
setting	hostname	orient1
setting	net.address	192.168.1.50
setting	net.gateway	192.168.1.1
setting	net.mode	static
setting	net.netmask	255.255.255.0
setting	time.offset	7200
setting	timezone	Europe/Vienna
source	file:///…/build/quelle	1c39291b4c4687c807c505c14545703e…
```

Why the shape is this one and what was rejected — one file per aspect, a
version header, JSON, `key=value` per line, rewriting old files — is
argued in [PLAN-FORMAT.md](PLAN-FORMAT.md) § 3.

---

## 2. The core measurement — a tree rebuilt from one text file

The claim to prove was: *from a PLAN plus a source, a system tree can be
built from nothing, and it is byte-identical to the original.*

```
$ opk.py export --root A -o plan.txt
plan.txt: 13 line(s), 745 octet(s), 3 app(s), kernel 32f1a6edc3f4,
          1 source(s), 7 setting(s), 1 account(s)

$ opk.py rebuild --root B --plan plan.txt --secrets A/system/secrets
   source file:///…/build/quelle: 69 package(s), signature by 1c39291b4c4687c8…
   4 package(s) fetched and verified, 1735716 octet(s)
   1 credential(s) restored and checked against the plan
generation 1 in B

$ opk.py snapshot --root A > snapA;  opk.py snapshot --root B > snapB
$ cmp snapA snapB && tail -1 snapA
count entries=39 files=18 octets=3470309
```

**Identical over 39 entries, 18 files, 3 470 309 octets** — comparing
kind, path, mode, size and the SHA-256 of every content. What is inside
that comparison and what is deliberately outside it (the generation
history, the contents of the user pots, the secret store) is written out
in [PLAN-FORMAT.md](PLAN-FORMAT.md) § 9. The counter-check is in the
test: remove one package from the rebuilt tree and the same comparison
falls over by 12 lines, so it is not a constant.

### The same rebuild without the credentials

```
$ opk.py rebuild --root C --plan plan.txt
   1 account(s) are LOCKED because their credential is not here: justin.

$ diff snapA snapC
29c29
< f etc/shadow 600 116 3a1af078…
---
> f etc/shadow 600  24 77eff5b7…
41c41
< count entries=39 files=18 octets=3470309
---
> count entries=39 files=18 octets=3470217
```

**Exactly one file differs**, and it is `/etc/shadow`: the account is
written `justin:!:0:0:99999:7:::` — locked, not open. That is what a plan
that carries no passwords can do, stated as a measurement rather than as
a caveat.

### The plan carries its trust

A second source directory with the *same packages* and a *different*
Ed25519 key:

```
   REFUSED /tmp/fremd: the index is not signed by any of the 1 key(s) this plan names
```

Re-signed with the key the plan names, the same directory is accepted.
Both directions are in the test, because “refuses everything” would
otherwise pass.

---

## 3. The kernel is part of the generation

`pkg/bauen.sh` now builds the kernel as a package: `osum 1.0.0`,
**1 639 513 octets**, hash `32f1a6edc3f4…`, metadata
`kind=kernel`, `origin=osum:c5fe12ff1f6c6b062d986bb11543f9aabac6f690`.
The commit is read from `vendor/osum/COMMIT` at build time, so it is not
a second place for the same fact.

Measured:

* `system/kernel/image` and `store/32f1a6edc3f4…/image` are **the same
  inode** (`st_ino` compared, 1 639 256 octets) — one copy, not two.
* A kernel switch changes the octets under `system/kernel/image`
  (`efc6c111863b…` → `ac51402c82ff…`).
* `zurueck` puts the old one back:
  `KERNEL zurueckgestellt: 31c690c190ca -> 32f1a6edc3f4`.
* Going back past the generation where the kernel was first set removes
  `system/kernel` entirely, and the settings roll back with it —
  `Einstellungen zurueckgestellt: 7 -> 0`.
* The store keeps the kernel that is *not* currently selected, so a
  second roll-back has something to roll back to.
* `installieren` refuses a `kind=kernel` package; `kernel` refuses one
  without it.

### What of this is real, and what is bookkeeping

Real: the octets, the hashes, the hard link, the store retention, the
roll-back, the collector that keeps every kernel any generation names.

**Bookkeeping:** nothing boots from it. The boot loader boots what
`build.sh` wrote onto the ISO; no code reads `/system/kernel` at boot
time, because there is no writable disk and no installer. A kernel
roll-back today moves 1.6 MiB into the right place and changes nothing
about the next reboot. The round INSTALL is building the missing half;
until it lands, this is honest accounting over real bytes and not a
working kernel roll-back, and calling it one would be a lie the next
reboot exposes.

---

## 4. Settings and accounts

Nine setting keys, a **closed** set (`opk.py settings` prints it with its
consumers). Three of them produce files something in Osum actually reads:

```
time.offset  → /etc/time.conf     kernel/user/einstellungen.fi
screen.mode  → /etc/display.conf   kernel/user/einstellungen.fi
net.*        → /etc/network.conf     kernel/user/dhcp.fi, einstellungen.fi
```

and three do not have a consumer at all — `hostname`, `timezone`,
`keymap`. `opk.py set` says so when it writes one. They are recorded and
rendered, and that is the entire truth about them.

`/etc` is **derived**: activation deletes the eight paths in
`GENERATED_FILES` and rewrites the ones the plan asks for. Measured
counter-check: `unset screen.mode` makes `/etc/display.conf` disappear
from the tree. The tree is a function of the plan, not of its history.

The account line carries the **SHA-256 of the credential**, never the
credential. Measured: the exported plan does not contain the credential
bytes, and does contain their hash — so a password change is a
generation and can be rolled back, and the plan can still be handed to
somebody. `verify` catches a credential that no longer matches its hash.

---

## 5. More packages: 3 → 69

Before this round exactly **three** of Osum's programs had ever been
packaged (explorer, hallo, widgets), and the reason was that every recipe
was written by hand.

`pkg/recipes.py` generates them. Counted, not estimated:

| | |
|---|---|
| `.fi` files in the pinned Osum tree (`c5fe12f`) | **77** |
| of those, built into `vendor/osum/bin/` | **69** |
| `.fi` that produce no program | **8** — `appdir`, `flate`, `nidx`, `pw`, `tools`, `ulib`, `wlib`, `wlibc`; every one of them is a library that other programs `import` |
| recipes generated by machine | **62** |
| skipped, with a reason | **7** |
| packages in `build/pakete` after a full build | **69** (62 generated + 5 Osum bundles + 1 hand-written + 1 kernel) |
| total size of all `.opk` | **5 587 549 octets** |
| entries in the signed `INDEX` | **69**, 6 312 octets, Ed25519 verified by both implementations |

The brief said 82 programs. **82 is the count in Osum's *current* working
tree**; the pinned commit this repository builds against has 77, and the
five newer ones — `dhcp`, `einstellungen`, `leiste`, `power`,
`schreibtisch` — arrived with the DESKTOP and K18 rounds after the nail
was driven. They will be packaged automatically the moment
`vendor/osum/COMMIT` moves, without anyone writing a recipe.

### The seven that were skipped, and why

| program | reason |
|---|---|
| `edit` | the bundle `editor.osp` already packages it |
| `explorer` | the bundle `explorer.osp` already packages it |
| `sh` | the bundle `terminal.osp` already packages it |
| `starter` | the bundle `suchen.osp` already packages it |
| `wigdemo` | the bundle `widgets.osp` already packages it |
| `hello` | the hand-written recipe `hallo` already packages it |
| `suchen` | **a real name collision.** Osum's `suchen.osp` bundle starts `/bin/starter`, and `/bin/suchen` is a *different* binary. A package called `suchen` would mean two things. Packaging it needs a second name, and that is a decision, not a default. |

Nothing else failed. The six “already packaged” skips exist so that the
same octets do not land in the store under two hashes.

The generated recipes deliberately do **not** copy the descriptions out
of Osum's source headers. Those headers are half German and half English,
and this repository does not machine-translate another project's prose
and ship the result as its own metadata. `info` says what is actually
known: the program's name and the commit it came from. A program that
deserves a real description gets a bundle in Osum, and `pkg/bauen.sh`
reads that.

---

## 6. The backup set, measured

Full rule and table in [BACKUP.md](BACKUP.md). One tree, 9 applications,
a kernel, 7 settings, one account and some user data:

```
full tree, naive sum         5 938 071 octets,  90 files
full tree, distinct inodes   3 571 511 octets,  71 files
  store/ (programs)          2 368 499        never backed up
  apps/  (second names)        727 304        never backed up
  etc/   (generated)               126        never backed up
  users …/cache              1 150 000        never backed up
  users …/config                 3 000
  users …/state                 40 000
  PLAN                           1 046
  system/secrets                    30

BACKUP SET      44 076 octets  =  1.23 % of the tree
NEVER BACKED UP  3 518 499 octets
```

Two numbers worth handing to round TRESOR without waiting for it:
`store/` and `apps/` should be excluded **by name** rather than
deduplicated away (they are already content-addressed, and `rebuild`
fetches them from a signed source), and anything that copies second names
as separate files inflates a backup of this tree by **66 %**
(5 938 071 against 3 571 511).

---

## 7. Backwards compatibility

Old `PLAN` files — plain `name<TAB>hash` — still parse, and the tool says
so rather than pretending otherwise:

```
$ opk.py plan --root old
generation 0, 1 app(s), kernel none, 0 source(s), 0 setting(s), 0 account(s)
1 line(s) were read in the OLD two-field shape (name<TAB>hash) and are
treated as applications
```

The one ambiguity — an application named after one of the five types — is
closed at build time and at install time. `kernel` is refused; the test
also builds `kernelchen` to prove that it is those five words and not a
substring match.

The metadata format grew two optional fields (`kind`, `origin`). They are
written **last and sorted**, after the fixed five and after `braucht=` /
`handle=`, so a recipe that uses neither produces exactly the octets it
produced before. Checked: `explorer` is still
`478632825585ffb1b8639f68da1bd3b0d132ea42e2cab51de820a320552d8f95`, the
same hash the older documents quote.

---

## 8. How much of the code is still German

The owner's rule for this round was: everything new in English, do not
rename the existing body. Counted in `pkg/opk.py` (1 007 → **2 148**
lines):

| | definitions | ≈ lines |
|---|---|---|
| German-named (`plan_schreiben`, `neue_generation`, `aktivieren`, `installieren`, `entfernen`, `aufraeumen`, `pruefen`, …) | **43** | **954** |
| English-named (round PLAN2) | **54** | **1 104** |

So roughly **46 %** of `pkg/opk.py` still carries German names. The field
names of the package metadata (`fassung`, `braucht`, `handle`, `titel`)
are German too. The handle name `konsole` was the same debt and is
**PAID**: round `rename` made it `console` on both sides in one commit,
which is the only way such a name moves -- spelling it two ways would
have put two names on one object. The flag `--wurzel` gained `--root` as
an alias; both work everywhere.

The two Osum paths this section used to call "German that stays" --
`/etc/network.conf` and `/etc/time.conf` -- are **gone**: round `rename`
made them `/etc/network.conf` and `/etc/time.conf`. Paths are structure
and structure is English. What is left of them here is their CONTENT
(`modus=fest`, `maske=`), which is Osum's file format, and this
repository writes what the other side reads. The *keys* in the plan
are English (`net.mode`, value `static`) and the renderer translates.

Of the 18 Markdown files in the repository root and `docs/`, **16 still
contain German**. The three added this round —
[PLAN-FORMAT.md](PLAN-FORMAT.md), [BACKUP.md](BACKUP.md) and this one —
do not.

---

## 9. What was measured, and by what

`tests/step-90-plan.sh`, one step, **33 assertions**, every one with its
counter-check, and three counter-checks to counter-checks (an intact plan
is accepted, a known setting is taken, `kernelchen` builds) — without
those, a tool that refuses everything would be green here.

In the whole suite: **15 steps, 234 assertions, 0 failures** (before:
14 / 201; after the addendum below: **16 / 261**). Nothing that was green before turned red — in particular
`step-80-pakete.sh`, which measures the older half of the package
manager, and `step-20`/`step-30`, which build and boot the product.
The metadata change is the reason that matters: it was designed so that
existing package hashes do not move, and the suite would have said
otherwise.

`opk.py verify` is the second, independent check and runs on the demo
tree with **28 assertions, 0 failed**. Its counter-checks are in the
test: a `PLAN` reversed by hand must fail the sort check, a credential
with one octet appended must fail its hash.

---

## 10. What still does not work

* **Nothing boots from `system/kernel`.** § 3. The INSTALL round is the
  other half.
* **No `https://` sources.** `file://` and local paths work; network
  fetching is not implemented and therefore not claimed.
* **`opk` still runs on the host, not on Osum.** It is a Python program
  and the booted system has no Python. Unchanged from the previous round;
  a `/bin/opk` in Firn is the honest continuation.
* **The product ISO carries no settings and no accounts.** `/etc` in the
  image is still assembled by `userland/PROGRAMME`, and the generated
  `/etc` of a plan would collide with it in `mkfs`. Merging the two is a
  small, real piece of work that was left out of this round on purpose:
  `build.sh` is on the critical path of a green test suite and three
  other rounds are running in parallel.
* **User data does not roll back**, and `entfernen` still really deletes
  the three pots. Unchanged, and stated in [BACKUP.md](BACKUP.md) § 6.
* **`suchen` is not packaged** — the name is taken by a bundle that
  starts a different program.
* **No dependency resolution.** Unchanged: what is missing is named, not
  fetched.

---

# Addendum, 27.08.2026 — "does it LOOK the same?"

The owner asked one question about the round above: *if I set up a new
device from the plan, does it look the same?* It did not. This addendum
is the answer, and the numbers below are from runs on this machine.

Design: [CONFIG-LEVELS.md](CONFIG-LEVELS.md). Test:
`tests/step-91-look.sh`, **27 assertions**, each with its counter-check.

**Acceptance after the addendum:** `./test.sh` -> **ALL 16 STEPS PASSED,
261 assertions**, 0 failures. Before the addendum: 15 steps, 234. The 27
that came in are exactly `step-91-look.sh`; nothing that was green turned
red, and the product ISO is unchanged (7424 KiB, same CRC32 0x1ff0e0ba)
because the shipped plan has no preferences yet -- see § A8.

## A1. What was wrong, measured on Osum's tree

`kernel/user/einstellungen.fi` is 788 lines and writes **only** into
`/etc`: `theme`, `schemas/`, `wallpaper`, `display.conf`, `time.conf`,
`network.conf`, `shadow`. None of it was in a `PLAN` or under
`/users/<who>/config`. A rebuilt device got the applications of the old
one and the looks of a fresh install.

Worse, half of those files sit at the **wrong level**. The test is not
"is it about looks" but *would two people on one machine want different
answers*. `/etc/theme` fails that test: it is a place where a second user
cannot exist.

## A2. What was built

* A **sixth plan type**, `pref <user> <key> <value>`, for the five
  per-person keys `theme`, `wallpaper`, `taskbar.edge`,
  `taskbar.height`, `taskbar.autohide`. System keys stay `setting`.
* **Asset packages**: `kind=asset`, `class=theme|wallpaper`, one file
  `blob`. A wallpaper travels the same way a program does — store, hash,
  signed `INDEX`, collector, `rebuild`. No second blob store.
* `pkg/osym.py`: four lines of text → the OSYM image
  `schreibtisch.fi` displays, deterministically.
* Two colour schemes and two wallpapers shipped as text
  (`userland/themes/`, `userland/wallpapers/`).
* A **compatibility view** under `/etc` — hard links, single-account
  machines only.
* New commands `asset-pack`, `pref`, `pref-unset`, `prefs`.
* One rename, allowed because the key was ours and one day old and had no
  consumer: `screen.mode` → `display.mode`. The **file** keeps Osum's
  name `display.conf`.

## A3. The measurement — does it look the same?

A system set up with a different colour scheme (`dusk`), a different
wallpaper (`deep`), the taskbar on the **left** at 40 px with autohide,
timezone Europe/Vienna, offset 7200, resolution 1024x768. Plan exported,
rebuilt on an **empty** directory:

```
$ opk.py export --root A -o look.txt
look.txt: 15 line(s), 855 octet(s), 3 app(s), kernel 32f1a6edc3f4,
          1 source(s), 4 setting(s), 1 account(s),
          5 preference(s) for 1 user(s)

$ opk.py rebuild --root B --plan look.txt
   6 package(s) fetched and verified, 2112740 octet(s)

$ cmp snapA snapB && tail -1 snapA
count entries=62 files=35 octets=4396976
```

**Identical over 62 entries, 35 files, 4 396 976 octets.** In the test
suite, on a slightly smaller tree (two applications instead of three, no
network setting): **45 entries, 25 files, 3 923 350 octets**, and the
eight files that actually make up the appearance —

```
etc/theme  etc/wallpaper  etc/taskbar.conf  etc/display.conf  etc/time.conf
users/justin/config/desktop/{theme,wallpaper,taskbar.conf}
```

— compared **individually**, octet for octet, all eight equal.

Counter-check: change **one** preference in the rebuilt tree
(`taskbar.edge` bottom→top) and the comparison falls over by 8 lines. So
the comparison is not a constant.

**The answer to the question is yes**, with the limits in § A6.

## A4. The level split, measured

Two accounts, `justin` and `anna`, each with their own scheme, wallpaper
and taskbar:

```
justin: edge=left height=40 autohide=yes   theme 7c48667d...  (dusk)
anna:   edge=top  height=28 autohide=no    theme c9a5cac7...  (dawn)
etc/schemas: dawn  dusk
```

and the compatibility view **disappears**:

```
NO compatibility view under etc/: this machine has 2 accounts, and there
is no honest answer to whose theme /etc/theme would be
```

With one account it is there — `etc/theme`, `etc/wallpaper`,
`etc/taskbar.conf` — which is what Osum's taskbar and desktop read today.
Both directions are assertions, so "deletes everything always" cannot
pass.

That the view vanishes is not a limitation of this program. It is the bug
of § A1 becoming visible instead of being decided arbitrarily.

## A5. Images are not text

The wallpaper is 172 812 octets and the plan is 855. What is in the plan
is the SHA-256; the octets are an asset package.

* The plan contains **no image octets** (checked: no `OSYM` anywhere in
  it, everything printable) and **does** contain the hash.
* The image has **three names and one inode** — `store/<h>/blob`,
  `users/justin/config/desktop/wallpaper`, `etc/wallpaper`. Verified
  by `st_ino`, and `opk.py verify` asserts it.
* The same four lines of text produce the same 172 812 octets twice, so
  the hash is reproducible from this repository.
* `pkg/osym.py` refuses 800×600 — `schreibtisch.fi` caps its source at
  240×180 and stretches. An unloadable image would be worse than an
  error.

Package count is now **73** (69 programs and the kernel, plus 2 colour
schemes and 2 wallpapers), `INDEX` 6 674 octets, still one signature over
all of it.

## A6. Passwords, restated

Unchanged from the round above, and it is the same principle the
wallpaper follows: the plan names content by its hash, never the content.

```
account	justin	1000	1000	/users/justin	/bin/sh	52d94b3107c08a8b…
```

Name, uid, gid, home, shell and the **SHA-256 of the credential**. The
credential itself is in `system/secrets/<name>`, mode `0600`, and is
never in a plan. A rebuild without it produces the account **locked**
(`justin:!:0:0:99999:7:::`); a rebuild with it **verifies** the hash and
refuses a credential that does not match. Measured: such a rebuild
differs from the original in exactly one file.

## A7. A defect found in this round's own work

`snapshot_lines` walked `users/<who>/{config,state,cache}` in full and
therefore compared the user's **documents**, which no plan determines —
contradicting its own docstring. It went unnoticed because the pots were
empty in the run that measured PLAN2. Fixed: the pots are walked for
their directory structure only, plus the few generated files under
`config/desktop/`. It is written down here because a number that was
measured with a broken instrument would otherwise stand unchallenged.

## A8. What still does not work

* **The product ISO carries no appearance.** `/etc` in the image is still
  assembled by `userland/PROGRAMME` and would collide with the plan's
  generated `/etc` in `mkfs`. One merge, deliberately not done while
  three rounds run in parallel on a green suite.
* **Nothing switches the view on login.** With two accounts the
  compatibility view is absent — correct, not yet useful. The login path
  must tell the desktop whose configuration to read; that is DESKTOP's.
* **`einstellungen.fi` still writes `/etc` directly**, so a change made
  in the running system does not become a generation and is lost on the
  next activation. Needs `opk` on Osum, i.e. `/bin/opk` in Firn.
* **`/etc/schemas` lists only the schemes the plan uses**, not everything
  available. A fuller picker is a catalogue question and the answer is
  the source's `INDEX`, not the system state.
* **Wallpapers are capped at 240×180** and stretched — Osum's memory
  budget, stated rather than worked around.

## A9. German that stayed, on purpose

`/etc/theme`, `/etc/wallpaper`, `/etc/schemas/`, `/etc/display.conf`,
`/etc/time.conf`, `/etc/network.conf` keep their names — Osum opens them by
name, and renaming another project's files from here would break it.
Their contents (`modus=fest`, `maske=`) are Osum's format too. Everything
this addendum *introduced* is English: `pref`, `theme`, `wallpaper`,
`taskbar.edge/height/autohide`, `config/desktop/`, `taskbar.conf`,
`asset-pack`, `class=`, `userland/themes/`, `userland/wallpapers/`,
`pkg/osym.py`. The full list, with the reason for each, is in
[CONFIG-LEVELS.md](CONFIG-LEVELS.md) § 8.

---

# Second addendum, 27.08.2026 — two machines

Firn, Osum and OrientOS are to run on x86-64 **and** on AArch64. Two
rounds are already at it: ARM-FREESTANDING in the Firn repository and
OSUM-ARM in Osum's. The question that landed here, because this round
was extending the PLAN format anyway: **what does a hash mean when there
are two machines?**

The full argument, with the alternatives that were thrown away, is in
[ARCHITECTURES.md](ARCHITECTURES.md). This is the log.

## B1. The answer, and why it needed almost no new idea

A package is identified by the SHA-256 of its content. Machine code for
another machine is other content, so it is another package with another
hash. That was already true before this addendum; nobody had had to
notice it. The work was not inventing a mechanism, it was making the
fact **visible** and **refusable** — because the bad outcome is not a
refusal, it is a device that installs the wrong binaries quietly and
stops at the first instruction with nothing to read.

Two facts, in two places, each written down once:

* `arch=` **in the package metadata** — what this package is for. It is
  the right place because the metadata is *inside the hash*: a package
  cannot change what it claims without becoming a different package.
* `arch` **as one PLAN line** — what this *machine* is. One line, like
  `setting display.mode`.

Rejected: putting the machine in the **package name**. It would make
`braucht=` unwritable (a dependency would have to name a machine it
cannot know), make two plans that should differ in one line differ in
every line, and put the word `x86_64` on the desktop of somebody who
does not care.

## B2. It is measured, not declared

`bauen` reads the ELF header of every file it packs — `e_machine`, two
octets at offset 18 — and takes the machine from there. Measured:

> the two recipes for the two builds are **identical except for the path
> of the binary**

A recipe *may* declare `arch=`, and then it is checked against the
measurement and loses if they disagree. This is the same rule as
everywhere else in this program: the octets decide, the text is checked
against them.

**An exception table that the measurement deleted.** The first version
had `ARCH_EXCEPTIONS`, so a recipe could declare `arch=x86_64` and be
forgiven for a 32-bit i386 file — the multiboot kernel image is EM_386
because the firmware enters it in protected mode (measured: `osum.mb` is
EM_386 class 1, `osum.mb.elf` is EM_X86_64 class 2, the same kernel).
Building the 73 packages showed it **never fired once**: with EM_386
mapped to `x86_64` in the lookup table, declaration and measurement
already agree. It was unreachable code with a story attached, which is
worse than a comment. It is gone.

## B3. The measurement

**One source, two machines, with a real toolchain — not a flipped byte.**
`firnc --target=x86_64-linux` and `--target=aarch64-linux` over
`firn/tests/048_print_number.fi`:

| | |
|---|---|
| x86-64, native | prints `12345`, exit 0 |
| aarch64, under `qemu-aarch64` | prints `12345`, exit 0 |
| e_machine | 62 (EM_X86_64) and 183 (EM_AARCH64) |
| package hashes | **different** |

**An arch-independent package has ONE hash.** The same wallpaper packed
on two build hosts: **identical, 173 018 octets**, `arch=any` — and
nobody typed that either, because there was no ELF header to read.

**Six refusals, each with its counter-check:**

| what | said |
|---|---|
| the aarch64 build on an x86-64 system | `twin is for aarch64, this system is x86_64 -- REFUSED` |
| a package from before this round | `does not say which machine it is for` |
| a source offering both, machine unstated | `nothing said which machine this is for` |
| a fat package | `holds machine code for 2 different machines (start=EM_X86_64, start2=EM_AARCH64)` |
| a dependency across the boundary | `is aarch64 and needs lib, but the lib installed here is x86_64` |
| a plan claiming aarch64 that names the x86-64 build | refused, and **nothing** was activated (0 bundles in `/apps`) |

**A rebuild on the other machine.** An aarch64 plan, rebuilt on an empty
root, produces ARM binaries in `/apps/twin.osp/start`. The two plans —
x86-64 and aarch64 — differ in **exactly the lines that name machine
code (2) plus the one that names the machine (2)**, and the aarch64 plan
is 4 printable lines, 281 octets. `cat` still reads the whole state.

**One store, both machines** (the build server, not the device):

```
  aarch64     1 entry     4246 octets  twin
  any         1 entry   172934 octets  deepw
  x86_64      1 entry     3941 octets  twin
```

and the activated view names exactly one of them, because a device is
one machine.

## B4. What `any` saves, and why the number is small

On the real source of 73 packages: 69 `x86_64` (5 588 377 octets) and 4
`any` (346 977). A source serving both machines holds 142 entries and
11 523 731 octets instead of 146 and 11 870 708 — **saved 346 977
octets, 2.9 %**.

**That is a small number and it should be read as one.** It is small
because only 4 of 73 packages are machine-independent today: two colour
schemes and two wallpapers. The category that moves it does not exist
here yet, and rather than estimate, here is what was measured next door:
`osum-mono.ttf` 14 604 and `osum-sans.ttf` 18 676 octets; on branch
`i18n`, `locale/de/messages` 4 897 and `locale/en/messages` 4 868. Every
one of those becomes `arch=any` the moment it is packaged, with no
further work. The mechanism is in place; what is missing is content, and
content belongs to round I18N.

## B5. `.opk` versus `.osp`, checked against the code

The owner asked; the answer was checked against the source before it was
written down. **`.opk` is what you hold, `.osp` is what runs.**

`.opk` is a FILE: 64-octet header (`OPKG0001`, two lengths, the SHA-256
of metadata plus data), then a deterministic archive — not `tar`,
because tar carries timestamps and owners and two runs would give
different octets, and then "same hash = same content" would be false.
`paket_lesen` recomputes the hash and is the only way into the system.

`/apps/<name>.osp/` is a DIRECTORY: the activated view of the current
generation, rebuilt completely on every activation, every file a **hard
link** into the store (`Wurzel._verweisen`), costing directory entries
and not one block. `PAKET` stays in the store — a bundle holds what a
*program* needs. Osum finds these by their suffix
(`kernel/user/appdir.fi`, `endet_auf_prog`), reads `INFO` from inside,
and runs `<bundle>/start`; `starter.fi` line 47 has
`/apps/suchen.osp/start` spelled out.

That bundle contract — one path called `start` — is also the third
argument against fat packages: a fat bundle needs either two paths or a
chooser inside every program.

## B6. What is explicitly NOT true yet

* **Osum does not run on AArch64.** The ARM binaries measured here were
  produced by Firn and run under `qemu-aarch64`. They are real ELF files
  that really execute — they are **not** Osum programs, because there are
  none. The package manager is ready for them and has not seen one.
* **All 69 code packages are `x86_64`.** The other half of the source
  cannot exist until OSUM-ARM lands.
* **The product ISO has no architecture.** `build.sh` builds one image
  for one machine and never asks. Rounds FORMAT and INSTALL own that.
* **Every package built before this addendum is now refused** by a
  system that states its machine. Intended, loud, and fixed by one run
  of `pkg/bauen.sh`.

## B7. Debt this addendum touched

`pkg/recipes.py` kept its **own copy** of the reserved-name list, five
names written out by hand. Round PLAN2 had added `pref` and this
addendum added `arch`, so the copy was wrong by two: a package called
`pref` would have been built there and refused three commands later. It
now imports `PLAN_TYPES` from `pkg/opk.py`. One list, in the program
that owns the format — and the step asserts the two agree.

Also swept: three messages that said "one of the five PLAN types" now
count the tuple instead of naming a number that had already been wrong
twice.

**English:** everything new here is English — `arch`, `arch=`, `any`,
`archs`, `ELF_MACHINE`, `arch_ok`, `arch_decide`, `arch_refusal`,
`ARCHITECTURES.md`, `tests/step-92-arch.sh` including its prose. The
machine names `x86_64` and `aarch64` are **Firn's**, taken from
`compiler/src/target.rs` lines 80-81 rather than invented; the `-linux`
half of the triple is dropped because in an `.opk` the operating system
is always Osum.

One inconsistency to record rather than hide: `tests/step-91-look.sh`,
written earlier in this same round, has **German prose** in its comments
and assertion texts. `step-92-arch.sh` is English throughout. The two
should match, and making them match is round RENAME's sweep, not a
silent edit here.

---

# Third addendum, 27.08.2026 — programs with no source

The owner's question: **"what about programs that are not in the app
store, when I set up a new device and want my applications back?"**

It was a real hole and not a detail. A PLAN names `app <name> <hash>`;
the octets have to come from somewhere; and for a package built by hand
and installed from a file there was no somewhere. Worse, `rebuild` said

```
opk: 4 of 39 hash(es) are in no source: 5eff0f81b5cf, …
```

which is the least useful thing it could have said, because the one
question anybody has at that moment is *which ones*.

## C1. The answer, in three parts, the first of which is not machinery

**1. The main road is a second source.** A plan may name **any number**
of sources, each with its own key — that was already true,
`plan.sources` is a dictionary and `source-add` appends. So the right
answer to "I build my own packages" is: run your own source. A directory
of `.opk` files with an `INDEX` and an Ed25519 signature *is* a source,
on a NAS, a server or a memory stick, and `opk.py quelle` makes one in
one command. **Measured:** one `source-add` of a private directory takes
a tree from `2 of 3 covered, 1 ORPHANED` to `3 of 3 covered, 0
ORPHANED`, and the backup set stops carrying the octets in the same
breath. The plan then holds two `source` lines and neither is a special
case of the other.

**2. Orphanhood is decidable**, so nobody keeps a list. A hash is
covered if some recorded source's INDEX has it, orphaned if none does.
That is the difference between this design and one where "which of my
programs are irreplaceable" is a question answered from memory.

**3. What is orphaned, and only that, goes in the backup.** The rule
`PLAN + config/ + state/` (§ 6) is unchanged; it gains one exception,
and the exception is decided by a machine.

## C2. The four claims, measured

**(a) It is reported.**

```
$ opk.py orphans --root /
  ok       hallo                        bc9c62d6877b  x86_64   from appstore
  ok       the theme of justin          45e6577d4bc4  any      from appstore
  ORPHANED mytool                       5eff0f81b5cf  x86_64   4146 octet(s) in the store
2 of 3 covered by a source, 1 ORPHANED
```

`--strict` exits 1 while anything is orphaned, so this is a nightly
check rather than a habit. Note that an asset is named in words — *the
theme of justin* — because the name has to come out of the **plan**, not
out of the package: the package is exactly the thing that may be
missing.

**(b) It lands in the backup.** `backup-set` produced 7 lines with
**exactly one** `package` line; `hallo`, which has a source, is not in
it, and neither is any `cache/`. `vault-export` wrote **1 package,
4 228 octets**, and that file is the original `.opk` back **octet for
octet** — the store holds packages unpacked, and repacking is
deterministic by construction, which is checked rather than assumed.

**(c) A rebuild on an empty root restores it.**

```
   source file:///…/appstore: 2 package(s), signature by a6594ee4849d3329...
   vault  …/backup/vault: 1 package(s), each verified against its own checksum
   3 package(s) fetched and verified, 34070 octet(s), 1 of them from the vault
```

The restored binary is the same one (`604441accffbe0ff…`), it **runs**
and prints what it printed before, and the whole tree matches
**entry for entry over 29 entries** (kind, path, mode, size, SHA-256 of
every content).

**(d) Without the backup, a name and not a hole.**

```
1 of 3 package(s) can be fetched from NOWHERE:
   MISSING  mytool                       5eff0f81b5cf2844
   These are orphans: no source of this plan has them and no vault was given.
opk: refusing to build a system that is missing 1 of its 3 package(s).
```

**Nothing was built** — 0 bundles in `/apps`. `--allow-missing` builds
the rest and then the **tree itself** carries the evidence in
`system/INCOMPLETE`; `verify` reads it and **fails**:

```
FAILED this tree is INCOMPLETE: it was rebuilt without 1 package(s) -- mytool
21 assertion(s), 1 failed
```

A complete tree does not have that file, so its absence is a statement
and not a default.

## C3. The orphan on the other machine

The case where "package missing" would be a lie — the octets are right
here and still cannot help. It gets its own message, because the usual
advice would be wrong:

```
opk: mytool is for x86_64 and this system is aarch64 -- and it is an
ORPHAN: no source of this plan has it, so there is no aarch64 build to
fetch instead. The octets in the vault are the only ones there are and
they cannot run here. This package has to be BUILT AGAIN from its source
code for aarch64; a backup cannot solve it, and it is not pretending to.
```

The step asserts that it does **not** say "fetch that one" — that advice
belongs to packages with a source.

## C4. Why the vault needs no signature, although a source does

The two are asked different questions:

> a **source** is asked *"give me the package called `explorer`"* — the
> answer is a **choice somebody else makes**, and a name says nothing
> about content, so it must be signed.
>
> a **vault** is asked *"give me the octets whose SHA-256 is `abc…`"* —
> the answer **checks itself against the question**.

So `--vault` takes a plain directory and no key, and that is not a hole.

## C5. Two honest distinctions that cost code

**Unreachable is not empty.** A source this host cannot read must not be
mistaken for a source that lacks the package, or a laptop on the wrong
network would archive its whole store. `orphans` names it unusable,
`backup-set` writes a `# unreachable` line into the file, and
`vault-export` **refuses outright** unless told otherwise. Both are
asserted.

**Repacking is checked, not trusted.** `store_repack` refuses if packing
the store directory again does not give back the very same hash —
writing that into an archive somebody will later trust is exactly the
kind of quiet damage this repository is built to avoid.

## C6. The interface to round TRESOR

Written down so that round can take it without a conversation
(`docs/BACKUP.md` § 5). `opk.py backup-set` emits typed tab-separated
lines — `plan`, `secret`, `tree`, `package`, and `# unreachable` as a
warning — all paths relative to the root. TRESOR needs no knowledge of
plans, sources or orphans, and **must never decide what is worth
keeping**: that decision is made where the plan is, by computation.

## C7. What is still not solved

* **`https://` is still not implemented**, and this addendum makes that
  hurt twice: a private source on a NAS is the *recommended* answer, and
  reaching one over a network is exactly what is missing. `file://` and
  local paths work; that is what was measured.
* **An orphan for the wrong machine cannot be rescued at all.** Nothing
  here can change that; the error says so rather than pretending.
* **`vault-export` is not a backup program.** It writes files into a
  directory. Scheduling, rotation and getting them off the machine are
  TRESOR's.
* **A backup restores the current state, not the history.** The set
  carries the *current* PLAN. Older generations are not in it, so a
  restored machine can go forward but not back past the restore.

---

# Fourth addendum, 27.08.2026 — moving house on a stick

The owner's question: **"if I have a backup on a USB stick — can I set a
new device up from it without signing in to an account? Just say: I want
this device to be like that one too."**

The full argument is in [MOVE.md](MOVE.md). This is the log.

## D1. The principle, because it is one

> **AN ACCOUNT IS A CONVENIENCE. IT IS NEVER A CONDITION.**

If a sign-in server is ever built it may make this *easier*; it may never
make it *possible*, because it already is. **The stick is the measure and
the server is the special case.** A server that can do one thing the
stick cannot is the day the stick stops being maintained and the promise
quietly becomes a lie.

It is measured rather than asserted: the whole restore in
`tests/step-94-move.sh` runs with the package source **moved out of the
way**, and the log is checked for the proof that it really was
unreachable at the time.

## D2. What a device is — the part that must NOT travel

"I want this device to be like that one" moves a state onto a **different
machine**. Two machines carrying one identity is a fault that is silent
on the day it is made: two hosts on one name, two static addresses on one
network, two devices presenting one key.

> **The PLAN holds wishes. An identity is not a wish.**

So identity is not in the plan at all — not as a filtered field, not as a
skipped line. It lives in `system/`, **outside the generations**, where a
plan cannot carry it even by accident. `Wurzel.anlegen` gives every root
a `machine-id` and an Ed25519 device key the moment it exists, because
the alternative is identity created lazily somewhere, and lazily created
identity is how two machines end up with one.

| never travels | dropped unless `--keep-identity` | travels |
|---|---|---|
| `system/machine-id` | `hostname` | apps, kernel, `arch`, sources+keys |
| `system/device.key`, `.pub` | `net.address/netmask/gateway` (static) | `timezone`, `keymap`, `net.mode=dhcp` |
| | `net.mode=static` | preferences, accounts, `config/`, `state/` |

`net.mode=dhcp` travels because it is a **policy, not an address**: two
machines may both ask for a lease and neither takes anything from the
other. That distinction is the whole reason `identity_settings` is a
function and not a tuple.

## D3. The measurement

A device with 4 applications, a kernel, an account with a password, six
settings, three preferences and 340 KB of documents → stick → **empty**
root, source moved away:

> compared **entry for entry over 64 entries**, documents included: the
> **only** differences are the PLAN (**921 → 768 octets**) and the two
> files those five machine settings rendered, `etc/hostname` and
> `etc/network.conf`

**The counter-check that makes it a measurement:** the same stick with
`--keep-identity` gives a tree **identical over all 64 entries**. So the
differences were exactly three, not roughly three.

`snapshot` gained `--with-data` for this, and it had to: the default
compares the pot *directories* and not their contents, which is right for
`rebuild` (a plan describes a system, not the work done on it) and would
have gone **green on losing every document** here. Two different
questions, two different answers, both now available and both named.

Three trees, **three distinct machine-ids**. The device key is new too,
mode 0600.

## D4. `small` or `full`, and why `full` won

| | octets | files |
|---|---|---|
| `small` (orphans only) | **219 213** | 11 |
| `full` (everything) | **2 170 942** | 16 |
| | `full` is **9.9×** | |

**`full` is the default for `stick-write`; `small` stays the default for
`backup-set`.** They are two jobs and one default for both would be wrong
for one of them. A nightly backup should be small and may assume a
network on the day it is read. A stick you set a machine up from is made
for the case where there **is** no network — so a mode that only works
with one fails exactly where it was needed. A move that needs a network
is not a move, it is a download with extra steps.

The ratio is not a constant: with the kernel installed by hand rather
than fetched, `small` was 1 858 872 octets and the ratio fell to **1.3×**.
`small` is small when the app store is doing its job.

One thing no mode may do: **leave an orphan behind.** It is on the small
stick too, and that is asserted.

## D5. Partial moves

* `--no-personal` — 3 programs, **0** documents, **0** accounts, **0**
  preferences; the plain settings such as the timezone stay. For a device
  that goes to somebody else. On `stick-write` it means the documents are
  **not on the stick at all** — "not present" and "not restored" are very
  different promises when the stick is in another hand, and both are
  asserted.
* `--no-data` — 3 preferences, 4 settings files, **0** documents. Same
  person, fresh start, still his machine.

## D6. One format for TRESOR, not two

`vault/` accepts **copied `store/<hash>/` directories** as well as `.opk`
files, because copying directories is what a block-deduplicating backup
wants to write. Measured: a stick whose vault holds copied store
directories restores to a tree **identical** to the one built from
`.opk` files. `dir_repack` is shared by both paths and refuses if
repacking does not give back the hash the list names.

The stick layout is the **backup set at the same relative paths** — the
step asserts that every path `SET` names is really on the stick, so
"it is not a second format" is checked rather than promised.

## D7. A correction to the third addendum

`stick-restore` first copied the credentials itself, after `rebuild`.
That worked and was wrong: `rebuild` hashes each credential and compares
it against what the plan claims, and going around it skipped that check —
so the log printed *"1 account(s) are LOCKED"* one line before silently
unlocking them. The secrets now go **through** `rebuild`, which is why
the run says `1 credential(s) restored and checked against the plan`.

## D8. What is still not solved

* **Nothing in Osum reads `machine-id` or the device key.** Generated,
  kept and regenerated correctly; no consumer. The same honest state as
  `hostname` and `keymap`.
* **A stick is not a disk.** Putting the tree on a partition so that it
  boots is round INSTALL's.
* **No user interface.** TRESOR is building "back up to here" in the file
  explorer; these are the commands underneath, on the same data.
* **`--keep-identity` is a loaded gun.** Nothing checks whether the old
  machine is really gone, and nothing can.

---

## 11. What the next round needs

1. **INSTALL**: make the boot path read `system/kernel`, so that the
   kernel line stops being bookkeeping. That is the single largest gap.
2. **A merge of `/etc`**: let `userland/PROGRAMME` and the plan's
   generated `/etc` coexist in one image, so a shipped product can carry
   a hostname and an account.
3. **The rename round**: 43 German definitions in `pkg/opk.py`, the
   metadata field names. (`konsole` is done — round `rename`.) It is mechanical and it is
   overdue, and it must be one round on its own with the hashes
   recomputed in the same commit.
4. **A stable source key.** `pkg/bauen.sh` generates a new Ed25519 key on
   every build, which is honest for a build directory and useless for a
   source anyone would actually fetch from. A real source needs a fixed
   public key, and then the question of where *that* lives.
5. **`https://` in `rebuild`**, at which point a plan is genuinely
   portable off this machine.
6. **OSUM-ARM, then 69 aarch64 packages.** The package manager handles
   two machines and has never seen a second one. The moment Osum builds
   for AArch64, `pkg/recipes.py` and `pkg/bauen.sh` produce the other
   half of the source with no change to either — that claim is worth
   testing the day it becomes testable.
7. **Fonts and translations as `arch=any` packages.** They are the
   content that makes the sharing in §B4 worth more than 2.9 %, and they
   need no new mechanism — only packaging. Round I18N.
8. **`https://` sources, again and louder.** Three addenda have now
   ended at the same wall. A private source is the recommended answer to
   orphaned packages and it cannot be reached over a network.
9. **Older generations in the backup set.** Today a restore lands on the
   current PLAN and cannot roll back past itself. Whether that is worth
   fixing is a real question — the generations are tiny, the packages
   they name are not — and it should be answered rather than left.
10. **A consumer for `machine-id` and the device key.** They are created
    correctly and nothing reads them. That is acceptable for one round
    and turns into decoration if it lasts three.
11. **INSTALL has to meet `stick-restore`.** One produces a tree, the
    other puts a tree on a disk. Until they are joined, "set a new device
    up from a stick" ends one step short of a machine that boots.
