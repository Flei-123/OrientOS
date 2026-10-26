# BRANDING.md - one source tree, several products

The product name is hardcoded nowhere. It lives in exactly one place per
repository, and every other place reads it from there.

| repo | the one place | read by |
|---|---|---|
| OrientOS (this repo) | `brands/<name>.toml`, default named in `brands/STANDARD` | `brand.sh` -> `build.sh`, `run-osum.sh`, `test.sh` |
| Osum (kernel + userland) | `marke.conf` (values) + `OSUM_MARKE_*` environment (overrides) | `kernel/lib/brand.fi`, `tools/marke-einsetzen.py`, `tools/lib/marke.sh` |

There is **no second value store**. This repo does not copy `marke.conf`:
`brand.sh` exports the brand file as `OSUM_MARKE_*`, and the Osum build lets
the environment win over its own `marke.conf`. A brand therefore needs exactly
one file.

The default brand is **XoffiOS** (`brands/xoffi.toml`, slug `xoffi`).
`brands/orientos.toml` is the alternative brand **OrientOS**.

---

## 1. Building a brand

```sh
./build.sh                    # default brand (brands/STANDARD) -> build/xoffi.iso
./build.sh --brand orientos   # alternative brand              -> build/orientos.iso
BRAND=orientos ./run-osum.sh --check
```

An unknown brand name aborts; it never falls back silently to the default.
`./rename.sh <os-name> [slug]` creates a brand file and makes it the default.

The kernel is called `osum` in every brand (`osum.mb`, `/boot/osum`): a brand
changes the product, not the kernel, the way NT stays NT in every Windows
edition. A brand that really wants another kernel file name sets `kernel-name`.

---

## 2. The brand file

```toml
os-name   = "XoffiOS"
slug      = "xoffi"
publisher = "FleiTec"
web       = "https://xoffi.fleitec.com"
feed      = "https://xoffi.fleitec.com/pakete"
```

| field | meaning | required | `OSUM_MARKE_*` | placeholder |
|---|---|---|---|---|
| `os-name` | display name: boot menu, installer, UI, docs | yes | `OSUM_MARKE_PRODUKT` | `{{PRODUCT}}` |
| `slug` | machine name: `<slug>.iso`, directories, EFI path, version line | yes (`^[a-z][a-z0-9-]*$`) | `OSUM_MARKE_KURZ` | `{{SLUG}}` |
| `publisher` | company above the product | no | `OSUM_MARKE_HERSTELLER` | `{{VENDOR}}` |
| `web` | public address | no | `OSUM_MARKE_WEB` | `{{WEB}}` |
| `feed` | package source of this brand | no | `OSUM_MARKE_FEED` | - |
| `kernel-name` | override the kernel name | no, normally left out | `OSUM_MARKE_KERN` | - |

Limits enforced by the Osum build (it aborts instead of truncating): product,
kernel, publisher and slug at most 31 characters, web and feed at most 95.

`feed` is separate per brand on purpose: an XoffiOS must never update itself
into an OrientOS. Optional fields that are missing fall back to Osum's
`marke.conf`.

### Placeholders

Text that is not code (locale files, `INFO` files, shapes, docs inside the
image) uses `{{PRODUCT}}`, `{{SLUG}}`, `{{VENDOR}}` and `{{WEB}}`. The Osum
build replaces them from the values above; nothing else substitutes names.

### Order of precedence

1. `brands/$BRAND.toml` (what `--brand` / `brands/STANDARD` select) - exported
   by `brand.sh` as `OSUM_MARKE_*`, overwriting anything stale in the
   environment of the calling shell.
2. In the Osum build: `OSUM_MARKE_*` in the environment beats `marke.conf`.
3. `marke.conf` of the pinned Osum commit (only reached when a field is
   missing in the brand file).

`vendor/osum/hole-osum.sh` caches the built kernel per commit **and brand**
(`vendor/osum/.gebaut` holds `<commit> <brand hash>`), so switching brands
rebuilds the kernel and never reuses an image that carries another name.

---

## 3. The rule and how it is checked

> No product name appears in the source text.

`tests/step-10-marken.sh` checks it:

* every brand file is complete, slugs are unique, `brands/STANDARD` exists
  and names XoffiOS,
* `build.sh`, `run-osum.sh`, `brand.sh` and `hole-osum.sh` contain no brand name,
* `brand.sh` maps the brand file onto `OSUM_MARKE_*` and beats a stale
  environment,
