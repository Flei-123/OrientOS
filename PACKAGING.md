# Software distribution in OrientOS

A design decision, settled. This document describes the **goal** and gives the
reasons for it; of the distribution model itself (store, generations, three
data pools) **nothing** is built yet — the file system for it is missing. What
the kernel brings along as a foundation is in § 7 and has been more than a
declaration of intent since phase 3.

---

## 1. The four rules

1. **An app is a single, immutable directory, named after the hash of its
   content.** After the installation nothing about it is ever changed again.
   Same hash = bit-identical software, everywhere.
2. **User data lies strictly separated in three pools per app:**
   `config` (backup), `state` (backup), `cache` (**never** backup, deletable
   at any time).
3. **Uninstalling means: delete two directories.** Leftovers are impossible by
   construction, because there is no place at all where they could lie.
4. **There is no global registry. Never.** No central state that an
   installation can damage.

---

## 2. What that looks like

```
/store/
  8f3a…c1/                      immutable, name = hash of the content
    app.toml                    name, version, required handles
    bin/editor
    lib/…                       only what this app really needs
  2b71…9d/                      another app — possibly the same lib, then the
  …                             same hash entry, so only once on the disk

/apps/
  editor  ->  /store/8f3a…c1    readable name, points at one version

/users/justin/
  config/editor/                settings             → backup
  state/editor/                 documents, session   → backup
  cache/editor/                 thumbnails, index    → NEVER backup
```

Uninstalling:

```
rm -r /users/justin/{config,state,cache}/editor   # user data
rm /apps/editor                                   # the reference
# /store/8f3a…c1 disappears as soon as nobody points at it any more (GC)
```

There is no fifth place. No `/etc` scraps, no registry keys, no services that
have registered themselves somewhere, no `~/.something` litter.

---

## 3. Why three pools and not one

The difference is not tidiness but a **backup and restore decision**:

| pool | content | backup | what loss means |
|---|---|---|---|
| `config` | settings, key bindings, accounts | yes, small, versionable | annoyance |
| `state` | documents, databases, history | yes, this is the valuable part | data loss |
| `cache` | thumbnails, indexes, compiled shaders | **no** | nothing, it is regenerated |

Where this separation is missing, backups carry browser caches along for
years, and nobody knows whether a directory may be deleted. The app does not
have to manage the separation itself — at startup it gets **three handles**
and cannot do anything but use them (see § 7).

The expected objection: *apps will not stick to it.* That is exactly why it is
not a convention but the only available interface. An app without a handle on
`state` has no way to put anything there.

---

## 4. Why content-addressed

* **Reproducible.** Same hash = same content. "It runs on my machine" becomes
  checkable instead of an excuse.
* **Dedup for free.** Two apps with the same library version share one store
  entry. The memory cost of "every app brings everything with it" goes away,
  without one app being able to swap the library out from under another.
* **No DLL / dependency hell.** Two versions of the same library can exist at
  the same time. There is no shared namespace in which they could quarrel.
* **Atomic.** An installation is: write a new store entry, then move **one**
  reference. There is no half-installed state.
* **Checkable.** A scan over the store detects every change to installed
  software — without antivirus signatures, purely by computing.

---

## 5. System generations

Every system change (update, installation, configuration change) produces a
**new generation**. The old one stays complete and bootable.

```
/system/generations/
  41/   ← current
  40/   ← previous, bootable
  39/
```

* **Rollback in seconds**: pick a boot menu entry or move one reference. No
  restoring, no undoing of changes.
* **Updates lose their terror.** The biggest risk of an update is not the bug
  but that you cannot get rid of it again.
* **Cleaning up is explicit**: old generations stay until somebody deletes
  them; after that the store GC clears out unreachable entries.

The precondition for this is a file system with **copy-on-write snapshots** —
the reason why the file system of our own in [FILESYSTEM.md](FILESYSTEM.md)
lists CoW as a mandatory feature and not as an extra.

---

## 6. Models — and what we take from them

