# OrientOS

The system around the Osum kernel: the repository that assembles a bootable
operating system image from the pinned kernel, a userland, a brand and a
package set. The product is called **XoffiOS**; the kernel is called `osum`
in every edition. The kernel itself lives in its own repository,
[Osum](https://github.com/Flei-123/Osum), written in
[Firn](https://github.com/Flei-123/Firn).

The product name is not hard-coded. It comes from a brand file
(`brands/*.toml`, default named in `brands/STANDARD`), so the same source
builds differently named editions.

## What this repository does

- **Assembles the image.** `build.sh` fetches the pinned Osum kernel and its
  programs, builds an OFS file system from `userland/PROGRAMME`, computes its
  CRC32, and packs kernel and file system with Limine into an ISO that boots
  through BIOS and UEFI.
- **Boots a userland from a module.** The file system travels as a boot module;
  the kernel verifies its checksum and mounts it as the root disk, so
  `/bin/sh` runs from an ISO without any disk.
- **Brands.** `brands/*.toml` describes everything exchangeable (name, slug,
  publisher, web address, package feed). The source text knows none of these
  values.
- **Packages.** `pkg/opk.py` builds and manages content-addressed packages:
  immutable store entries, system generations, rollback, signed sources
  (`bauen`, `installieren`, `entfernen`, `generationen`, `zurueck`, ...).
  The format is described in [PAKETE.md](PAKETE.md).
- **Acceptance suite.** `./test.sh` checks provenance of the kernel, the
  absence of foreign code where there must be none, BIOS and UEFI boots, the
  userland, the brands and the packages. Every check has a counter-check.

The kernel, the desktop and the drivers are documented in the Osum
repository.

## Quick start

```sh
sudo apt install qemu-system-x86 xorriso mtools ovmf python3 binutils build-essential git
# plus rustc/cargo: the pinned Firn compiler is built from source

./build.sh                    # kernel from vendor/osum + userland -> build/<slug>.iso
./run-osum.sh                 # boot through SeaBIOS, serial output in the terminal
./run-osum.sh --uefi          # the same image through OVMF
./run-osum.sh --script 'ls /bin;exit'
./test.sh                     # full acceptance of the system
```

### Scripts

| command | effect |
|---|---|
| `./build.sh` | product ISO `build/<slug>.iso` |
| `./build.sh --brand <name>` | another brand (`brands/<name>.toml`), same source |
| `./build.sh --cmdline "..."` | custom kernel command line |
| `./build.sh --ohne-userland` | counter-check: ISO without the boot module |
| `./build.sh --kaputte-summe` | counter-check: wrong CRC32, the kernel must reject the module |
| `./run-osum.sh [--uefi] [--script "..."] [--timeout N]` | boot the ISO in QEMU; exit code 0 when the kernel ends cleanly |
| `./test.sh` | acceptance suite |
| `./vendor/osum/hole-osum.sh` | build kernel and programs from `vendor/osum/COMMIT` |
| `./rename.sh <os-name> [slug]` | create a brand and make it the default ([RENAME.md](RENAME.md)) |

## Layout

| Path | Contents |
|---|---|
| `vendor/osum/` | the pinned Osum kernel (commit hash and fetch script; no binaries in the repo) |
| `vendor/firn/` | the pinned Firn compiler (same scheme) |
| `vendor/limine/` | the Limine bootloader, BSD-2-Clause |
| `brands/` | brand files |
| `userland/` | what goes into the image: `PROGRAMME`, files, themes, wallpapers |
| `pkg/` | package tool, recipes, file system builder |
| `tests/` | the acceptance steps |
| `docs/` | design and measurement documents |

## Limits

- The ISO's root file system is a RAM disk built into the image; changes
  survive only after the system is installed to a disk with `/bin/install`.
- The package tool `pkg/opk.py` runs on the host; the booted system reads the
  store and starts packages.
- Only x86-64 images are built; see [docs/ARCHITECTURES.md](docs/ARCHITECTURES.md).
- Testing happens in QEMU, not on real hardware.

## What is still to come

- An AArch64 kernel and device-tree based hardware support.
- A clean architecture boundary inside the kernel.
- Certus as an Android app.

The full list, sorted by dependency, is in [ROADMAP.md](ROADMAP.md).

## Documentation

| file | content |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | who does what, and where the boundaries lie |
| [ROADMAP.md](ROADMAP.md) | planned work |
| [BRANDING.md](BRANDING.md) | brands |
| [PACKAGING.md](PACKAGING.md), [PAKETE.md](PAKETE.md) | package design and the format as built |
| [FILESYSTEM.md](FILESYSTEM.md) | file system layout |
| [ASSISTENT.md](ASSISTENT.md) | interface for an assistant |
| [NETZWERK.md](NETZWERK.md) | network design |
| [userland/README.md](userland/README.md) | how the userland gets into the image |
| [tests/README.md](tests/README.md) | how the acceptance suite is built |

## Licence

**GPL-2.0-only** ([LICENSE](LICENSE)); version 2 only, never "or later".
Every source file carries an `SPDX-License-Identifier` line, which is
authoritative for that file. Programs written for the system are not
affected: the runtime they link comes from Firn (MPL-2.0) and from the Osum
ring-3 libraries (MIT). Reasoning: [LICENSING.md](LICENSING.md).
Third-party material, above all the Limine bootloader (BSD-2-Clause):
[THIRD_PARTY.md](THIRD_PARTY.md).
