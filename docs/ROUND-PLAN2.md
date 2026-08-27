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
time.offset  → /etc/zeit.conf     kernel/user/einstellungen.fi
screen.mode  → /etc/schirm.conf   kernel/user/einstellungen.fi
net.*        → /etc/netz.conf     kernel/user/dhcp.fi, einstellungen.fi
```

and three do not have a consumer at all — `hostname`, `timezone`,
`keymap`. `opk.py set` says so when it writes one. They are recorded and
rendered, and that is the entire truth about them.

`/etc` is **derived**: activation deletes the eight paths in
`GENERATED_FILES` and rewrites the ones the plan asks for. Measured
counter-check: `unset screen.mode` makes `/etc/schirm.conf` disappear
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
| `edit` | the bundle `editor.prog` already packages it |
| `explorer` | the bundle `explorer.prog` already packages it |
| `sh` | the bundle `terminal.prog` already packages it |
| `starter` | the bundle `suchen.prog` already packages it |
| `wigdemo` | the bundle `widgets.prog` already packages it |
| `hello` | the hand-written recipe `hallo` already packages it |
| `suchen` | **a real name collision.** Osum's `suchen.prog` bundle starts `/bin/starter`, and `/bin/suchen` is a *different* binary. A package called `suchen` would mean two things. Packaging it needs a second name, and that is a decision, not a default. |

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
are German too, and so is the handle name `konsole` — the generated
recipes use `konsole` on purpose, because spelling it `console` would put
two names on one object, which is worse than one German name. The flag
`--wurzel` gained `--root` as an alias; both work everywhere.

Two places where German stays and is **not** debt: `/etc/netz.conf` and
`/etc/zeit.conf` are Osum's file formats (`modus=fest`, `maske=`), and
this repository writes what the other side reads. The *keys* in the plan
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
`/etc`: `theme`, `schemas/`, `hintergrund`, `schirm.conf`, `zeit.conf`,
`netz.conf`, `shadow`. None of it was in a `PLAN` or under
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
  name `schirm.conf`.

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
etc/theme  etc/hintergrund  etc/taskbar.conf  etc/schirm.conf  etc/zeit.conf
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

With one account it is there — `etc/theme`, `etc/hintergrund`,
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
  `users/justin/config/desktop/wallpaper`, `etc/hintergrund`. Verified
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

`/etc/theme`, `/etc/hintergrund`, `/etc/schemas/`, `/etc/schirm.conf`,
`/etc/zeit.conf`, `/etc/netz.conf` keep their names — Osum opens them by
name, and renaming another project's files from here would break it.
Their contents (`modus=fest`, `maske=`) are Osum's format too. Everything
this addendum *introduced* is English: `pref`, `theme`, `wallpaper`,
`taskbar.edge/height/autohide`, `config/desktop/`, `taskbar.conf`,
`asset-pack`, `class=`, `userland/themes/`, `userland/wallpapers/`,
`pkg/osym.py`. The full list, with the reason for each, is in
[CONFIG-LEVELS.md](CONFIG-LEVELS.md) § 8.

---

## 11. What the next round needs

1. **INSTALL**: make the boot path read `system/kernel`, so that the
   kernel line stops being bookkeeping. That is the single largest gap.
2. **A merge of `/etc`**: let `userland/PROGRAMME` and the plan's
   generated `/etc` coexist in one image, so a shipped product can carry
   a hostname and an account.
3. **The rename round**: 43 German definitions in `pkg/opk.py`, the
   metadata field names, and `konsole`. It is mechanical and it is
   overdue, and it must be one round on its own with the hashes
   recomputed in the same commit.
4. **A stable source key.** `pkg/bauen.sh` generates a new Ed25519 key on
   every build, which is honest for a build directory and useless for a
   source anyone would actually fetch from. A real source needs a fixed
   public key, and then the question of where *that* lives.
5. **`https://` in `rebuild`**, at which point a plan is genuinely
   portable off this machine.
