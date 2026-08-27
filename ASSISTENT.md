# ASSISTENT.md — the built-in assistant and the perception layer

A design decision. Of the assistant itself **nothing** is built — the file
system, graphics and network for it are missing. What stands here is the
interface it will dock onto later, and the reason why it is being settled
already now: today it costs almost nothing and later it cannot be retrofitted.

The idea comes from an earlier project (Orient OS, ~2024): an operating system
in which an AI assistant is not a program *on* the system but part of it — it
sees what the human sees, it can search, it can operate things, it can extend
the system.

---

## 1. The one rule that determines everything else

> **The system stays complete without the assistant.**

The assistant is a **package**, not a component. At installation time the
system asks whether it should exist; in the settings it can be installed later
or removed again at any time. Afterwards nothing of it is left and nothing
depends on it.

That is not a politeness formula but a building rule with three consequences:

1. **No kernel code belongs to the assistant.** What it needs, others need
   too: accessibility tools, remote administration, test automation, scripts.
   There is no syscall, no service and no data structure that exists only
   because of it.
2. **No program calls the assistant.** The flow always goes the other way. An
   editor that does not start when the assistant is missing is a bug in the
   editor.
3. **Removing means removing.** `PACKAGING.md` already delivers that: an app
   is a directory in the store, uninstalling is two `rm`. No registry entry
   left behind, no service that keeps running.

---

## 2. Why no screenshots

The obvious way — an image of the screen every 500 ms to a model — is the
worst one. Not only because of the data volume:

| | screenshot | scene tree |
|---|---|---|
| data volume | 1920×1080 as a PNG: ~1–2 MB, as model input roughly 1,100–1,600 image tokens | one window as text: a few KB, a few hundred tokens |
| frequency | polled on a clock, mostly for nothing | only on a real change |
| latency | produce the image, encode, transmit, decode | one message over a channel |
| **expressiveness** | **the model has to guess what a button is** | **it says so: kind, name, state, place** |

The last point is the real one. A screenshot is a makeshift for the fact that
common operating systems do not know their own interface. Whoever sends pixels
throws away information that already exists in the system — and then has the
model guess it back.

**Whoever draws text knows it as text.** There is no reason in this system ever
to recover characters from pixels.

---

## 3. The perception layer

### 3.1 The scene tree is a by-product

The window server has to know anyway which element lies where, which text it
carries and whether it currently has the focus — otherwise it could not draw it
and could not deliver input. The perception layer is nothing more than **this
state, handed outwards** instead of only downwards into the frame buffer.

A node carries: kind (button, text field, list, label, image …), name, value,
state (focused, disabled, selected), rectangle, child nodes.

### 3.2 Mandatory instead of retrofitted — the real advantage

Windows (UIA), Linux (AT-SPI) and macOS (AX) have such a tree. It is full of
holes everywhere, because it came **afterwards**: programs have to join in,
many do not, and in interfaces that draw everything themselves it is empty.

Here the order is reversed. The window server is only just being built, so:

> **No window gets a canvas if it does not deliver its tree.**

No existing system could still enforce that without destroying half of its
programs. This one does not have programs yet — the cheapest moment there will
ever be.

The tree is thereby not "the interface for the AI" either, but the **user
interface in data form**: the same source serves the assistant, a screen
reader, remote administration, keyboard control and the testing of the
interface. Five consumers, one mechanism — that is the justification for its
sitting in the core.

### 3.3 Events, not polling

The observer does not ask "what does it look like", it subscribes to changes:

```
fenster 7 · feld "suche" · wert -> "firn"
fenster 7 · fokus -> knopf "öffnen"
fenster 3 · geschlossen
```

A complete tree is fetched exactly once, after that only differences. Nothing
happens as long as nothing happens.

### 3.4 Three levels, so that it stays small

| level | scope |
|---|---|
| `fokus` | only the window with the focus |
| `sichtbar` | everything the human is currently seeing |
| `alles` | covered windows as well |

The observer chooses. The default is `fokus`, because that is enough in almost
all cases and gives away the least.

### 3.5 A screenshot as the fallback

