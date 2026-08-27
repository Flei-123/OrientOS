# File systems in OrientOS

Order and reasoning. Nothing of it is built in phase 1 yet — this document
settles **in which order** things get built and why exactly that way.

---

## The order

| step | what | why at this place |
|---|---|---|
| **1** | **VFS layer**, object/handle based, FS drivers behind a trait | without a clean boundary the first driver cements its semantics into the whole system |
| **2** | **FAT32 reading and writing** | mandatory: the EFI System Partition is FAT. Without FAT there is no bootloader of our own and no update of our own system on UEFI |
| **3** | **ext4 reading only** | interoperability: fetch data off existing Linux disks without endangering them |
| **4** | **a file system of our own** (working title `osumfs`) | only once everything above it stands and you know what you really need |

---

## 1. VFS: object and handle based, **no** POSIX semantics enforced

The VFS is the place where most systems saddle themselves with POSIX forever.
Linux' `struct inode`/`struct dentry` are not just an implementation, they are
a **contract**: every file system has to pretend that it has inodes, hard
links, `..` entries, `mode_t` bits, timestamps at a certain resolution and a
global root tree. FAT has none of that and is therefore bent into shape with
emulation layers everywhere.

OrientOS does it the other way round:

* A file system delivers **objects behind handles**, not inodes.
* The trait boundary demands only what every file system really can do: open
  relative to a directory handle, read, write, size, list, create, delete,
  rename, `sync`.
* Everything beyond that is **optional and queryable** instead of assumed:
  hard links, symlinks, extended attributes, nanosecond timestamps, snapshots,
  checksums. A driver reports its capabilities; whoever wants more asks first.
* **No global root.** A process is handed a namespace handle (see
  [PACKAGING.md § 7](PACKAGING.md)); there is no process-independent path `/`.
  `..` is therefore not a special operation but simply not there — you cannot
  "climb out" of a handle.
* **POSIX semantics live in the POSIX layer.** That is where paths are
  resolved, that is where `cwd`, `umask`, `mode_t` and `errno` exist. Drop the
  layer and the semantics drop with it — the VFS does not notice.

Concretely that means for the trait (a draft, not implemented yet):

```rust
pub trait FileSystem {
    fn capabilities(&self) -> FsCaps;                 // what this FS really can do
    fn root(&self) -> NodeId;                         // root of THIS file system
    fn open(&self, dir: NodeId, name: &str, rights: Rights) -> Result<NodeId>;
    fn read (&self, node: NodeId, off: u64, buf: &mut [u8]) -> Result<usize>;
    fn write(&self, node: NodeId, off: u64, buf: &[u8])     -> Result<usize>;
    fn list (&self, dir: NodeId, cb: &mut dyn FnMut(&DirEntry)) -> Result<()>;
    fn create(&self, dir: NodeId, name: &str, kind: NodeKind) -> Result<NodeId>;
    fn unlink(&self, dir: NodeId, name: &str) -> Result<()>;
    fn sync(&self) -> Result<()>;
}
```

No `stat` with 20 fields, 15 of which are lies. Whoever wants metadata asks
for them specifically and gets `NotSupported` when they do not exist.

---

## 2. FAT32 first — not out of nostalgia, out of necessity

The **EFI System Partition is FAT**. That is not a convention, it is in the
UEFI specification. Without write access to FAT, OrientOS cannot:

* install its own bootloader,
* enter its own system generations into the boot path,
* update itself.

On top of that: FAT is on every USB stick, in every camera, on every SD card.
It is the lowest common denominator for exchanging data with the world.

At the same time FAT is the perfect **counter-example** for the VFS design: no
inodes, no hard links, no permissions, the 8.3 legacy with an LFN add-on,
timestamps in steps of 2 seconds. If the VFS trait carries FAT without
contortions, it is not secretly POSIX. **FAT is the litmus test.**

---

## 3. ext4 reading only — deliberately cut down

Writing ext4 support would mean: keep the journal correctly, rebuild extent
trees, handle orphan lists, reproduce `e2fsck` semantics. One mistake in there
destroys foreign data — data that belongs to the user and that he did not
entrust to us but to Linux.

