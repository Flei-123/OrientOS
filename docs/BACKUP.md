# What a backup of OrientOS is

*Round PLAN2, 27.08.2026. Every number below comes from one measured
tree; the script that produced it is in
[ROUND-PLAN2.md](ROUND-PLAN2.md) § “The backup set, measured”.*

---

## 1. The rule

> **A complete backup is `PLAN` + `config/` + `state/` — and the secret
> store. Programs and `cache/` are never backed up.**

That is the whole rule. It is not a policy anybody has to remember, it is
a consequence of how the tree is built:

| what | where | in a backup? | why |
|---|---|---|---|
| the system state | `system/generations/<n>/PLAN` | **yes** | one text file, ~1 KiB, and it *is* the machine |
| credentials | `system/secrets/<name>` (0600) | **yes** | not in the PLAN by construction — see below |
| settings per app | `users/<who>/config/<app>/` | **yes** | small, and losing it is annoyance |
| appearance per person | `users/<who>/config/desktop/` | **yes**, and also **reproducible** — see below | the plan names it too, so a restore has two independent sources that must agree |
| documents | `users/<who>/state/<app>/` | **yes** | this is the valuable part |
| programs **with a source** | `store/<hash>/` | **no** | reproducible from the PLAN + a source |
| programs **with no source** | `store/<hash>/` | **yes**, as a named exception | orphaned — see § 1a. The only copy in the world. |
| the activated view | `apps/<name>.prog/` | **no** | second names on the store, rebuilt on every activation |
| generated config | `etc/…` | **no** | a function of the PLAN, rewritten on every activation |
| thumbnails, indexes | `users/<who>/cache/<app>/` | **no** | regenerated, by definition |

Everything in the “no” column is either **derived from the PLAN** or
**derived from use**. Backing it up would be backing up an output.

## 1a. The one exception: programs with no source

*Third addendum, 27.08.2026. The owner asked: "what about programs that
are not in the app store, when I set up a new device and want my
applications back?"*

It was a real hole. A PLAN names `app <name> <hash>`; the octets have to
come from somewhere; and for a package built by hand and installed from a
file there was no somewhere.

### The main road is a second source, not an exception

A plan may name **any number** of sources, each with its own Ed25519
public key. So the right answer to "I build my own packages" is: **run
your own source.** A directory of `.opk` files with an `INDEX` and a
signature *is* a source — on a NAS, on a server, on a memory stick:

```
opk.py quelle     ~/mypackages --schluessel ~/keys/geheim.key
opk.py source-add --root / file:///mnt/nas/mypackages <public key>
```

Then nothing is orphaned and none of the machinery below ever runs.
Measured: one `source-add` of a private directory takes a tree from
`2 of 3 covered by a source, 1 ORPHANED` to `3 of 3 covered, 0 ORPHANED`,
and the backup set stops carrying the octets in the same breath.

### Orphanhood is decided, not remembered

A hash is **covered** if some recorded source's INDEX contains it, and
**orphaned** if none does. That is a computation over the plan and the
sources — nobody keeps a list, nobody has to remember which of their
programs are irreplaceable:

```
$ opk.py orphans --root /
generation 5, 3 hash(es) named by the plan
   source file:///…/appstore: 2 package(s), signed by a6594ee4849d3329...
  ok       hallo                        bc9c62d6877b  x86_64   from appstore
  ok       the theme of justin          45e6577d4bc4  any      from appstore
  ORPHANED mytool                       5eff0f81b5cf  x86_64   4146 octet(s) in the store
2 of 3 covered by a source, 1 ORPHANED
```

`--strict` exits 1 while anything is orphaned, so this can be a nightly
check instead of a habit.

Two honest details. A source that **cannot be reached from here** is not
the same as a source that does not have the package: `orphans` names it
as unusable, `backup-set` writes a `# unreachable` line into the file,
and `vault-export` **refuses outright** rather than archive packages that
are perfectly fetchable from somewhere else. And an orphan for **another
machine** is worse than an orphan: the report says so, because an
x86-64 orphan cannot be put on an ARM device even with the octets in
hand.

### What that means for a backup

Exactly the orphans are carried, and only they. Everything with a source
is fetched on restore and would be dead weight; everything without one is
the only copy that exists. The rule in § 1 is unchanged — it gains one
exception, and the exception is decided by a machine.

## 2. Why this is now true and was not before

Before round PLAN2 the rule could not be stated honestly, because the
PLAN did not describe the machine. It listed the applications. Restoring
`PLAN + config + state` onto a fresh disk would have given you the
applications back and left the kernel, the network, the time and the
accounts to be set up by hand — which is not a restore, it is a
reinstall with the documents copied over.