* a second brand really builds (own ISO, own boot menu entry, same `osum`),
* an invented brand aborts,
* **counter-proof (TestMarke):** a throw-away brand `Probe-Distro` / `zzprobe`
  is built. The built output (ISO root: boot menu, kernel image, all files;
  the userland image) must contain the probe name, and must contain **neither
  "XoffiOS" nor "OrientOS"** (nor their slugs) in any case. A hardcoded name
  anywhere makes this step red.

---

## 4. What does NOT belong in a brand file

Differences between brands belong in data (which packages, appearance,
package source), never in code. The moment `if brand == "xoffi"` stands
anywhere, the separation is broken and you have built a fork that only
pretends not to be one.

---

## 5. Deliberately kept hits

Format ids, ABI and crypto labels are not product names; changing them
destroys existing disks, backups and signatures. Everything else that still
says `Osum`, `osum`, `OrientOS` or `orientos` is listed here with its reason.

| class | hit | reason (source) |
|---|---|---|
| format | `OSUM-OFS`, `OSUMMOD\n`, `.osp`, `osum.mb`, `/users/osum` | On-disk and module formats and fixed paths; programs and old disks read them (Osum 1, 2) |
| wire | `SSH-2.0-OsumSSH`, CPUID hypervisor id `0x6D75734F`, syscall numbers, kernel command-line words | Protocol/ABI identifiers; peers match on them (Osum 1) |
| symbol | `osum_panic`, `_F0.*` symbols | Symbol names called from assembly and the module table, not display text (Osum 1) |
| crypto | `orientos konto v1 siegel`, `orientos konto v1 marke`, `osum backup v1 ...` (key-derivation labels) | Changing a label makes every existing seal/backup unreadable = data loss (Osum 1, 2) |
| protocol | `orientos-device` / `orientos` (anb_fleitec OIDC client id, audience), melder backend id `orientos`, OIDC provider ids in `tools/anbfleitec` | Server-side protocol ids, must match the server (Osum 1, 3) |
| service | `anb_xoffi`, `kbund`, `sync`, `tools/account` Xoffi account provider, FreeViewer vector file `orientos-vectors.json` | Name of a separate service/contract file, not the OS product (Osum 3, Firn 4) |
| kernel name | `Osum` / `osum` as kernel name (`KERN`, boot line `osum <hash>`, `kernel-name` default) | The kernel keeps its name in every brand (this repo, Osum 1) |
| device | `HP-ORIENTOS` (device name), VM names in `tools/vbox/*`, OrientOS test VMs | Names of existing machines at the owner, not a product string (Osum 3) |
| history | `pruef/laeufe` old logs, `testlauf-r*.log`, `wasm-module/quelle` evidence, `mgr-orientos` in expected-red, repo path `/u_DiS4in7esMF1/orientos`, `ORIENTOS_REPO/REF` | Archived evidence and infrastructure names; rewriting them falsifies history (Osum 3) |
| test data | `FIRN_TEST_BRAND=OrientOS`, `tools/mini/fake_jarvis.py`, `webref/mockserver.py`, `tools/english/*` | Test fixtures and the English-rename tool, deliberately contain the old name (Firn 4, Osum 3) |
| asset | `tools/sounds/orientos.json` | Name of a sound family, an asset id (Firn 4) |
| format description | `tools/pack` text "OrientOS store package" | Describes the opk format of the earlier product (Firn 4) |
| vendor | `Fleitec` / `FleiTec` in notify/portald and `publisher` | The company above the product, not the product (Firn 4) |
| brand data | `marke.conf` in Osum, `brands/*.toml` here | The one place where names are allowed |
| comments/docs | Comments and design documents that describe the project history (`KERNELWECHSEL.md`, `NAMEN.md`, `PLAN.md`, ... in this repo; `platform_osum` header, `plat/*.fi`, `docs/*` in Firn) | Prose about the past; not read by any program. The brandcheck scanner skips comment lines (strict mode: `scan.py --comments`) (all workers) |
| pixel assets | `assets/marke/start-*.png` | Rendered logo pixels cannot be substituted as text; `tools/brand/startimage.py` would have to render the brand (Osum 2) |
| installed systems | existing installs keep `/EFI/orientos` | New installs use `/EFI/<slug>`; migration of old ESP directories is not handled (Osum 1) |
| pending | `explorer.fi` "OrientOS (/)", `trusttext.fi` / `wm.fi` trust dialog title, `sysinfo.fi`, `privacy.fi`, `hwcheck/check.fi`, `odbd.fi`, `wayd.fi`, `fvviewer.fi`, `jmlink.fi` User-Agent, `anb_fleitec` | NOT deliberate: still to be converted by the next Osum brand round (Osum 1 and 3 reports); listed so nothing is hidden |
