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
| documents | `users/<who>/state/<app>/` | **yes** | this is the valuable part |
| programs | `store/<hash>/` | **no** | reproducible from the PLAN + a source |
| the activated view | `apps/<name>.prog/` | **no** | second names on the store, rebuilt on every activation |
| generated config | `etc/…` | **no** | a function of the PLAN, rewritten on every activation |
| thumbnails, indexes | `users/<who>/cache/<app>/` | **no** | regenerated, by definition |

Everything in the “no” column is either **derived from the PLAN** or
**derived from use**. Backing it up would be backing up an output.

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
  `state/` — and guarantees that nothing else on the machine is
  irreplaceable.
* That side may store it however it likes. Block-level deduplication is
  the answer to the one open question PACKAGING.md § 8 left standing
  (“dedup over hashes is coarse — whole files”), and it is a good answer,
  but it changes nothing about *which* files are handed over.

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
  local paths work; that is what was measured.
