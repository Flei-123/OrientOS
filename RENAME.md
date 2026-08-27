# Renaming the kernel and the operating system

> **First check whether you need this at all.** For a **second brand**
> (same source text, different product) [BRANDING.md](BRANDING.md) is the
> right way: `./build.sh --brand xoffi` leaves the tree untouched.
> `rename.sh` is for the **final** rename of the project — it touches
> directories, Cargo names and documentation.

The name is a **configuration, not a property of the code**. Whoever wants to
keep the final name open should be able to do that without refactoring.
Therefore the following holds in this project:

> **No product name appears in the source text.** It comes exclusively from
> `kernel/src/kcore/branding.rs`, fed from the Cargo metadata.
> `./test.sh` step 14 fails the build when somebody gets around that.

---

## The short way (under 15 minutes, usually under 2)

```sh
./rename.sh <new-kernel-name> <new-os-name>
# example:
./rename.sh nova Novaos
```

The script does everything and **checks itself**: at the end it builds the
kernel and boots it in QEMU. When `ERGEBNIS: alle Pruefungen bestanden.`
appears, the rename is complete.

Rules for the names:

| | format | example |
|---|---|---|
| kernel name | `^[a-z][a-z0-9-]*$` (a valid Cargo package name) | `nova` |
| OS name | `^[A-Za-z][A-Za-z0-9-]*$` | `Novaos` |

**Verified:** `./rename.sh nova Novaos` was run in a copy under `/tmp`. Result:
`x86_64-nova-none.json`, `libs/nova-mem`, `libs/nova-abi-native`,
`libs/nova-abi-posix`, boot log
`[nova] boot nova v0.1.0 — Kernel von Novaos`, all checks passed.

---

## What the script does in detail

1. **Read out the old names** — do not guess them:
   * the kernel name from `kernel/Cargo.toml`, `name = "…"`
   * the OS name from `kernel/Cargo.toml`,
     `[package.metadata.branding] os-name = "…"`
2. **Rename directories and files**
   * `libs/<old>-mem`, `libs/<old>-abi-native`, `libs/<old>-abi-posix`
   * `x86_64-<old>-none.json` (and `.VERIFIED`)
3. **Text replacement across all text files** (excluding `.git/`, `target/`,
   `vendor/`, `build/`, ISO and log files). The order matters, because the OS
   name contains the kernel name as a substring (`orientos` ⊃ `osum`):
   1. the OS name (`OrientOS`) and its lower-case form (`orientos`)
   2. compounds (`osumfs`)
   3. Rust module paths (`osum_mem` → `nova_mem`)
   4. hyphenated forms (`osum-abi-native`)
   5. the name on its own, with word boundaries (`\bkarst\b`)
4. **Delete `Cargo.lock`** (it is recreated on the next build, otherwise it
   refers to packages that no longer exist)
5. **Counter-check**: `./build.sh` and `./run-qemu.sh --check`

---

## By hand — in case the script does not fit for once

```sh
ALT_K=osum; ALT_OS=OrientOS
NEU_K=nova;  NEU_OS=Novaos

# 1. directories
for s in mem abi-native abi-posix; do mv libs/$ALT_K-$s libs/$NEU_K-$s; done
mv x86_64-$ALT_K-none.json          x86_64-$NEU_K-none.json
mv x86_64-$ALT_K-none.json.VERIFIED x86_64-$NEU_K-none.json.VERIFIED

# 2. text (mind the order!)
find . -type f -not -path './.git/*' -not -path './target/*' \
       -not -path './vendor/*' -not -path './build/*' \
  -exec perl -pi -e "
      s/\Q$ALT_OS\E/$NEU_OS/g;
      s/\Qorientos\E/\L$NEU_OS/g;
      s/\b\Q$ALT_K\Efs\b/${NEU_K}fs/g;
      s/\b\Q$ALT_K\E_/${NEU_K}_/g;
      s/\b\Q$ALT_K\E-/${NEU_K}-/g;
      s/\b\Q$ALT_K\E\b/$NEU_K/g;
  " {} +

# 3. lock file away, rebuild, check
rm -f Cargo.lock && ./build.sh && ./run-qemu.sh --check
```

---

## Changing only the OS name (the kernel name stays)

One line in `kernel/Cargo.toml`:

```toml
[package.metadata.branding]
os-name = "Novaos"
```

Or without changing a file, just for one run:

```sh
OS_NAME_OVERRIDE=Novaos ./build.sh && ./run-qemu.sh --check
```

---

## Where the name in the binary comes from

```
kernel/Cargo.toml
   name = "osum"                      ─┐
   [package.metadata.branding]         │
   os-name = "OrientOS"               ─┤
                                       │  reads
OS_NAME_OVERRIDE (environment, optional)┤
                                       ▼
kernel/build.rs   ──  cargo:rustc-env=BRANDING_KERNEL_NAME / BRANDING_OS_NAME
                                       │
                                       ▼
kernel/src/kcore/branding.rs
   KERNEL_NAME · OS_NAME · VERSION · LOG_TAG · NATIVE_ABI · banner()
                                       │
                                       ▼
   klog!(), panic handler, boot banner, ABI description — everything in the tree
```

`build.rs` gets by **without a TOML crate**: it looks for three lines by text
comparison. A build dependency for that would be exactly the ballast this
project avoids.

---

## What the script does **not** touch — and why

| | reason |
|---|---|
| `vendor/limine/` | foreign code, it is not ours |
| `target/`, `build/` | throwaway artefacts, they are recreated |
| `.git/` | history stays history |
| ISO and log files | binary, or snapshots of old runs |

After the rename the old ISO is still in `build/`. `./build.sh` overwrites it
on the next run.

---

## Edge cases

* **The new name is a prefix of the old one** (`osum` → `kar`): works, because
  all rules use word boundaries.
* **The new OS name contains the new kernel name** (`nova` → `Novaos`):
  intended and tested — the OS rule runs before the kernel rule.
* **Renaming twice in a row**: works, because the old names are read freshly
  out of `kernel/Cargo.toml` each time.
* **Renaming with a dirty working directory**: commit first. The script changes
  very many files; without a clean state the diff is unreadable.