Reading gives almost the same benefit (getting data across) at a fraction of
the risk. Write access comes at the earliest once a test corpus of our own
with an `e2fsck` counter-check exists — and honestly probably never, because
the benefit does not justify the effort.

---

## 4. Our own file system — last, and deliberately so

### Why not first?

Because file systems are the component in which mistakes are
**irreversible**. A crashing kernel costs a reboot; a file system with a
mistake in the write ordering costs the data.

Two examples nobody should ignore:

* **btrfs RAID 5/6.** Announced, built, looking usable — and afflicted with a
  "write hole" weakness that can lead to silent data loss on a power failure
  during a stripe update. The official warning not to use it in production has
  stood for **over a decade**. It is not enough for a file system to work in
  the normal case.
* **ZFS.** From the first drafts at Sun (around 2001) to broadly trustworthy
  maturity took **years**, with a paid team that did nothing else. Checksums,
  CoW, snapshots, resilvering — every single feature is easy to describe and
  hard to get right.

A file system of our own is therefore not a weekend project and must not lie
on the critical path to "OrientOS boots and is usable". Until then FAT32 and
an initramfs will do — the latter has been **built** since phase 3 and already
loads the first unprivileged program (see § 5 and ARCHITECTURE.md § 5c).

### Requirements for when it does get built

**Mandatory, because the system model rests on it:**

1. **Copy-on-write with snapshots.** Without it there are no system
   generations and no rollback in seconds ([PACKAGING.md § 5](PACKAGING.md)).
   That is the reason why CoW is not an extra feature here but the
   foundation.
2. **Checksums over data *and* metadata.** Silent data loss is the worst class
   of failure: it only becomes visible when restoring a backup, when the
   backup is already broken too.
3. **Dedup over content hashes.** The app store is content-addressed; the file
   system must then not store the same block several times. Block-wise, not
   file-wise.
4. **Handle-oriented**, matching the VFS: no path namespace as the foundation,
   directories are objects like everything else.

**Legacy questions that get decided cleanly once — and then never again:**

| question | decision |
|---|---|
| 8.3 names | do not exist. Names are names. |
| timestamps | 64-bit nanoseconds since 1970, **no 2038 problem**, no 2-second grid |
| character set | UTF-8, **normalised at creation time** (NFC), rejected when invalid |
| upper/lower case | **case-sensitive, exact**. No "case-preserving, case-insensitive" — that is the origin of whole classes of bugs and turns Unicode comparisons into a time bomb |
| separator | `/` is forbidden in a name, `\0` is forbidden, everything else is allowed |
| maximum name length | 255 bytes of UTF-8, checked hard |
| special files | no device nodes in the file system. Devices are handles, not files |

The point of this table: every one of these questions is answered
**several times and contradictorily** in existing systems (HFS+ vs. APFS vs.
ext4 vs. NTFS). Whoever decides them once and documents them saves himself
twenty years of special-case code.

---

## 5. Order in relation to the roadmap

| phase | precondition for it |
|---|---|
| initramfs (phase 3) ✔ **built** | nothing — it lies in memory. Format `IRFS0002` (header + fixed-width table, CRC32 per entry), packed by `vendor/osum/mkfs.py` (until the kernel switch: `userland/mkinitramfs.py`, see [tests/GELOESCHT.md](tests/GELOESCHT.md)), as a Limine module in the ISO, read by `kernel/src/kcore/initramfs.rs`. Deliberately **no** cpio/tar: both carry POSIX metadata (mode, uid/gid, paths) that this kernel does not know, and both are stream-oriented instead of random access. 7 negative cases in the boot log |
| VFS + FAT32 (phase 4) | a block device trait, so AHCI/NVMe |
| ext4 reading (phase 4) | the VFS stands |
| own FS (phase 8) | everything above it stands, and there is a test corpus together with a crash simulation |

Before the file system of our own comes a **crash test tool**: artificial
power failures at random points of the write ordering, then a check for
consistency. Without that tool the work does not start. That is the lesson
from btrfs RAID 5/6.