For real pixels — video, a game, a photo, a free canvas — a single image stays
possible, but:

* only on an explicit request, never on a clock,
* only the requested section, not the whole screen,
* as a right of its own, granted separately.

A node whose content cannot be described as a structure marks itself as "only
an image helps here". Then the observer knows when the expensive way is worth
it — and only then.

---

## 4. How this fits into the existing ABI

**It needs not a single new syscall.**

`libs/osum-abi-native/src/syscall.rs` has 23 numbers (0–22), without gaps,
stable, "only ever appended". The window server is an ordinary process, and the
existing building blocks are entirely sufficient:

| existing | task here |
|---|---|
| `NamespaceOpen` (6) | find the window server — no global path, no process-global root |
| `ChannelCreate` / `Send` / `Recv` (11–13) | request the tree, send operating commands |
| `PortCreate` / `PortWait` / `PortBind` (14–16) | wait for the change stream without polling |
| `HandleDuplicate` with a rights mask (4) | pass fewer rights on to a helper, never more |
| `MemoryCreate` / `MemoryMap` (8–9) | hand over a screenshot without copying it |

That a capability of this magnitude fits in without extending the call list is
the best available evidence that the ABI is cut correctly. Should that change,
the rule still holds: only append, never reassign a number.

---

## 5. Rights — this is where everything is decided

An assistant that sees the screen and produces input is a security nightmare
elsewhere, because it has the same power as the logged-in human. Not here: it
gets **handles with `Rights`**, and rights can only be reduced when passing
them on (`Rights::restrict`).

Planned, in the language of the existing rights:

| right | permits | without this right |
|---|---|---|
| `OBSERVE` | reading the scene tree | the observer sees nothing |
| `CAPTURE` | a single image of an area | no pixels, not by detours either |
| `INJECT` | producing input | it can report but not act |

Three properties that follow from it and that no retrofitted system can offer:

* **Granted separately.** Seeing without operating is the normal case.
  `INJECT` is granted individually and visibly.
* **A window may withdraw itself.** A password store that refuses `OBSERVE` is
  not merely blocked for the assistant — it **does not exist for it**. That is
  not a policy you could get around; the handle is simply missing.
* **No inheriting.** A fresh handle table is empty. A helper started by the
  assistant has **no** access to anything of its own accord.

Whether the assistant is currently watching belongs visibly in the interface —
enforced by the window server, not by the assistant. Whoever observes must not
be able to keep that quiet himself.

---

## 6. Where the model runs

Two operating modes, the same interface:

**Remote.** The assistant talks over the network to a service. Doable as soon
as the network stack stands. It needs the right `NET`, and the combination
`OBSERVE` + `NET` has to be confirmed explicitly — that is the point at which
screen content leaves the machine.

**Local.** The model runs on the machine. Honestly: that is far away. It needs
graphics drivers, usable throughput for matrix computation, management of very
large memory regions — and there is no file system yet. No reason to build it
now.

The reason to settle the interface today anyway: it is the same in both cases.
Whoever cuts the perception layer cleanly can swap the model out later without
touching the interface.

---

## 7. Order

None of this is the next step. The dependencies are hard:

1. **File system** — without it no store, no package, no installing later.
2. **Graphics and the window server** — this is where the scene tree comes into
   being. **The moment at which § 3.2 is decided.**
3. **Input** — keyboard and pointer, and with them `INJECT`.
4. **Network** — remote operation.
5. **A file manager and the rest of the interface** — the first serious
   consumers of the tree.
6. **The assistant as a package.**

What is to be done **now** is exactly one thing and it costs no
implementation: when the window server is built, the scene tree is **built
along with it and made mandatory**. Retrofitted, it turns into the same
landscape of holes as everywhere else.

---

## 8. What is deliberately left open here

* The exact format of the tree — it comes into being with the window server,
  not before.
* Whether `INJECT` presents itself as a synthetic input event or as a call on
  the tree node. The latter is more reliable, but the programs have to play
  along.
* What "extend the system" looks like. An assistant that installs packages
  needs rights in the store — and that is a decision of its own that can only
  be taken once `PACKAGING.md` exists in built form.
