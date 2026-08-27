# Two levels of configuration — the machine's and the person's

*Round PLAN2, addendum of 27.08.2026. Format:
[PLAN-FORMAT.md](PLAN-FORMAT.md) · backup: [BACKUP.md](BACKUP.md) ·
numbers: [ROUND-PLAN2.md](ROUND-PLAN2.md) · test:
`tests/step-91-look.sh`*

---

## 1. The question that forced this

> *“If I set up a new device from the plan — does it **look** the same?”*

Until this addendum the answer was **no**, and for two separate reasons.
The first is obvious and the second is the one worth getting right.

## 2. Reason one: appearance was not in the system state at all

Measured on Osum's own tree, `kernel/user/einstellungen.fi`, 788 lines.
The settings application writes **only** into `/etc`, with German file
names, and none of it was in a `PLAN` or under `/users/<who>/config`:

| file | what it holds | written by | read by |
|---|---|---|---|
| `/etc/theme` | the active colour scheme | `einstellungen.fi` | `wlibc.theme_load`, `leiste.fi:579`, `schreibtisch.fi:204` |
| `/etc/schemas/` | the schemes to choose from | shipped | `einstellungen.fi:301` |
| `/etc/wallpaper` | the wallpaper, an OSYM image | `einstellungen.fi` | `schreibtisch.fi:203` |
| `/etc/display.conf` | resolution for the next start | `einstellungen.fi` | `einstellungen.fi` |
| `/etc/time.conf` | timezone offset | `einstellungen.fi` | `leiste.fi:228` |
| `/etc/network.conf` | address, mask, gateway | `einstellungen.fi`, `dhcp.fi` | both |
| `/etc/shadow` | password hashes | `passwd.fi`, `einstellungen.fi` | `pw.fi` |
| `/etc/taskbar.conf` | taskbar edge/height/autohide | round DESKTOP, **not there yet** | `leiste.fi` |

A machine rebuilt from its plan therefore came up with the applications
of the old one and the looks of a fresh install.

## 3. Reason two: half of those files are at the wrong level

The test is **not** “is it about looks”. It is:

> **Would two people on one machine want different answers?**

Two people share one screen resolution and one network — they *cannot*
each have their own. Two people do not share a wallpaper.

`/etc/theme` is therefore not a small blemish. It is **a place where a
second user cannot exist.** Whoever sets a colour scheme sets everyone's.

### The split, drawn

| level | who it belongs to | plan line | renders to |
|---|---|---|---|
| **system** | the machine — one answer, needs privilege | `setting <key> <value>` | `/etc/…` |
| **user** | the person — one answer *per person* | `pref <user> <key> <value>` | `/users/<who>/config/desktop/…` |

**System (`setting`), nine keys:**
`display.mode` · `net.mode` · `net.address` · `net.netmask` ·
`net.gateway` · `time.offset` · `timezone` · `hostname` · `keymap`

**User (`pref`), five keys:**
`theme` · `wallpaper` · `taskbar.edge` · `taskbar.height` ·
`taskbar.autohide`

Accounts stay at system level (`account` lines) — an account is a fact
about the machine, not a preference of the person who owns it.

Both key sets are **closed**. `opk.py set` and `opk.py pref` refuse
anything else. An open key space is the junk drawer a system without a
registry exists to prevent.

## 4. Plan line or file? Decision versus content

> **A PLAN line holds a DECISION. A file holds CONTENT.
> Content is named in the plan by its SHA-256.**

`taskbar.edge=left` is a decision: four possible values, nothing to store
twice, it belongs in the text. A wallpaper is content — an image, and no
image goes into a tab-separated line.

So the two blob-valued preferences, `theme` and `wallpaper`, carry a
**SHA-256**, and the octets are an **asset package** in the store:

```
pref	justin	theme	380961b0606dcd6789f3071c4e10a0bfbf5d417a818733383f24cb7162ce2c28
pref	justin	wallpaper	cb90bea23844a4518c362c8ec1c50c0415b255cc1e0ce77ef0e0f4b6a3ced051
pref	justin	taskbar.edge	left
pref	justin	taskbar.height	40
pref	justin	taskbar.autohide	yes
```

An asset package is an ordinary `.opk` with `kind=asset`, `class=theme`
or `class=wallpaper`, and one file called `blob`. It is not special in
any other way: same header, same hash, same store, same collector, same
signed `INDEX`, same `rebuild`.

**The alternative that was rejected: a second, non-package blob store**
(`blobs/<hash>/`). It would have meant a second collector, a second
verifier, a second fetch path and a second thing to sign — all for
something `store/` already does. Nothing about a wallpaper needs a
mechanism that a program does not.

**And the octets are still generated from text.** `userland/themes/*.theme`
are 18 `key=rrggbb` lines; `userland/wallpapers/*.wallpaper` are four
lines that `pkg/osym.py` turns into the OSYM image
`schreibtisch.fi` displays. A blob checked into a repository is
something nobody can read in a diff — the same argument Osum makes for
its own icons (`tools/k15/symbol.py`). Measured: the same four lines
produce the same 172 812 octets twice, so the hash in a plan is
reproducible from this repository and not only from a surviving copy of
the image.

`pkg/osym.py` also **refuses** anything larger than 240 × 180, which is
`BILD_MAX_W`/`BILD_MAX_H` in `schreibtisch.fi` — a process has 832 KiB of
heap here and the desktop stretches its source. Producing an image that
cannot be loaded would be worse than an error.

## 5. Where a user's appearance lives now

