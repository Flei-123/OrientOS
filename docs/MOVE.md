# Moving a system to another machine

*Round PLAN2, fourth addendum, 27.08.2026. Every number comes from one
run; the step that produces it is `tests/step-94-move.sh`.*

The owner's question: **"if I have a backup on a USB stick — can I set a
new device up from it without signing in to an account? Just say: I want
this device to be like that one too."**

---

## 1. The principle

> ## AN ACCOUNT IS A CONVENIENCE. IT IS NEVER A CONDITION.

Whoever holds their system state on a stick must be able to set a device
up completely from it: **no sign-in, no network, no server asked.**

If a sign-in server is ever built, it may make this *easier*. It may
never make it *possible*, because it already is. **The stick is the
measure and the server is the special case** — not the other way round.
A server may not be allowed to do one thing the stick cannot do, because
the day it can is the day the stick stops being maintained and the
promise quietly becomes a lie.

That is not a preference about convenience. It is the difference between
a machine somebody owns and a machine somebody is allowed to use, and
being on the first side of that line is a large part of why this project
exists.

**This is measured and not asserted.** In `tests/step-94-move.sh` the
whole restore runs with the package source *moved out of the way*, and
the log is checked for the proof that it really was unreachable at the
time:

```
   REFUSED file:///…/appstore: /…/appstore has no INDEX
   vault /…/full/vault: 6 package(s), each verified against its own checksum
   6 package(s) fetched and verified, 1955318 octet(s), 6 of them from the vault
…
done. No account was asked for and no network was used.
```

---

## 2. What is on the stick

`opk.py stick-write` produces a directory. It is **not a new format**: it
is the backup set of § 1 of [BACKUP.md](BACKUP.md), at the same relative
paths the machine uses, plus the package octets.

```
STICK                              the manifest: mode, arch, generation, what was dropped
SET                                the backup set — same file `backup-set` writes
system/generations/<n>/PLAN        the state itself
system/secrets/<name>              credentials, mode 0600
users/<who>/config/…               settings per application
users/<who>/state/…                the documents
vault/<hash>.opk                   the package octets
```

A backup program that reads `opk.py backup-set` and copies what it names
produces this directory **without being told about it**. The step asserts
that every path `SET` names really exists on the stick.

`vault/` also accepts **copied `store/<hash>/` directories** instead of
`.opk` files, because that is what a block-deduplicating backup wants to
write. Measured: a stick whose vault holds copied store directories
restores to a tree that is **identical** to the one built from `.opk`
files. One format, not two.

---

## 3. `small` or `full` — the decision, and the numbers

A stick can carry only what no source can give back, or everything.

| | `small` | `full` |
|---|---|---|
| carries | orphaned packages only | every package the plan names |
| needs a network on restore | **yes** | **no** |
| what it is for | the nightly backup | setting a device up |

**Measured** on a tree with 4 applications, a kernel, two assets, one
account and 340 KB of documents:

| | octets | files |
|---|---|---|
| `small` | **219 213** | 11 |
| `full` | **2 170 942** | 16 |
| difference | **1 951 729** | `full` is **9.9×** the size |

**The decision: `full` is the default for `stick-write`, `small` stays
the default for `backup-set`.** They are two jobs. A backup that runs
every night should be small and may assume the network exists on the day
it is read. A stick you set a machine up from is made for the situation
where there *is* no network — the old machine is dead, you have moved
house, the new place has no line yet — so a mode that only works with one
fails exactly where it was needed. **A move that needs a network is not
a move, it is a download with extra steps.**

One mode difference that does not exist: **an orphaned package is on the
small stick too.** No source can give it back, so no mode may ever leave
it behind. That is asserted.

The ratio is not a constant. It is entirely determined by how much of
the machine has a source: on the same tree with the kernel installed by
hand rather than fetched, `small` was 1 858 872 octets and the ratio fell
to 1.3×. `small` is small when the app store is doing its job.

---

## 4. What is a device, and what must never travel

"I want this device to be like that one" moves a state onto a
**different machine**. Most of it should follow. A small, named part must
not — and it is exactly the part that says *which machine this is*.

Two machines carrying one identity is a fault that is **silent on the
day it is made** and very hard to find six months later: two hosts
answering to one name, two static addresses colliding on one network,
two devices presenting one key.

The line is drawn by one sentence:

> **The PLAN holds wishes. An identity is not a wish.**

So identity does not live in the plan at all — not in a field that is
skipped on restore, not in a line that is filtered out. It lives in
`system/`, **outside the generations**, where a plan cannot carry it even
by accident. The step checks that: the machine-id does not appear
anywhere inside `system/generations/`.

