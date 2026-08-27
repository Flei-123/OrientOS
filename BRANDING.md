# BRANDING.md — two brands, one source tree

There are **two** ways to give this system another name. They solve different
problems; whoever picks the wrong one makes unnecessary work or a fork.

| | `./build.sh --brand <name>` | `./rename.sh <kernel> <os>` |
|---|---|---|
| changes | only the **product** at build time | the **whole tree**: directories, docs, Cargo names |
| tree afterwards | **unchanged** | renamed, one commit |
| result | `build/<slug>.iso` next to the other one | a project with a new name |
| what for | second brand (XoffiOS), trial balloon, customer edition | final rename, once |
| instructions | this document | [RENAME.md](RENAME.md) |

The normal case is the first one. A fork is **never** the right way — two
trees drift apart, and from that day on you maintain everything twice.

---

## 1. Building a brand

```sh
./build.sh                    # default brand -> build/orientos.iso
./build.sh --brand xoffi      # second brand  -> build/xoffi.iso
```

Both images sit next to each other, from the same source text, without a
single file in the tree having been touched. To start it, name the same
brand:

```sh
BRAND=xoffi ./run-osum.sh --check
```

Counter-check that it really takes effect — the line comes from
`kcore::branding`:

```
[osum] boot       osum v0.1.0 — Kernel von XoffiOS
```

The kernel is called `osum` in both brands. That is deliberate: a brand
changes the product, not the kernel — the way NT is called NT under every
Windows edition, and XNU under macOS as under iOS.

---

## 2. Creating a brand

One file in `brands/`, done:

```toml
# brands/xoffi.toml
os-name   = "XoffiOS"
slug      = "xoffi"
publisher = "FleiTec"
web       = "https://xoffi.fleitec.com"
feed      = "https://xoffi.fleitec.com/pakete"
```

| field | meaning | required |
|---|---|---|
| `os-name` | name for humans: banner, user interface, docs | yes |
| `slug` | short name for machines: `<slug>.iso`, directories | yes |
| `publisher` | publisher | no |
| `web` | public address | no |
| `feed` | **package source of this brand** | no |
| `kernel-name` | override the kernel name | no, normally left out |

`feed` is separate per brand, and that is not a detail: an XoffiOS must never
"update" itself into an OrientOS. The same lesson is in FreeViewer
(`src/brand.rs`, `FV_BRAND_FEED`).

If a field is missing, the value from `[package.metadata.branding]` in
`kernel/Cargo.toml` applies.

---

## 3. Where the values come from

`kernel/build.rs` asks in this order, and the first answer wins:

1. **Individual environment variables** — `OS_NAME_OVERRIDE`,
   `OS_SLUG_OVERRIDE`, `OS_PUBLISHER_OVERRIDE`, `OS_WEB_OVERRIDE`,
   `OS_FEED_OVERRIDE`, `KERNEL_NAME_OVERRIDE`. For a quick attempt without a
   file:
   ```sh
   OS_NAME_OVERRIDE="Testsystem" ./build.sh
   ```
2. **`brands/$BRAND.toml`**, when `BRAND` is set (that is what `--brand`
   does).
3. **`[package.metadata.branding]`** in `kernel/Cargo.toml`.
4. **Derivation** from the Cargo package name.

An **unknown brand name aborts** and does not quietly fall back to the
default brand — otherwise you calmly build the wrong product.

The build scripts resolve the same order in `brand.sh` (`OS_NAME`, `SLUG`,
`KERNEL_PKG`), so that script and kernel can never drift apart.

---

## 4. The rule behind it

> **No product name appears in the source text.**

Everything comes from `kernel/src/kcore/branding.rs`:

```rust
KERNEL_NAME  OS_NAME  SLUG  PUBLISHER  WEB  FEED  VERSION  LOG_TAG  NATIVE_ABI
banner()
```

Instead of `"osum laeuft"` you write `"{} laeuft", branding::KERNEL_NAME`.
`./test.sh` fails the build when a product name turns up as a literal
anywhere else in `kernel/src` — the rule is checked, not just written down.

The same holds for the test scripts: `run-osum.sh` checks the boot banner
against `$OS_NAME` from `brand.sh`, not against a fixed name. Otherwise every
second brand would turn the test run red although everything is right.

---

## 5. What does NOT belong in a brand file

Differences between brands belong in **data**, never in code:

* which packages are in the image,
* appearance and defaults,
* package source.

The moment `if marke == "xoffi"` stands anywhere, the separation is broken and
you have built yourself a fork that only pretends not to be one.