| system | adopted | not adopted |
|---|---|---|
| **Nix / Guix** | content addressing, generations, atomic update, GC | the language and the complexity of the derivations; OrientOS does not need a build system of its own to install an app |
| **macOS `.app`** | one app = one directory, deleting = throwing away | the exceptions (`~/Library` litter, `/Library/LaunchDaemons`, installer packages) |
| **Android** | strict data separation per app, an app gets only what it may have | the Play Services dependency and asking for permissions at runtime |
| **GoboLinux** | a readable directory structure instead of historically grown paths | compatibility symlinks into `/usr/bin` — that would be the back door that softens everything again |

---

## 7. What this has to do with the kernel

The decisive point, and the reason why this stands here and not in a userspace
document: **this model is only enforceable when the kernel knows no ambient
authority.** On a POSIX kernel it would be cosmetics — every program could
open `/etc` and undercut the separation.

Three kernel properties are a precondition, and all three are built that way —
and since phase 3 not only as types but as running code with negative tests in
the boot log (handle table per process with slot + generation + per-process
random value, `kernel/src/abi/native.rs`, `libs/osum-abi-native/src/table.rs`):

1. **No global path namespace in the core.**
   `osum-native` has no `open("/path")`. There is only
   `NamespaceOpen(namespace_handle, name, rights)` — resolved **relative to a
   handle** that the process owns. A process without a namespace handle on
   `/store` cannot even name `/store`.
   → `libs/osum-abi-native/src/syscall.rs`, `Syscall::NamespaceOpen`
2. **No ambient authority.**
   Every access needs a `Handle` with `Rights`. Rights can only be **reduced**
   when passing them on (`Rights::restrict`), never enlarged.
   → `libs/osum-abi-native/src/rights.rs` (host-tested:
   `rights_can_only_shrink`)
3. **No `fork`.**
   `fork` inherits the complete address space and all descriptors — the exact
   opposite of explicit passing of rights. OrientOS has only
   `ProcessSpawn(image, namespace, handles[])`: the parent process **lists**
   what the child gets. In the POSIX layer `fork` is permanently `-ENOSYS`.
   → `libs/osum-abi-posix/src/lib.rs`, `sys_fork_unsupported()`
   On the kernel side this is built as `spawn`/`spawn_with` in
   `kernel/src/abi/native.rs`: a fresh handle table is **empty**, the child
   gets only handles passed by name, and rights can only get smaller in the
   process. Demonstrable in the boot log (`Prozessprobe: spawn ohne fork,
   Kind "dienst" mit 1 explizit uebergebenen Handle(n)`), together with the
   counter-check `Schreiben ohne Recht -> RightsDenied`. As a syscall from
   ring 3, `ProcessSpawn` is still `NotSupported` — that comes with the
   per-process address spaces.

Starting an app therefore looks like this:

```
ProcessSpawn(
    image     = <handle on /store/8f3a…c1/bin/editor, EXEC only>,
    namespace = <handle on /store/8f3a…c1, READ only>,
    handles   = [ config:  READ|WRITE,
                  state:   READ|WRITE,
                  cache:   READ|WRITE|CREATE,
                  konsole: WRITE ]
)
```

After that the app has **no means** of reaching the user's file system, other
apps or system directories. Not because a policy forbids it, but because it
cannot name the objects. File selection happens as on Android/macOS through a
trusted service that hands back a **handle on exactly one file** — not through
a path the app opens itself.

---

## 8. Open questions (honestly)

* **Store GC**: reference counting or reachability analysis? Reachability is
  more robust but needs a pass over all generations.
* **Update without a restart**: a running app holds handles on its store
  entry. Let it run until it exits — or offer migration? Probably the former,
  it is more honest.
* **Services in the background**: who starts them, with which handles, and how
  do you prevent that from turning into a global `/etc` after all?
* **Signatures**: the hash proves that nothing was altered, not where it came
  from. A signature over the hash, checked by whom, with which keys?
* **Size**: dedup over hashes is coarse (whole files). Block-level dedup in
  the file system would be finer — but then the file system has to be able to
  do it.

These questions get answered when phase 4 (VFS, file systems) stands — not
before. The kernel first, then the model on top of it.