### The table

| | travels | why |
|---|---|---|
| applications, kernel, `arch` | **yes** | the wish, and what fulfils it |
| sources and their public keys | **yes** | whose packages this machine is made of |
| `timezone`, `keymap`, `time.offset` | **yes** | the person, not the machine |
| `net.mode=dhcp` | **yes** | a policy; two machines may both ask for a lease |
| preferences, accounts | **yes** (unless `--no-personal`) | the person |
| `config/`, `state/` | **yes** (unless `--no-data` / `--no-personal`) | the work |
| **`system/machine-id`** | **NEVER** | generated fresh on every restore |
| **`system/device.key` / `.pub`** | **NEVER** | generated fresh, mode 0600 |
| **`hostname`** | **no**, unless `--keep-identity` | two machines answering to one name |
| **`net.address` / `netmask` / `gateway`** (static) | **no**, unless `--keep-identity` | a fixed address is a claim on a network, and two machines cannot both make it |
| `net.mode=static` | **no**, with the above | without its address the mode describes nothing |
| `cache/` | **no** | regenerates, by definition |

`--keep-identity` exists for the case where the old machine is gone for
good. It carries the hostname and the address; it does **not** carry the
machine-id or the device key, and nothing does. Measured: three trees,
**three distinct machine-ids**.

---

## 5. The measurement

A device with 4 applications, a kernel, an account with a password, six
settings, three preferences and 340 KB of documents was written to a
stick and restored onto an **empty** root, with the source **moved
away**.

> compared **entry for entry over 64 entries**, with the documents
> included: the **only** differences are the PLAN (**921 → 768 octets**,
> five settings shorter) and the two files those settings rendered,
> `etc/hostname` and `etc/netz.conf`

Everything else — every package, every bundle, the kernel, the
appearance, the settings that are not a machine, and every document by
content — is identical.

**The counter-check that makes this a measurement rather than an
impression:** the same stick restored again with `--keep-identity` gives
a tree that is **identical over all 64 entries**. So the differences
above were exactly three, not roughly three.

Two more that are asserted rather than assumed: the documents really are
in the comparison (`users/justin/state/editor/roman.txt`, compared by
SHA-256 — a comparison that skipped them would go green on losing all of
them), and the **cache directory exists while its 300 000 octets of
contents do not**.

---

## 6. Partial moves

| | what it is for | measured |
|---|---|---|
| `--no-personal` | the device goes to somebody else | 3 programs, **0** documents, **0** accounts, **0** preferences — and the plain settings like the timezone stay |
| `--no-data` | same person, a fresh start | 3 preferences, 4 settings files, **0** documents |

`--no-personal` on `stick-write` means the documents are **not on the
stick at all** — nothing to leak, rather than nothing restored. That is
asserted separately, because "not restored" and "not present" are very
different promises when the stick is in somebody else's hand.

---

## 7. The wrong machine

An x86-64 stick at an ARM device is refused, and told why:

```
opk: this stick holds a x86_64 system and you are setting up a aarch64
device. Nothing on it can run here: the packages are machine code for
another machine, and a backup cannot translate them. What CAN be carried
over is the plan itself -- the settings, the accounts and the list of
applications by NAME -- but every package has to come from a aarch64
source. This is refused rather than half done.
```

Nothing is built. The message also says what *is* still possible, which
is the useful half: the plan travels, the packages must be fetched for
the new machine.

---

## 8. What is not solved here

* **Nothing in Osum reads `machine-id` or the device key yet.** They are
  generated, kept and regenerated correctly; no running program asks for
  them. The same honest state as `hostname` and `keymap` in
  [PLAN-FORMAT.md](PLAN-FORMAT.md) — recorded and rendered, waiting for a
  consumer. They exist now so that identity is created in one place
  rather than lazily invented in three later.
* **A stick is not a disk.** Writing one onto real hardware so that it
  boots is round INSTALL's work. This produces the tree; something else
  has to put it on a partition and make it start.
* **There is no user interface.** Round TRESOR is building "back up to
  here" in the file explorer. `stick-write` and `stick-restore` are the
  commands underneath it and use the same data — but somebody still has
  to type them.
* **`--keep-identity` is a loaded gun.** It does what it is asked and
  says so once. Nothing checks whether the old machine is really gone,
  and nothing can.
* **A restore lands on the current state, not the history.** The stick
  carries one PLAN. The new device can go forward but not back past the
  moment it was set up.