Now the PLAN carries the kernel, the sources with their public keys, the
settings and the accounts, and `opk.py rebuild` turns it back into a
tree. What a backup has to carry is therefore exactly what the PLAN
*cannot* determine: the work, and the passwords.

Measured on a tree with 9 applications, a kernel, 7 settings, one account
and some user data:

```
full tree, distinct inodes   3 571 511 octets,  71 files
  store/ (programs)          2 368 499 octets   ← never backed up
  apps/  (second names)        727 304 octets   ← never backed up
  etc/   (generated)               126 octets   ← never backed up
  users .../cache            1 150 000 octets   ← never backed up
  users .../config               3 000 octets
  users .../state               40 000 octets
  PLAN                           1 046 octets
  system/secrets                    30 octets

BACKUP SET = 44 076 octets = 1.23 % of the tree
NEVER BACKED UP = 3 518 499 octets
```

Ninety-nine per cent of the bytes on that machine are things a backup has
no business carrying. That ratio is the argument for the whole design,
and it is a measurement rather than a claim.

### The one place where backup and plan overlap

`users/<who>/config/desktop/{theme,wallpaper,taskbar.conf}` is **both**
inside the backup set (it lives under `config/`) **and** determined by
the plan (`pref` lines, with the blobs content-addressed in the store).
That looks like a contradiction of the rule above and is not — it is
redundancy with a checker:

* restore from the **plan** and a source: the appearance comes back
  because the theme and the wallpaper are asset packages;
* restore from the **backup**: it comes back because the octets are in
  `config/`;
* `opk.py verify` requires the two to agree, by asserting that
  `users/<who>/config/desktop/wallpaper` is the *same inode* as
  `store/<hash>/blob`.

It costs nothing in bytes — the file has one inode and several names —
and it means a wallpaper survives both a lost backup and a vanished
source. That is deliberate: everything else in the "no" column is
reproducible from *one* place, and appearance is the one thing a person
notices the moment it is wrong.

## 3. The passwords

`system/secrets/<name>` is `0600` and holds the credential in the form
`kernel/user/pw.fi` reads (`$osum1$<rounds>$<salt>$<derived>`,
PBKDF2-HMAC-SHA256). It is **not** in the PLAN — a PLAN is meant to be
handed to another machine, and a credential in it would be a credential
in every copy of that file.

What the PLAN carries instead is the **SHA-256 of the credential file**.
Two things follow:

* A restore that has the secret store can be checked: `opk.py rebuild
  --secrets D` recomputes the hash and **refuses** a credential that does
  not match what the plan was written with. A silently wrong password is
  not a possible outcome.
* A restore *without* the secret store produces the accounts **locked**
  (`justin:!:0:0:99999:7:::` in `/etc/shadow`). Measured: such a rebuild
  differs from the original in exactly **one** file, `/etc/shadow`, and
  in nothing else.

So the secret store belongs in the backup, at the same protection level
as the rest of it, and its absence degrades gracefully and visibly rather
than silently.

## 4. What restores mean

| you have | you get back |
|---|---|
| `PLAN` + a reachable source | the exact system: kernel, applications, settings, accounts (locked) — byte for byte |
| … + `system/secrets/` | the same, with the passwords |
| … + `config/` + `state/` | the same machine, with the work on it |
| `config/` + `state/` alone | the documents, and a machine to be rebuilt by hand |

The first row is the one this round measured. `opk.py rebuild` on an
empty directory, from a 745-octet plan and a signed source, produced a
tree that `opk.py snapshot` found **identical over 39 entries, 18 files
and 3 470 309 octets** — comparing kind, path, mode, size and the SHA-256
of every content.

## 5. Where this meets the TRESOR round

Round TRESOR is building `/bin/backup` inside Osum: content-addressed,
SHA-256 blocks, deduplication. This document describes **what** to hand
it; that round describes **how** the bytes get stored. The two do not
block each other, and the interface between them is deliberately thin:

* This side names the set — `PLAN`, `system/secrets/`, `config/`,
  `state/`, **plus the orphaned packages** — and guarantees that nothing
  else on the machine is irreplaceable.
* That side may store it however it likes. Block-level deduplication is
  the answer to the one open question PACKAGING.md § 8 left standing
  (“dedup over hashes is coarse — whole files”), and it is a good answer,
  but it changes nothing about *which* files are handed over.

### The exact interface: `opk.py backup-set`

So that TRESOR does not have to know what a plan, a source or an orphan
is, `opk` answers the whole question in one file. It is the format of
everything else in this system: one tab-separated line per item, a type
in the first field, `#` for comments, readable with `cat`.

