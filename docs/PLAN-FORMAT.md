# The PLAN, version 2 — the whole state of a machine, as one sorted text file

*Round PLAN2, 27.08.2026. Tool: `pkg/opk.py`. Test: `tests/step-90-plan.sh`,
33 assertions. Numbers: [ROUND-PLAN2.md](ROUND-PLAN2.md).*

This document is written in English, and so is everything this round
added to the code. The older half of `pkg/opk.py` is German and was left
alone on purpose — a rename is a round of its own, and mixing one into a
format change would make both unreviewable. How much is still German is
counted in the round log, not estimated.

---

## 1. What was wrong

A `PLAN` used to be this, and only this:

```
explorer	478632825585ffb1b8639f68da1bd3b0d132ea42e2cab51de820a320552d8f95
hallo	73d3f6f3c4e8256de0b444fd191c3cd6f14f5218a80052029b7bed70971fa6a0
widgets	e9cc64391932e1ed26f1696a2ed2f0789e3755170dd12647dc8eecee3578b28a
```

Three lines, about 200 octets, and [PAKETE.md](../PAKETE.md) called it
“the whole state of the system”. It was not. It described the installed
**applications**, which is roughly a quarter of what a machine is:

1. **No kernel.** The plan did not say which Osum the applications run
   on. The commit was nailed down in `vendor/osum/COMMIT` — outside the
   generation system, so a kernel update could not be rolled back the way
   an application update could.
2. **No sources.** Handed to another machine, the file names hashes and
   says nothing about where those hashes may be fetched from, or whose
   signature makes them trustworthy. A plan that is only a list of hashes
   is a shopping list without a shop.
3. **No system settings.** Time, network, screen, accounts — none of it
   was state anybody had written down. It lived in `/etc` files that some
   program had written at some point, which is precisely the “fifth
   place” PACKAGING.md § 2 says must not exist.

## 2. The shape

A line is TAB separated. The first field is a **type**.

```
app       <name>    <sha256>
kernel    <sha256>
source    <url>     <ed25519-public-key, 64 hex digits>
setting   <key>     <value>                 SYSTEM level  -> /etc/...
pref      <user>    <key>     <value>       USER level    -> /users/<who>/config/
account   <name> <uid> <gid> <home> <shell> <secret-sha256 | ->
```

`pref` came with the addendum of 27.08.2026 and is the answer to "does a
new device **look** the same". Why it is a separate type and not another
`setting` is argued in [CONFIG-LEVELS.md](CONFIG-LEVELS.md): the test is
not "is it about looks", it is *would two people on one machine want
different answers*. There are six types, so six reserved names.

A real one, from the run in the round log:

```
account	justin	1000	1000	/users/justin	/bin/sh	52d94b3107c08a8b…
app	hallo	73d3f6f3c4e8256de0b444fd191c3cd6f14f5218a80052029b7bed70971fa6a0
app	ls	a52a49084a96b9930a510d927f8e237a41d181dc7e88cec26414e198b38fc313
app	wc	d5f76847e9a83f889023529e840d12ea686dddb82e46c5a2b02a4e7d5896096b
kernel	32f1a6edc3f4e1fd6d8430a03b0b64f609d9d4c69b1dabf8fd24f4f17caa6a12
setting	hostname	orient1
setting	net.address	192.168.1.50
setting	net.gateway	192.168.1.1
setting	net.mode	static
setting	net.netmask	255.255.255.0
setting	time.offset	7200
setting	timezone	Europe/Vienna
source	file:///…/build/quelle	1c39291b4c4687c807c505c14545703e…
```

13 lines, 745 octets. That is a whole machine.

With a face on it — colour scheme, wallpaper, taskbar — it is **15
lines, 855 octets**:

```
pref	justin	taskbar.autohide	yes
pref	justin	taskbar.edge	left
pref	justin	taskbar.height	40
pref	justin	theme	380961b0606dcd6789f3071c4e10a0bfbf5d417a818733383f24cb7162ce2c28
pref	justin	wallpaper	cb90bea23844a4518c362c8ec1c50c0415b255cc1e0ce77ef0e0f4b6a3ced051
setting	display.mode	1024x768
```

