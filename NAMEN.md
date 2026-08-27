# NAMEN.md — finding the names: state and reasoning

State: 2026-08-25 (the OS name is set; the kernel part is unchanged from
2026-08-19)

| level | name | status |
|---|---|---|
| **Kernel** | **osum** | **set** (Cargo package name `osum`) |
| **Operating system** | **OrientOS** | **set** (2026-08-25, replaces the working title `Karstos`) |
| **Language** | **Firn** | set, its own project |

Renaming happens exclusively through `./rename.sh <kernel> <os>` — see
`RENAME.md`. No product name appears in the source text; everything comes from
`kernel/src/kcore/branding.rs`.

---

## Why OrientOS

Set on 2026-08-25. **OrientOS** was the name of an earlier project of our own
(~2024): an operating system from scratch with a built-in AI assistant. The
kernel of back then never flew, the idea did -- it now stands in
[ASSISTENT.md](ASSISTENT.md). The name closes that off.

### Checked on 2026-08-25

| registry | `orientos` |
|---|---|
| crates.io | free (irrelevant for the OS name anyway -- only the **kernel** is a Cargo package) |
| npm | free |
| PyPI | free |

| domain | state |
|---|---|
| `orientos.org`, `orientos.sh`, `orient-os.com` | **free** |
| `orientos.com` | taken |
| `orientos.dev` | no name servers set, to be checked with the registrar |

**Prior use:** `OrientDB`, a graph database, still exists. A different
category, no danger of confusion in the operating system domain -- unlike the
candidates rejected earlier, which all collided *in the same category*. On
GitHub there is no serious operating system of this name.

**Note:** "Orient" is a geographically loaded term. Deliberately accepted.

### Why not `OsumOS`

Free in the registries, but rejected: the kernel `osum` and the system
`OsumOS` differ by two letters. You separate the kernel name from the product
name -- NT/Windows, XNU/macOS, Linux/GNU all do it, and for good reason:
otherwise you cannot say in any sentence which level is meant.

### Second brand

A variant **XoffiOS** is **built and checked**: no branch of its own, no fork,
but one source tree and two builds.

```sh
./build.sh                    # -> build/orientos.iso
./build.sh --brand xoffi      # -> build/xoffi.iso, same source text
```

The brand files lie in `brands/`, the order of the sources and all fields are
in [BRANDING.md](BRANDING.md). The kernel is called `osum` in every brand.
Differences between brands belong in **data** (which packages are in the image,
appearance, package source), never in code.

---

## Why osum

- **All three relevant package registries free**: crates.io, npm, PyPI. No
  other checked candidate had that.
- **Short, typeable, pronounceable**, no umlauts, no special characters.
- A pun on **"awesome"** — deliberately chosen, not chance.

### Prior use (checked, deliberately accepted)

| finding | assessment |
|---|---|
| **OSUM** — a hobby OS blog project, 2008, "an 'Awesome' Operating System" | inactive since ~2008, no reach. Uncritical |
| **Ossum Inc.** — a software company, double s | different spelling, different market |

### Domains (whois, 2026-08-19)

| free | taken |
|---|---|
| `osum.cc`, `osum.sh`, `osumos.org` | `osum.com`, `.dev`, `.org`, `.io`, `.systems`, `osumos.com`, `osumos.dev` |

`osum.com` is occupied and active (HTTP 200). Irrelevant for an open source
project with no marketing ambition — `osum.sh` fits a systems project better
anyway.

---

## Checked and rejected candidates

### Because of existing software in the same domain

| name | conflict |
|---|---|
| **zircon / zirkon** | **the kernel of Google Fuchsia OS.** An identical category — the worst possible collision |
| **wolfram** | **`WolframKernel`** is the documented process name of the Mathematica computation engine. On top of that a trademark of Wolfram Research for software |
| **vanadium** | **the browser of GrapheneOS** (a hardened Chromium + WebView). A security OS with its own browser = exactly this project. Its fame is admittedly limited to a niche |
| **osmium** | **libosmium** (the OpenStreetMap C++ library) + `osmium-tool`, `pyosmium`, `node-osmium`. All three registries taken. In addition: "osmium" literally means *smell* |
| **titan** | a Google security chip, a moon of Saturn, dozens of products |
| **radix** | Radix DLT (crypto) + "radix sort" as a standard term. All registries taken |
| **solum** | OpenStack Solum, PyPI/npm taken |

### Because of occupied package registries

limen · cardo · silex · stratum · basalt · obsidian · serac · arx · vallum ·
nodus · urd · lithos · gaia · nadir · umbra · penumbra · syzygy · skarn ·
achat · rutil · kaldera · iridium · hafnium · palladium · rhodium · fortis ·
magnus · primus · imperium · robur · vis · hyperion · kronos · tartarus ·
aegis · bastion · ferrum

### Serious alternatives that stayed free

| name | meaning | remark |
|---|---|---|
| **nunatak** | a rocky peak sticking out of the glacier ice (Greenlandic) | the strongest image next to *Firn*, no conflict found. Runner-up |
| **tantal** | a metal that resists acids and absorbs nothing | fits the isolation theme. npm taken |
| **niob** | a metal, corrosion-resistant | free everywhere, but unknown |
| **chthon** | "earth, depth" (ancient Greek) | the hardest sound |
| **gneis** | rock under high pressure | `gneis.org` free |
| **eklogit** | rock from ~50 km depth | `eklogit.org` free |

---

## Lesson for later naming decisions (the OS name!)

**The periodic table is grazed bare as a source of names.** Security and OS
projects have been reaching for it for years: Zircon, Vanadium, Osmium,
Wolfram, Titan, Iridium — all taken, some of them in exactly this domain.

Checking order for the OS name:
1. **crates.io + npm + PyPI** (fast, hard facts)
2. **a web search** for existing software — do *not* guess from memory
3. **whois** for domains
4. trademark registries, should the project ever be marketed

The yardstick is not "does the word exist somewhere?", but
**"does it collide where this project lives?"**

---

## Check after the rename

```
./rename.sh osum OrientOS     # ausgeführt 19.08.2026
./build.sh && ./run-qemu.sh --check
./test.sh                    # ALLE TESTS BESTANDEN
```
