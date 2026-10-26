# Renaming the operating system

The product name is a **configuration, not a property of the code**
([BRANDING.md](BRANDING.md)). Renaming means: one brand file, one default.

## The short way

```sh
./rename.sh <os-name> [slug]
# example:
./rename.sh Novaos nova
```

The script creates `brands/<slug>.toml` (publisher copied from the current
default brand, `web` and `feed` as `https://<slug>.example` placeholders) and
points `brands/STANDARD` at it. Then:

1. set `web` and `feed` in the new file,
2. `./build.sh` - produces `build/<slug>.iso`,
3. `./test.sh` - step 10 builds a probe brand and fails when any known brand
   name (including the new one) shows up in the built output.

Without `slug` it is derived from the name: lower case, trailing `os` removed
(`Novaos` -> `nova`).

| | format | example |
|---|---|---|
| os-name | `^[A-Za-z][A-Za-z0-9-]{0,30}$` | `Novaos` |
| slug | `^[a-z][a-z0-9-]{0,30}$` | `nova` |

## Without a script

Create `brands/<slug>.toml` by hand, write `<slug>` into `brands/STANDARD`.
For a single build: `./build.sh --brand <slug>`.

## What stays when the OS is renamed

* the kernel name `osum` (file `osum.mb`, `/boot/osum`) - only `kernel-name`
  in the brand file changes it, and it is normally left out,
* format ids, ABI numbers and crypto labels (`OSUM-OFS`, `OSUMMOD`,
  `SSH-2.0-OsumSSH`, `osum_panic`, key-derivation labels): they are
  interfaces and data formats, not product names. Changing them makes existing
  disks, backups and signatures unreadable. The full list with reasons is in
  [BRANDING.md](BRANDING.md), section "Deliberately kept hits".

## Renaming the kernel

The kernel lives in the Osum repository and is not renamed here. A brand that
wants another kernel file name sets `kernel-name`; the Osum repository itself
keeps its name.