### It is sorted by the octets of the whole line

Not by type and then by name — by the plain bytes of the line, which is
what `sort` does. That is deliberate and it buys one specific thing: the
canonical order can be checked **from outside this program**.

```
$ LC_ALL=C sort -c /system/generations/17/PLAN && echo canonical
```

The types happen to fall into `account, app, kernel, setting, source`,
which reads well enough. `opk.py verify` checks the same property, and
the test proves the check works by reversing the file and requiring
`sort -c` to fall over.

### Reading an old PLAN

The compatibility rule is one line long, needs no version header, no
migration and no rewrite of files that already exist:

```python
fields = line.split("\t")
if fields[0] in PLAN_TYPES:  →  typed line
elif len(fields) == 2:       →  the old `name<TAB>hash`, i.e. an app
else:                        →  error, naming the line
```

Every `PLAN` written before this round still parses. `opk.py plan` says
so out loud (“1 line(s) were read in the OLD two-field shape”) instead of
quietly pretending the file was always typed.

**The one hole, and how it is closed.** An application literally named
`app`, `kernel`, `source`, `setting`, `pref` or `account` would make an
old-shape line ambiguous. Six names. They are refused by `opk.py bauen` and by
`opk.py installieren`, which turns a silent misreading into a loud error
at the only two moments where it can still be fixed. `kernelchen` builds
fine — it is exactly those five words and not everything that sounds like
them.

## 3. Alternatives that were considered and dropped

**One file per aspect** (`PLAN`, `KERNEL`, `SOURCES`, `SETTINGS`). This
was the first idea and it is the one that looks tidiest. It was dropped
because a generation must be **one** object: with four files there are
four moments at which a generation is half written, and “roll back” stops
being a single operation. It also reintroduces exactly what PACKAGING.md
§ 1.4 forbids — several places where system state lives — only with a
nicer name.

**A version header line** (`#plan 2` as the first line). Dropped because
it buys nothing that typed lines do not already buy: a typed line says
what it is, so a reader needs no header to know. A header would also have
to be written into every existing file, i.e. a migration, i.e. a moment
where an old tool and a new tool disagree about a file both can read
today.

**JSON / TOML.** Dropped for the reason the whole design exists: `cat`,
`grep`, `sort`, `diff` and `wc -l` must work on the system state, on a
system whose userland is 69 small programs and has no JSON parser. A
tab-separated line is greppable from Osum's own `/bin/grep`; a nested
document is not.

**Key=value on every line** (`app name=explorer hash=…`). Dropped as
noise: the fields of a plan line are positional and few, and the type
already says which they are. It would also have made the sort order
depend on key spelling rather than on content.

**Renaming everything to typed lines at once, rewriting old files.**
Dropped because it makes the format change destructive for no gain. The
conversion happens when a plan is *written* anyway, so every tree
converts itself the first time anything changes. `opk.py export` performs
the conversion explicitly for the one case where it matters — handing the
plan to another machine.

**Keeping the Osum commit in the kernel line** (`kernel <hash> <commit>`).
Dropped: the commit is already in the kernel package's metadata
(`origin=osum:c5fe12ff…`), and putting it in the plan as well would be a
second place for the same fact. `opk.py verify` prints it by reading the
store, which is where it lives.

## 4. The kernel is a package

A kernel change is a **generation**, with the same mechanism as
everything else. `pkg/bauen.sh` builds `osum.opk` from
`vendor/osum/osum.mb`; it carries `kind=kernel` and `origin=osum:<commit>`
in its metadata. `opk.py kernel` writes a generation whose `kernel` line
is that hash; activation makes `system/kernel/image` a **hard link** to
`store/<hash>/image` — measured by comparing `st_ino`, not assumed. On a
1.6 MiB kernel and a 2 MiB image that is not a nicety.

Two guards, because “the kernel” must not become a place where anything
can be put:

* `opk.py installieren` **refuses** a package with `kind=kernel` — it
  belongs to the generation, not to `/apps`.
* `opk.py kernel` **refuses** a package without `kind=kernel` — otherwise
  any application could be made the kernel.

