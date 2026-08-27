# Round PLAN2 — the plan that carries a whole machine

**Date:** 27.08.2026 · branch `plan2` in OrientOS · kernel pinned at Osum
`c5fe12f` · format: [PLAN-FORMAT.md](PLAN-FORMAT.md) · backup:
[BACKUP.md](BACKUP.md) · test: `tests/step-90-plan.sh`

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