```
users/justin/config/desktop/theme          hard link -> store/<h>/blob
users/justin/config/desktop/wallpaper      hard link -> store/<h>/blob
users/justin/config/desktop/taskbar.conf   generated text
```

`taskbar.conf` is a **new** file, so its keys are English, and all three
are always written so the file alone determines the taskbar:

```
edge=left
height=40
autohide=yes
```

`edge` takes `bottom` / `top` / `left` / `right` — not invented, those
are the names in Osum's own `wlibc.fi` (`e_names`). The default height 28
is `const PH` from `leiste.fi`.

**These files are generated, and `config/` belongs to the human.** The
discipline is the same as for `/etc`: activation deletes exactly the
three named paths above and rewrites the ones the plan asks for. Nothing
else under `config/` is ever touched. Measured: a user's own file in
`config/eigenes/notiz` survives an activation.

## 6. What this means for the programs that read `/etc` today

Moving the truth to the user does **not** move Osum's programs. The
taskbar and the desktop open the absolute paths `/etc/theme` and
`/etc/wallpaper`, today, in the pinned commit.

So activation additionally writes a **compatibility view** under `/etc` —
**hard links to the very same store octets**, not copies:

```
etc/theme        -> the single account's theme
etc/wallpaper  -> the single account's wallpaper
etc/taskbar.conf -> the single account's taskbar
etc/schemas/<name> -> every scheme any user of this plan has chosen
```

Measured: the wallpaper has **three names and one inode** — in the store,
in the user's config, and in `/etc` — 172 812 octets lying there once.

**And it exists only while a machine has exactly one account.** With two
accounts there is no honest answer to *whose* theme `/etc/theme` would
be, so the view is **absent**, and `opk.py` says why:

```
NO compatibility view under etc/: this machine has 2 accounts, and there
is no honest answer to whose theme /etc/theme would be
```

That is not a limitation of this program. It is the bug of § 3 becoming
visible instead of being decided arbitrarily. `opk.py verify` asserts
both directions: one account → the view is exactly what the plan implies;
two accounts → the view is empty.

## 7. Hand-off to round DESKTOP — nothing here blocks it

Round DESKTOP is building the taskbar (`kernel/user/leiste.fi`) and the
desktop (`schreibtisch.fi`), and this round has **not touched the Osum
repository**. Three things are offered across the fence, all optional:

1. **`taskbar.conf` already exists and has a format.** `edge=`,
   `height=`, `autohide=`, all three always present, edge names taken
   from `wlibc.fi`. If DESKTOP picks a different shape, this side follows
   — one function, `render_taskbar` in `pkg/opk.py`.
2. **The path that should win in the long run is
   `/users/<who>/config/desktop/…`, not `/etc/…`.** The moment
   `leiste.fi` and `schreibtisch.fi` resolve the current user's
   configuration directory instead of an absolute `/etc` path, the tuple
   `COMPAT_FILES` in `pkg/opk.py` becomes empty and nothing else changes
   here. Until then the compatibility view keeps DESKTOP working
   unchanged.
3. **`/etc/schemas/` is generated from the plan** and lists the schemes
   the plan's users have chosen. A picker that wants to offer schemes
   that are *available but not chosen* is asking a catalogue question,
   not a system-state question — that answer is in the source's signed
   `INDEX`, and `opk.py` can list it. Putting every downloadable scheme
   into `/etc/schemas` would be putting a shop into the system state.

## 8. What was deliberately **not** renamed

English is the rule for everything new. Existing files belong to round
RENAME, and renaming another project's file from here would break it.

| stays as it is | why |
|---|---|
| `/etc/theme`, `/etc/wallpaper`, `/etc/schemas/` | Osum opens these paths by name today |
| `/etc/display.conf`, `/etc/time.conf`, `/etc/network.conf` | ditto; and their *contents* (`modus=fest`, `maske=`) are Osum's format |
| `/etc/passwd`, `/etc/shadow` | the shape `pw.fi` reads |
| `console` as a handle name | the existing handle namespace; two names for one object is worse than one German name |
| `fassung`, `braucht`, `handle`, `titel` in package metadata | changing them moves every package hash |

| renamed here, and why it was allowed | |
|---|---|
| `screen.mode` → **`display.mode`** | the key is *ours* and was one day old, with no consumer anywhere. The **file** it renders keeps Osum's name, which round `rename` made `display.conf`. |

New things introduced by this addendum, all English:
`pref` · `theme` · `wallpaper` · `taskbar.edge` · `taskbar.height` ·
`taskbar.autohide` · `config/desktop/` · `taskbar.conf` · `asset-pack` ·
`class=theme` / `class=wallpaper` · `userland/themes/` ·
`userland/wallpapers/` · `pkg/osym.py`.

## 9. What is still not solved

* **The product ISO carries no appearance.** `/etc` in the image is still
  assembled by `userland/PROGRAMME`, and the plan's generated `/etc`
  would collide with it in `mkfs`. Same gap as the rest of the settings;
  it is one merge, and it was left out because `build.sh` is on the
  critical path of a green suite while three rounds run in parallel.
* **Nothing switches the compatibility view when a user logs in.** With
  two accounts the view is simply absent — correct, but not yet useful.
  What is missing is the login path telling the desktop *whose*
  configuration to read, and that is DESKTOP's to build.
* **`einstellungen.fi` still writes `/etc` directly.** So a change made
  in the running system does **not** become a generation and is lost on
  the next activation. Closing that needs `opk` on Osum, which needs
  `/bin/opk` in Firn — the same missing round as before.
* **A wallpaper is capped at 240 × 180** and stretched. That is Osum's
  memory budget, stated, not worked around.