`zurueck` restores the kernel of the target generation with the same
single operation that restores its applications, and says so:

```
zurueck auf Generation 14 (war 15): 3 Paket(e) (hallo, ls, wc)
   KERNEL zurueckgestellt: 31c690c190ca -> 32f1a6edc3f4 (system/kernel/image)
```

**What that really does today, and what it does not.** It moves the
octets under `/system/kernel/image` and it keeps every kernel any
generation names in the store, so a roll-back has something to roll back
to. What it does **not** do is boot. The boot loader boots what
`build.sh` put on the ISO; nothing reads `/system/kernel` at boot time,
because there is no writable disk and no installer yet. Until the INSTALL
round lands, the kernel line is correct bookkeeping over real octets —
and calling it more than that would be a lie the next reboot exposes.

## 5. Settings

The key space is **closed**. Nine keys, listed in `SETTING_KEYS` in
`pkg/opk.py`, and `opk.py set` refuses anything else. An open key space is
the junk drawer that a system without a registry exists to prevent;
adding a key is a change to the program, which is the right amount of
friction.

| key | renders | read by |
|---|---|---|
| `time.offset` | `/etc/zeit.conf` (`offset=…`) | `kernel/user/einstellungen.fi` |
| `display.mode` | `/etc/schirm.conf` (`modus=…`) | `kernel/user/einstellungen.fi` |
| `net.mode` `net.address` `net.netmask` `net.gateway` | `/etc/netz.conf` | `kernel/user/dhcp.fi`, `einstellungen.fi` |
| `hostname` | `/etc/hostname` | **nothing yet** |
| `timezone` | `/etc/timezone` | **nothing yet** |
| `keymap` | `/etc/keymap` | **nothing yet** |

The last three are recorded and rendered, and that is all they do. Osum
has no consumer for a hostname or a keymap today. `opk.py set` prints
that fact when it writes one, rather than letting the user believe
otherwise.

The file contents are Osum's, not this repository's: `modus=fest`,
`maske=`, `gateway=`. The *keys* are English (`net.mode`, value `static`)
and the renderer translates. Adopting a foreign project's German words
into our own namespace would have been the easier and the wrong choice.

**`/etc` is derived, never edited.** Activation deletes every path in
`GENERATED_FILES` and then writes the ones the plan asks for. That is why
`unset screen.mode` makes `/etc/schirm.conf` disappear: the tree is a
**function of the plan**, not of its history. Files under `/etc` that are
not in that fixed list are never touched.

## 6. Accounts, and the password that is not in the file

A plan is meant to be handed to someone else — that is the entire point
of `rebuild`. A password hash in it would be a password hash in every
backup, every paste and every repository that ever saw the file.

So the account line carries the **SHA-256 of the credential**, never the
credential:

```
account	justin	1000	1000	/users/justin	/bin/sh	52d94b3107c08a8b…
```

The credential itself lives in `system/secrets/<name>`, mode `0600`,
written by `opk.py secret-set`. Two consequences, both wanted:

* **Changing a password is a generation.** The hash in the plan changes,
  so it can be rolled back like anything else.
* **A rebuild from a plan alone produces locked accounts.** `/etc/shadow`
  gets `justin:!:0:0:99999:7:::` — locked, not empty, and visible in the
  file rather than guessed at the next login.

`/etc/passwd` and `/etc/shadow` are written in the shape
`kernel/user/pw.fi` reads (`name:x:uid:gid:desc:home:shell` and
`$osum1$<rounds>$<salt>$<derived>`), and `/etc/shadow` is written `0600`
— the whole reason for splitting it off `passwd` in the first place.

## 6a. The user level, and content that is not text

The addendum of 27.08.2026 added a second level. Full argument in
[CONFIG-LEVELS.md](CONFIG-LEVELS.md); the format part is short.

```
pref	<user>	theme	<sha256>
pref	<user>	wallpaper	<sha256>
pref	<user>	taskbar.edge	bottom | top | left | right
pref	<user>	taskbar.height	<12..200>
pref	<user>	taskbar.autohide	yes | no
```

Five keys, closed set, rendered to
`/users/<who>/config/desktop/{theme,wallpaper,taskbar.conf}`.