```
$ opk.py backup-set --root / -o /tmp/SET
# opk backup-set, root /, generation 5
# Everything NOT listed here is deliberately not backed up: programs with a source,
# the activated view under apps/, generated files under etc/, and every cache/.
plan	system/generations/5/PLAN
secret	system/secrets/justin
tree	users/justin/config
tree	users/justin/state
package	5eff0f81b5cf…14d7	mytool	x86_64	store/5eff0f81b5cf2844b9da
```

| type | fields | what `backup` does with it |
|---|---|---|
| `plan` | path | copy the file |
| `secret` | path | copy the file, **keep mode 0600** |
| `tree` | path | copy recursively |
| `package` | sha256, name, arch, path | copy that store directory — this is the orphan exception |
| `# unreachable` | source, reason | **a warning, not an item**: this host could not check that source, so the list may name packages as orphaned that are not |

All paths are relative to the root. A backup program needs no other
knowledge of OrientOS, and it must never decide what is worth keeping —
that decision is made where the plan is, and it is made by computation.

A companion command writes the orphan octets out as ordinary `.opk`
files, if TRESOR would rather have files than store directories:

```
$ opk.py vault-export --root / -o /mnt/backup/vault
  mytool                       5eff0f81b5cf      4228 octet(s) -> 5eff0f81b5cf2844b9da.opk
1 orphaned package(s), 4228 octet(s) in /mnt/backup/vault
```

A store entry is held **unpacked**, so this packs it again and refuses if
the result is not the very same hash — nothing enters an archive that
cannot later be checked against the plan that will ask for it. Measured:
the exported file is the original `.opk` back, octet for octet.

### And why the vault needs no signature, although a source does

The two are asked different questions, and that is the whole of it:

> a **source** is asked *"give me the package called `explorer`"* — the
> answer is a **choice somebody else makes**, and a name says nothing
> about content, so the answer must be signed.
>
> a **vault** is asked *"give me the octets whose SHA-256 is `abc…`"* —
> the answer **checks itself against the question**. A signature would
> add nothing the hash has not already settled.

That is why `opk.py rebuild --vault` takes a plain directory and no key,
and why that is not a hole in the design.

### Restoring with something missing

If a package can be fetched from neither a source nor a vault, `rebuild`
**refuses**, and names it:

```
1 of 3 package(s) can be fetched from NOWHERE:
   MISSING  mytool                       5eff0f81b5cf2844
opk: refusing to build a system that is missing 1 of its 3 package(s).
```

Nothing is built — a half-tree that looks finished is worse than no
tree. `--allow-missing` builds the rest, and then the **tree itself**
carries the evidence in `system/INCOMPLETE`, because a missing package
mentioned once in a terminal that has since been closed is a missing
package nobody knows about. `opk.py verify` reads that file and **fails**
on it. A complete tree does not have it, so its absence is a statement
and not a default.

Two things are worth saying across the fence, without waiting for it:

1. **The store is already content-addressed and already deduplicated at
   file granularity.** A backup tool that also walks `store/` would be
   deduplicating a deduplicated tree — and storing 2.4 MiB that
   `opk.py rebuild` can fetch from a signed source instead. `store/` and
   `apps/` should be *excluded by name*, not merely deduplicated away.
2. **`apps/` would be counted twice by anything that does not follow
   inodes.** Naively summed, the measured tree is 5 938 071 octets; by
   distinct inode it is 3 571 511. A backup tool that copies second names
   as separate files inflates a backup by 66 % on this tree.

## 6. What is not solved here

* **User data does not roll back.** A generation describes the system,
  not the documents: a jump back to yesterday must not take today's work
  away. The cost is stated where it hurts — `entfernen` really does throw
  the three pots away and a later `zurueck` does **not** bring them back
  (`--behalte-daten` opts out). Rolling user data back needs snapshots in
  the file system, which OFS does not have.
* **There is no scheduler, no rotation, no off-machine copy.** This
  document defines a set, not a backup program.
* **`https://` sources are not implemented**, so “restore from the
  internet” is not something this repository can do today. `file://` and
  local paths work; that is what was measured. This is felt twice as hard
  since the third addendum: a private source on a NAS is the recommended
  answer to orphaned packages, and reaching one over the network is
  exactly what is missing.
* **An orphan for the wrong machine cannot be rescued at all**, and the
  error says so instead of pretending. The octets are the only ones in
  the world and they are for another architecture; the way out is the
  source code, not the backup. Nothing here can change that.
* **`vault-export` is not a backup program.** It writes files into a
  directory. Scheduling, rotation and getting them off the machine are
  round TRESOR's.