**A plan line holds a decision; a file holds content.** `taskbar.edge` is
a decision — four values, nothing to store twice. A wallpaper is content,
and no image goes into a tab-separated line. So the two blob-valued keys
carry a **SHA-256** and the octets become an **asset package**: an
ordinary `.opk` with `kind=asset`, `class=theme|wallpaper`, one file
called `blob`. Same header, same store, same collector, same signed
`INDEX`, same `rebuild` — a wallpaper needs no mechanism a program does
not already have. A second blob store beside `store/` was considered and
dropped for exactly that reason.

`opk.py pref <user> <key> <value>` takes a hash, an `.opk`, **or any
ordinary file** — in the last case it packs it into an asset on the spot,
so "I picked this photo" is one command. The class is checked: a
wallpaper offered as a colour scheme is refused.

Measured: the wallpaper has **three names and one inode** (store, user
config, compatibility view), 172 812 octets lying there once.

## 7. Sources, and why the key travels with the plan

```
source	file:///…/build/quelle	1c39291b4c4687c807c505c14545703e…
```

A plan that says only **where** to fetch from is not self-bearing:
whoever controls the location controls the contents. A plan that carries
the **public key** is a statement about *whose* packages this machine
consists of. `opk.py rebuild` verifies every source index against the
keys **from the plan** and refuses everything else — not “warns about”.
The test builds a second source with the same packages and a different
key and requires the rebuild to refuse it, then re-signs the same
directory with the right key and requires it to be accepted.

This program can read `file://` sources and local paths. `https://` is
**not implemented**; it is written down here rather than claimed,
because nothing in this round could have measured it.

## 8. The commands round PLAN2 added

`--root` is the English spelling of `--wurzel`; both work everywhere.

```
opk.py plan           --root R        the plan of the active generation
opk.py export         --root R -o F   write it out as a standalone file
opk.py kernel         --root R <opk|name|none>
opk.py source-add     --root R <url> <pubkey-hex|file>
opk.py source-remove  --root R <url>
opk.py set / unset    --root R <key> [<value>]
opk.py settings       --root R        the closed key set and its consumers
opk.py account-add    --root R <name> [--uid --gid --home --shell]
opk.py asset-pack     <file> -o F --class theme|wallpaper [--name N]
opk.py pref           --root R <user> <key> <value>
opk.py pref-unset     --root R <user> <key>
opk.py prefs          --root R [<user>]
opk.py account-remove --root R <name>
opk.py secret-set     --root R <name> <file|->
opk.py snapshot       --root R        what the plan determines, canonically
opk.py verify         --root R        the plan against the tree
opk.py rebuild        --root R --plan F [--source D] [--secrets D]
```

## 9. `snapshot` — what is compared, and what is not

`rebuild` is only worth something if “identical” can be checked. `opk.py
snapshot` prints a canonical description of **everything the plan
determines**: the plan itself, every store entry the plan names, the
activated `/apps` view, `system/kernel`, the generated `/etc` files and
the three pots — kind, path, mode, size and the SHA-256 of every content.
The last line counts what it compared, because two empty trees always
match:

```
count entries=39 files=18 octets=3470309
```

Three things are deliberately **outside** it, and this is the honest half
of the design:

* **`system/generations/*` and `system/AKTUELL`.** How a machine got here
  is a *log*, not state. A rebuilt machine has one generation where the
  original had fifteen. Demanding they match would be demanding that the
  rebuild fake a past it does not have.
* **The contents of `users/<who>/{config,state,cache}/<app>`.** A plan
  describes a system, not the work done on it. The *directories* are
  compared, and so are the few files under `config/desktop/` that the
  plan generates — the user's own files are not. That is the same line
  [BACKUP.md](BACKUP.md) draws.

  **A correction, because it was wrong for a day.** The first version of
  `snapshot_lines` walked the three pots and therefore compared the
  user's documents, which no plan determines. It passed unnoticed
  because the pots were empty in the run that measured it. Fixed with
  the addendum; the pots are now walked for their directory structure
  only.
* **`system/secrets/`.** Credentials are not in a plan by construction.
