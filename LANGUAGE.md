# Logbook: the road from Rust to Firn

> **SUPERSEDED ON 2026-08-25, SETTLED ON 2026-08-26 — and in the good
> way.** The road this document describes did not fail; it was **cut
> short**. Instead of translating 18,000 lines of Rust to Firn module by
> module, OrientOS uses the kernel that already stands finished in Firn:
> **Osum**, its own repository, embedded through `vendor/osum/COMMIT`.
>
> **The migration state in M-00 below is history, not a statement of
> fact.** On 2026-08-25 it stood at 5 %. On 2026-08-26 the Rust kernel
> was deleted — 18,255 lines of Rust, 1,206 lines of C and 965 lines of
> Firn, after every open item had been worked off. **Today's state is: 0
> lines of Rust in this repository** (plus 378 lines of uncompiled
> template), 0 lines of Firn in this repository — Firn stands completely
> in Osum, 32,800 lines.
>
> **What holds now is in [KERNELWECHSEL.md](KERNELWECHSEL.md):** the
> comparison module by module, what was ported (UEFI boot, capability
> handles, SMEP/SMAP, boot modules), what was deleted and what stays open
> (the arch boundary; channels/ports/namespaces).
>
> **Why this document stays anyway:** it is the logbook of the friction
> points between Rust and a kernel, and every entry in it is a
> requirement for Firn. Those entries (M-02 downwards, L-*) hold
> unchanged — they are the reason Firn exists, and they do not become
> wrong just because the Rust code is gone. Only the **figures** in M-00
> are history, and they stand there explicitly as such.

**Decided (2026-08-21): osum becomes Firn-only.** No Rust, no adopted foreign
code. Until then this document was called "where Rust gets in the way in the
kernel" and collected evidence that a language of our own would be
**justified**. That question is answered — the language is called **Firn**, it
exists, and the rebuild has begun. From here on the document is two things: the
**migration state**, and still the logbook of the friction points, because
every one of them is a requirement for Firn.

> **The old purpose, for context:** "The language is not being built now —
> writing a language together with a backend, tooling and a library while the
> kernel does not even have a userspace is the classic yak shave that projects
> die of." That was right at the time of round 3. It is superseded, because
> Firn has since compiled itself and carried a kernel of its own with address
> spaces, a scheduler and a file system in round 62.

---

## M-00 · Migration state — **settled by the kernel switch**

The state that stood here until 2026-08-25 was:

| | |
|---|---|
| **In Firn** | `serial.fi` (117) · `bitmap.fi` (344) · `elf.fi` (461) — 922 lines |
| **Still in Rust** | 17,993 lines in `kernel/`, `libs/`, `userland/` |

So about **5 %** after three rounds. The rest would have been some 18,000
lines of translation work — with the result of a kernel that can do **less**
than the one that already stands finished in Firn next to it.

These two figures are **history**. They describe 2026-08-25 and are not kept
up to date; whoever is looking for today's state finds it in the table below.

**The state since 2026-08-26:**

| | |
|---|---|
| **Kernel** | comes from the **Osum repository**, pinned through `vendor/osum/COMMIT`. 15,495 lines of Firn in the kernel, 3,592 in the userland, 1,234 libc, 1,204 assembly. |
| **Compiler of the kernel** | Osum's own pin (there `vendor/firn/COMMIT`) — a **different** commit from this repository's. Two projects, two pins. |
| **Firn in this repository** | **none any more.** `kernel/firn/serial.fi`, `bitmap.fi`, `elf.fi` (922 lines) belonged to the Rust kernel and were deleted with it; their counterparts are in Osum (`serial.fi`, `mem.fi`, `elf.fi`). |
| **Still in Rust** | **0 lines.** Deleted on 2026-08-26 (18,255 lines, counted as checked in). One file stays as an **uncompiled template**: `vorlage/arch_iface.rs`, 378 lines, with the reasoning in its header — there is no `cargo` and no `Cargo.toml` in this repository any more, so it is text and not code. Why: [KERNELWECHSEL.md](KERNELWECHSEL.md) § 4.1. |
| **Compiler of this repository** | `vendor/firn/COMMIT` — no longer needed here for compiling, since no Firn module lies in this tree any more. Osum pins its own. |
| **Call direction** | moot: there is no language boundary in this repository any more. The entry M-02 stays valid nevertheless — it describes what Firn lacked, not what OrientOS does. |

**What the switch does NOT change about this document.** The friction points
M-02 and L-* are experiences with Rust in a kernel; they stay valid and are
still requirements for Firn. What changed is the answer to the question *"who
writes the kernel"* — not the one to *"which language".*

### Why the compiler is pinned

Firn is under active development. If osum always built against the newest
state, then with every bug it would be unclear whether it comes from the
kernel or from the compiler. Hence: **one commit**, entered in
`vendor/firn/COMMIT`, and it is moved forward **only when `./test.sh` is
green**. The binary is not checked in — only the hash and the build script.

## M-01 · Bridgehead: the serial console

**Done and measured.** `kernel/src/arch/x86_64/serial.rs` contains no logic any
more; the 16550 UART is driven by `kernel/firn/serial.fi`.

* **Why this module of all things:** it is a **leaf**. It calls nothing outside
  itself and holds no state — as the only module it therefore meets neither of
  the two language boundaries from M-02.
* **Calling convention:** Firn produces ordinary SysV AMD64 (arguments in
  `rdi`/`rsi`, return value in `rax`, `rbp` and `r12`–`r15` are saved).
  `extern "C"` is enough, no special treatment is needed.
* **Symbol scheme:** `firnc` assigns `_F0.<name>`. The dot is not a valid
  identifier in Rust, hence `#[link_name = "_F0.serial_init"]`.
* **Build path:** `build.sh` compiles the Firn modules **before** cargo and
  checks that the object has **zero undefined symbols**; `kernel/build.rs`
  passes it on to the linker and aborts with a clear message when it is
  missing.
* **Proof:** `./test.sh` — **all 21 sections passed**, 14 QEMU boots. Every
  line of boot output in that run went through the Firn code; a bug there would
  have blinded the entire test run.
* **First compiler change survived (2026-08-21):** from `1bd95ceb` to
  `c889170a` — 13 Firn rounds apart, among them type aliases, a default type
  for number literals, `+=`, `str` and the splitting of `std`. The produced
  object is **bit-identical** to the previous one (same SHA-256), and
  `./test.sh` runs through 21/21 unchanged. With that, the promise from the
  Firn project — additive rounds do not break existing code — has been
  **measured** once for osum instead of believed. Log:
  `build/testlauf-firn-c889170a.log`.
* **Reproducible:** the same compiler produces byte-wise the same object from
  the same source (checked with `cmp`). A difference in the object is therefore
  always a difference in the source or the compiler, never noise.

## M-02 · The two boundaries that block the next step

Stage 0 of Firn knows **no `extern fn`** and **no `static`**. Both are refused
by the compiler. From that follows, hard:

1. **Firn cannot call back into Rust.** A Firn module can be called but cannot
   call anything outside its compilation unit.
2. **Firn cannot hold global mutable state.** Firn's own demo kernel gets
   around this with a state block whose address the startup code passes in — a
   working crutch, not a solution.

**As long as that is so, it stays with leaf modules.** Every driver wants both
sooner or later. The language round for it is Firn's and is planned; until then
nothing is forced here and nothing is faked.

That is, by the way, exactly **L-04** from the list below, only from the other
side: Rust has no category for "belongs to the CPU" and pushes towards
`static mut` (16 occurrences). Firn does not have the category yet either — but
it is the language in which it can still come into being.

## M-03 · What Firn round 73 releases — and why it changes the plan

Until round 73 `profile kernel` forbade **every** `import std.*`. A Firn module
in the kernel therefore had the language but no library. That is lifted:

* **`std.core` is allowed in the kernel profile.** In it lies everything that
  neither requests memory nor makes a system call: the span layer (`find`,
  `trim`, `starts_with`, `compare`, splitting), UTF-8 reading, number
  conversion (`text_to_i64`/`u64`/`f64`), maths (`isqrt`, `gcd`, `ilog2`). The
  compiler **verifies the claim** — a module that does allocate after all stays
  forbidden, with a counter-check in the Firn test run.
* **The allocator is a visible parameter** (the way Zig goes): no global
  allocator, no hidden request. At the call site you can read off whether a
  function costs memory. There is an arena implementation **without** a system
  call that a kernel can use.

**Why that is more than comfort:** it is the answer to **L-06**, and precisely
the one that already stands there as a wish — "the allocator as an explicit
argument, without a special path for the standard data types". Firn built it
before osum needed it.

**And it turns M-02 from a blocker into a design principle.** "No global
mutable state" was noted as a shortcoming. If state is consistently passed
through as a parameter — allocator as well as device state — that is not a way
around the missing `static` but the better design. osum's own `frame_alloc` can
satisfy the interface; with that the memory management can move to Firn
**without** waiting for a language round.

What is **not** solved by it: `extern fn`. As long as Rust code exists that a
Firn module has to call, the direction stays one-way. That settles itself with
the last Rust module — or with the planned language round, whichever comes
first.

*(Addendum 2026-08-23: round 75 brought `extern fn`, in both directions. The
boundary from M-02 has thereby half fallen. `static` is still missing.)*

## M-04 · Second building block: the bitmap frame allocator

**`libs/osum-mem/src/bitmap.rs` (593 lines of Rust) has been removed**, and in
its place stands `kernel/firn/bitmap.fi` (335 lines of Firn), attached through
`kernel/src/mm/firn_bitmap.rs`.

### Why this module of all things

* It holds **no global state**. The bit storage and the control block belong to
  the caller and are handed in as pointers — the Rust version did it exactly
  the same way with `&'a mut [u64]`. It therefore does **not** meet the
  remaining boundary (`static`).
* It is **pure logic**: bit arithmetic, next-fit with wraparound, range
  operations. No port fiddling, no hardware.
* It had **23 test cases**, among them a property test against a reference
  model. That made it objectively checkable whether the new version does the
  same thing.
* It runs in **every boot**. A bug shows up immediately, not at some point.

### What Firn contributes here that Rust did not

The arithmetic is **checked**. In Rust's release build `used -= 1` silently
wraps; from then on the allocator reports billions of free frames and hands out
memory that does not exist. The same line aborts in Firn with the file, the
line, the column and **both numbers**.

Where an overflow is **not** a bug — range boundaries from the memory map are
allowed to reach past the end of the bitmap — that is stated explicitly in the
source text: the boundary is clamped beforehand (`kappen`), and `bm_free`
checks `anzahl > U64MAX - index` instead of forming the sum. The difference
between "may overflow" and "may not" is thereby **readable** instead of being a
convention.

### Proof

* **23 of 23** cases passed — `tests/firn-bitmap/`, a C test harness against
  **the same** object that ends up in the kernel image (kernel profile,
  freestanding). The same random generator as the Rust version, so the same
  sequence of requests.
* `test.sh` section 22 additionally checks that the control block has the same
  layout on both sides (with a counter-check) and that the Rust version really
  is gone.
* In the kernel: **8/8** memory map checks, **28/28** frame checks, in every
  boot.

### What was lost in the process — named honestly

Four host tests in `libs/osum-mem/src/lib.rs` checked the transition memory map
→ bitmap. Two of them (an unsorted overlapping map, unaligned edges) have moved
into `mm::frame::map_selftest()` and now run in **every boot** against the real
object — that is an improvement.

**Gone without replacement** is a property test over **200 randomly shredded
memory maps**. It needed `Vec` and a host. The randomness coverage has been
missing since.

### The price: speed

Measured under identical load (40,000 requests/returns, 65,536 frames, the same
sequence, best of 7 runs):

| version | time | ratio |
|---|---|---|
| Rust method, unchecked | 0.081 s | 1.00× |
| **Firn `dev-fast`** (what osum builds) | **0.415 s** | **5.11×** |
| Firn `release-fast` (checks off) | 0.098 s | 1.22× |

**That is not a rounding error and it is not being talked up here.** The
breakdown does say where it comes from, though: with the checks switched off
Firn is at 1.22× — the rest is the checks themselves plus a code generator that
still routes every local variable through the stack instead of through
registers.

**Why it is defensible anyway:** the frame allocator is called a few thousand
times during the boot, not a few million. 5× on an operation in the microsecond
range is not measurable in the boot log. Should that change — for instance if
the allocator ends up in a hot path — the figure is recorded here and will have
to be re-evaluated then.

### A compiler bug found along the way

`--opt-level=release-safe` produces wrong code: a checked multiplication uses
`mul`, which implicitly overwrites `rdx`, and the register allocator does not
account for it. A value that lies in `rdx` and is supposed to survive the
computation is destroyed afterwards. The bitmap falls over at this level in
**19 of 23** cases and is faultless at all the others.

Minimal case (20 lines), disassembly and analysis: `tests/firn-fehler/`. osum
builds with `dev-fast` and is not affected; `release-safe` stays blocked.

## M-05 · Third building block: the ELF64 checking part

**The checking part of `kernel/src/kcore/elf.rs` (268 lines of Rust) has been
removed**, and in its place stands `kernel/firn/elf.fi` (415 lines of Firn),
attached through `kernel/src/kcore/firn_elf.rs`.

### Why this part of all things

An ELF comes **from outside**. Every number in it is a stranger's claim: "my
table begins at offset X", "my segment is Y long". Whoever computes with that
unchecked builds himself exactly the hole attackers look for — an
`off + filesz` that wraps around yields a small number, the range check passes,
and afterwards something is read at an address that was never checked.

The **loading part** stays in Rust. It hangs on page tables, the frame
allocator and the direct map window — a dozen callbacks. But it also does not
touch any foreign bytes: it works with the already checked values. That is not
convenience, it is the right cut.

### First the yardstick, then the port

`elf.rs` had **zero `#[test]`**. There was a self-test in the kernel with 16
cases — better than nothing, but not a yardstick against which a new version
can be measured: the cases stood as Rust code **inside** the file that was to
be replaced.

That is why the yardstick was built **first** and evidenced against the **old**
version before a single line of Firn came into being:

* **53 case files**, produced by `tests/firn-elf/faelle.py` — 8 valid, 45 to be
  refused
* Covered: files truncated at every interesting place, every header field
  corrupted individually, lying offsets and sizes, **values close to
  `u64::MAX` that only overflow on addition**, overlapping segments, segments
  in the same page, edge cases of alignment, exactly 16 against 17 segments
* **The old Rust version passes 53 of 53** — evidenced, not claimed

The old version lies as a **reference yardstick** in
`tests/firn-elf/alter-pruefteil.rs.txt`. It is no longer built, but it is
**driven** in every test run: as long as both versions say the same thing about
every case, the port is demonstrably **behaviourally identical** — not merely
"passes tests too".

### Proof

* **53 of 53** against the Firn version (`tests/firn-elf/lauf.sh`)
* **53 of 53** against the Rust version (`tests/firn-elf/lauf-rust.sh`)
* `test.sh` section 23: both runs, the `MAX_SEGMENTS` comparison, the struct
  layout with a counter-check, the symbols in the image, "the Rust checking
  part is gone"
* In the kernel: **16/16** ELF negative cases, **5/5** loading cases, every
  boot — the real `hello` from the boot file system is loaded through the Firn
  parser

### What Firn contributes here

The Rust version caught the overflows by hand with `checked_add`, at every
single place, and the module header said so explicitly: *"all additions are
overflow-safe"*. That was **discipline**, and it held.

In Firn it is the **language**: every computation that overflows aborts — even
the one nobody thought of. Where an overflow is an *expected error case* and
not a programming mistake, that is stated explicitly: `passt_dazu(a, b)` does
not form the sum at all but checks `b <= U64MAX - a`. The difference between
"must not overflow" and "has to deliver an error value" is thereby
**readable**.

### The price: speed

1,060,000 calls across all 53 cases, best of 5 runs:

| version | per call | ratio |
|---|---|---|
| Rust method (`checked_*`, `cc -O2`) | 79 ns | 1.00× |
| **Firn `dev-fast`** (what osum builds) | **651 ns** | **8.25×** |
| Firn `release-fast` (checks off) | 136 ns | 1.97× |

**That is worse than with the bitmap** (5.11× / 1.22× there), and the reason is
clear: a parser computes in every line.

**Why it is bearable anyway:** the checking part runs **five times** during the
boot — once per program plus the self-tests. 651 instead of 79 nanoseconds add
up to under three microseconds. Should osum ever start programs by the second,
the figure is recorded here and will have to be re-evaluated.

**The cause is found and written down**, not accepted: `tests/firn-fehler/TEMPO.md`
names three codegen findings with a minimal example and a disassembly against
`cc -O2`. The largest one — **the checked addition puts both operands on the
stack, on the success path** — explains about three quarters of the overhead
and can be fixed without touching the semantics.

### What was lost — named honestly

**Nothing in test coverage.** For once that is the whole truth here: the kernel
self-test with its 16 cases runs on unchanged (now against the Firn version),
and 53 case files that did not exist before were added.

What is **not** ported and stays open: the loading part (create pages, copy
contents, roll back), about 440 lines. It needs callbacks into `mm` and `arch`
— doable since round 75, but a chunk of its own.

---

## The friction points with Rust

Still valid as a **list of requirements for Firn**: every entry describes a
place that has to be solved better in the new language than in the old one.
State of the measurements (2026-08-21): **438** `unsafe` occurrences in
`kernel/src`, **16** `static mut`.

**Rule:** whoever writes a workaround enters it here. No entry without a file
and a line. Every reference has the form `` `datei.rs:Zeile` (`Anker`) `` and is
checked mechanically by `test.sh` (the step "Doku-Verweise").

## L-01 · `abi_x86_interrupt` — nightly for something fundamental

* **File:** `kernel/src/main.rs:26` (`abi_x86_interrupt`), used in `kernel/src/arch/x86_64/interrupts.rs`
* **Problem:** exception handlers need a calling convention of their own (the
  CPU lays down a different frame, it ends with `iretq`, on some vectors an
  error code sits on the stack). Rust can do that only through an **unstable**
  feature. A kernel without interrupt handlers is not a kernel — which means:
  for a core task the stable language is not sufficient.
* **Workaround:** `#![feature(abi_x86_interrupt)]`, documented in
  ARCHITECTURE.md § 9. The alternative would be hand-written assembly stubs for
  256 vectors.
* **A language of our own:** calling conventions have to be **declarable**, not
  built in. Something like "this function receives its frame from the hardware,
  returns with instruction X, saves registers according to list Y" — as a
  language construct, not as a compiler special case per architecture.

## L-02 · `alloc_error_handler` — nightly for an error message

* **File:** `kernel/src/main.rs:30` (`alloc_error_handler`)
* **Problem:** on an exhausted heap we want a **readable** message with heap
  statistics instead of a bare panic. That requires another unstable feature.
* **Workaround:** `#![feature(alloc_error_handler)]`.
* **A language of our own:** getting memory should not need "language hooks" at
  all. Allocation is an ordinary operation with an ordinary result (`Result`),
  not global state with a global error hook. See also L-06.

## L-03 · `naked` + `naked_asm` for the context switch

* **File:** `kernel/src/arch/x86_64/context.rs:100` (`fn switch`)
* **Problem:** a context switch must have **no** compiler-generated prologue and
  epilogue — it swaps the stack out from under itself. Rust needs
  `#[unsafe(naked)]` plus `naked_asm!` for that and forbids practically
  everything else in such functions.
* **Workaround:** it works, but it is an island: the compiler knows nothing
  about what happens in there and can check nothing.
* **A language of our own:** the context switch is one of the ten most
  important operations of a kernel. It should be an **understood language
  construct** ("this function switches the execution context from A to B"), so
  that the compiler generates the register saving correctly itself and can
  check it, instead of a human writing it down in assembly and hoping.

## L-04 · `static mut` everywhere the hardware dictates the address

* **Files:** 13 places, among them
  `kernel/src/arch/x86_64/idt.rs:50` (`static mut IDT`),
  `kernel/src/arch/x86_64/gdt.rs:36` (`static mut DF_STACK`),
  `kernel/src/boot/limine.rs:68` (`static mut REGIONS`),
  `kernel/src/kcore/sched.rs:734` (`static mut ACTIVE`),
  `kernel/src/kcore/print.rs:31` (`static mut SINK_OBJ`)
* **Problem:** these objects have **exactly one** instance, their address is
  handed to the hardware (`lidt`, `lgdt`, `ltr`), and they are read from
  interrupt context. Rust's ownership model has no category for that: there is
  no "belongs to the CPU". You end up with `static mut` + `addr_of_mut!` (25
  places) or you build pointer gymnastics.
* **Workaround:** `static mut` with `addr_of_mut!` so as not to create
  references; every access is `unsafe`.
* **A language of our own:** a notion of ownership for **hardware ownership**.
  Something like `hardware static IDT: [Gate; 256] @ align(16)` — the compiler
  then knows: one instance, a stable address, access only through a defined
  interface, no references leaving. That is not a security hole, it is a
  missing category.

## L-05 · Page tables: 34 `unsafe` occurrences for a tree structure

* **File:** `kernel/src/arch/x86_64/paging.rs` (34), `mm/mod.rs` (9)
* **Problem:** a page table is a tree of 512-entry arrays chained through
  **physical** addresses and reachable only through a direct map window
  (`phys + HHDM`). Rust's references cannot express physical addresses; every
  descent in the tree is raw pointer arithmetic. The borrow checker helps
  nowhere here.
* **Workaround:** `PhysAddr`/`VirtAddr` as types of their own,
  `phys_to_virt()`, raw pointers, everything `unsafe`.
* **A language of our own:** **address spaces as first-class types.** A pointer
  should know in which address space it is valid (`phys`, `virt`,
  `virt@prozess-7`), and the compiler should enforce the conversion instead of
  permitting it. Then "a physical address dereferenced without HHDM" would be a
  type error instead of a crash.

## L-06 · The global allocator is global state

* **File:** `kernel/src/mm/heap.rs` (37 `unsafe` occurrences)
* **Problem:** `#[global_allocator]` is exactly one allocator for the whole
  process. A kernel wants several: an early boot allocator, the kernel heap,
  DMA-capable memory (physically contiguous, below a boundary), per-CPU areas,
  later per-process. `Vec::new_in(alloc)` exists only behind `allocator_api`
  (nightly again), and `Box`/`String`/`format!` keep using the global one.
* **Workaround:** a single global heap; everything else goes past the `alloc`
  crate and works with frames directly.
* **A language of our own:** the allocator as an **explicit argument** or as
  context in the type, without a special path for the standard data types.
  "Where does this memory come from" is a core question in a kernel, not a
  footnote.

## L-07 · Interrupt context is invisible in the type system

* **Files:** `arch/x86_64/interrupts.rs`, `arch/x86_64/serial.rs`
  (deliberately unlocked), `kcore/print.rs`
* **Problem:** some functions must not be called from an interrupt handler
  (everything that takes a lock the interrupted code already holds). Rust
  cannot express that — it stays with comments. The concrete consequence in
  this tree: the serial output runs **without a lock**, so that the panic
  handler does not deadlock (`serial.rs`, module comment). That is a deliberate
  decision that nobody checks.
* **Workaround:** discipline and comments.
* **A language of our own:** **effects in the type system.** A function carries
  "may run in interrupt context" or "takes lock L"; the compiler refuses
  violations. Deadlocks are a statically decidable class of bugs as soon as the
  lock hierarchy is declared.

## L-08 · `transmute` for a function pointer to data

* **File:** `kernel/src/arch/x86_64/selftest.rs:71` (`transmute`)
* **Problem:** the NX self-test has to jump into a data buffer on purpose. Rust
  knows no way to turn an address into a callable pointer other than
  `transmute` — the bluntest tool in the language, the one that permits
  everything.
* **Workaround:** `core::mem::transmute`, tightly encapsulated.
* **A language of our own:** a **targeted** operator "interpret an address as
  code" that permits exactly that and nothing else. A tool that can do
  everything prevents nothing.

## L-09 · Lost type information at the `arch` façade

* **Files:** `kernel/src/arch/mod.rs`, `kcore/arch_iface.rs`
* **Problem:** the arch boundary is built as a trait with associated types
  (`ArchOps::AddressSpace`, `::Timer`, `::IntCtl`). So that the core can use it
  without generics, there are free functions in `arch/mod.rs` that pass through.
  That is duplication by hand: every new trait method has to be entered in three
  places (trait, façade, implementation). It became visible with
  `fault_stack_top()` in this round.
* **Workaround:** discipline; `test.sh` step 15 at least catches leaks.
* **A language of our own:** something like "a module satisfies an interface and
  is selected at build time" (signatures as in ML/OCaml, only with static
  selection) — then the façade goes away without replacement.

## L-10 · No `core` without contrivances: `build-std` and target JSON

* **Files:** `build.sh`, `.cargo/config.toml`, `x86_64-osum-none.json`
* **Problem:** for a target of our own, `core`/`alloc` have to be rebuilt from
  source (`-Z build-std`, unstable). On top of that, `build-std` had to come
  **out** of `.cargo/config.toml`, because `[unstable]` applies to **every**
  target and the host tests then fail on a second `core`
  (`duplicate lang item: sized`) — a tooling problem that cost half an hour of
  searching.
* **Workaround:** `build-std` as a flag in `build.sh`, documented in
  ARCHITECTURE.md § 9.
* **A language / toolchain of our own:** a target is a **description**, not a
  privileged list inside the compiler. The standard library is treated the same
  way for every target — there is no "built-in" and no "self-built" case.

## L-11 · The full register set is hand work — and nobody checks it

* **Files:** `kernel/src/arch/x86_64/preempt.rs:244` (`fn irq0_entry`,
  `naked_asm!`), the struct `kernel/src/arch/x86_64/preempt.rs:90`
  (`struct FullFrame`) in the same file
* **Problem:** for preemption the entry point has to save **all 15** GPRs (the
  cooperative switch gets by with the callee-saved ones). The assembly part
  pushes them in one order; the Rust struct `FullFrame`, with which the
  dispatcher reads them, has to list **exactly the reverse** order. The two do
  stand below each other in one file, but nothing connects them: if somebody
  swaps two lines in the assembly and not in the struct, the kernel reads `r11`
  as `rax` — and only reads it wrong once a tick lands exactly there. The
  compiler sees none of it.
* **Workaround:** a comment above the struct ("reverse push order"), a self-test
  that checks after 60 ticks that all three threads carry on computing intact.
* **A language of our own:** saving registers belongs **derived**, not copied
  out. From a declaration "this entry point receives hardware frame X and saves
  register set Y" the compiler should generate the prologue, the epilogue *and*
  the type with which the frame is read, together. Two descriptions of the same
  fact, kept in sync by hand, are a language deficiency.

## L-12 · System call entry: a convention Rust does not know

* **Files:** `kernel/src/arch/x86_64/user.rs:773` (`fn syscall_entry`),
  `kernel/src/arch/x86_64/user.rs:656` (`fn enter_ring3`),
  `kernel/src/arch/x86_64/user.rs:702` (`fn resume_kernel`),
  `kernel/src/arch/x86_64/user.rs:724` (`fn resume_kernel_after_fault`)
  — four more `naked_asm!` bodies
* **Problem:** on `syscall` the CPU delivers no stack frame but puts the return
  address in `rcx` and the flags in `r11`, and **keeps the caller's stack** —
  the kernel has to switch itself, swap the base register beforehand and go
  back afterwards in exactly the reverse order. Rust has no calling convention
  for that; `extern "C"` would simply be wrong. The result: five naked
  functions in one file in which the compiler can check nothing, and a per-CPU
  block as `static mut` (`kernel/src/arch/x86_64/user.rs:117`
  (`static mut PERCPU`)), because "belongs to this processor core" is no
  category in the ownership model.
* **Workaround:** everything in one file, a detailed sequence comment in the
  module header, measured values (CS, CPL, stack) from a **real** exception
  frame instead of from claims.
* **A language of our own:** see L-01 — calling conventions as a declarable
  language construct. In addition: **the privilege level as an effect.** "This
  function runs on the kernel side of a transition" is checkable as soon as the
  language knows the concept.

## L-13 · A pointer from ring 3 looks like every other pointer

* **Files:** `kernel/src/kcore/user.rs:66` (`fn is_user_addr`),
  `kernel/src/kcore/user.rs:254` (`aus Ring 3 abgewiesen`, message in the log),
  the buffer check in the dispatcher `kernel/src/abi/native.rs`
* **Problem:** a system call gets an address and a length as two `u64`. In the
  type system nothing about it differs from a kernel address — the check "lies
  in its own area, the length does not overflow" is pure discipline at every
  single call site. Forget it in one place and an unprivileged program writes
  into the kernel, and **no tool reports it**.
* **Workaround:** every buffer from ring 3 goes through the same check; the
  negative test stands as a check in the boot log ("Zeiger … aus Ring 3
  abgewiesen").
* **A language of our own:** a continuation of L-05: pointers should carry
  their **address space and their provenance** in the type (`virt@user(pid)`
  vs. `virt@kernel`). A conversion is then a check with a result — not a
  convention you have to remember.

## L-14 · Reading bytes into structs: either `unsafe` or by hand

* **File:** `kernel/src/kcore/initramfs.rs:154` (`fn rd32`), next to it `rd64`
  * *State 2026-08-23:* the ELF side of this entry is gone — the byte readers
    of the ELF header now live in `kernel/firn/elf.fi`. **That does not solve
    the point:** there they stand just as much by hand, only with checked
    arithmetic. Firn lacks the format description as a language construct just
    as much. The archive reader is still in Rust and keeps the entry open.
* **Problem:** an ELF header and an archive entry are byte sequences at fixed
  positions. Rust offers exactly two ways for that: `transmute`/pointer play
  (unsafe, and on wrong alignment simply undefined) or field by field
  `u64::from_le_bytes([b[o], b[o+1], …])`. We chose the second — that is safe,
  but it is eight index accesses for **one** number, and the field offsets
  stand next to it as constants instead of following from a format description.
  A crate like `zerocopy` would be a foreign dependency for something the
  language should be able to do.
* **Workaround:** small readers of our own with a length check beforehand; all
  additions `checked_*`; 11 + 7 negative cases in the boot log as proof.
* **A language of our own:** **format descriptions as a language construct.**
  "This struct lies little-endian from offset N in this buffer" should be
  declared, from which the compiler generates the check and the access — safe,
  without an alignment assumption and without a foreign crate.

## L-15 · You must not allocate in an interrupt — only the comment says so

* **Files:** `kernel/src/kcore/preempt.rs`, `kernel/src/kcore/sched.rs:734`
  (`static mut ACTIVE`)
* **Problem:** the timer entry point decides on a thread switch. In doing so it
  may neither touch the heap (which takes locks) nor take a lock that the
  preempted code is currently holding. The scheduler is therefore reachable
  through a raw pointer instead of through a lock — it works, but it is again
  only discipline. Rust cannot express "this function does not allocate";
  `#[no_alloc]` does not exist.
* **Workaround:** the preempting path demonstrably does not allocate (all
  thread stacks come into being at `spawn`, not in the tick), a comment in the
  module header.
* **A language of our own:** an effect system as in L-07, but with the category
  **allocation**. "Must not allocate" is the most common unwritten rule in a
  kernel and belongs in the signature.

---

## Patterns that are emerging

After phases 1–3 the same three themes repeat — now with more evidence:

1. **Address spaces and kinds of pointer** (L-05, L-08, **L-13**) — Rust knows
   exactly one kind of pointer. A kernel needs at least four (physical,
   virtual in its own space, virtual in a foreign space, **handed in from ring
   3**) and does not want to be allowed to confuse them. With ring 3 a
   convenience problem has turned into a security problem.
2. **Hardware ownership and single instances** (L-04, **L-12**) — objects whose
   owner is the CPU fit into no ownership model that only knows parts of a
   program. The per-CPU block is the third case of the same kind.
3. **Execution context as an effect** (L-03, L-07, **L-11**, **L-12**,
   **L-15**) — "runs in an interrupt", "switches the context", "holds lock L",
   "does not allocate", "runs on the kernel side of a privilege transition" are
   checkable properties that today all stand only in comments. This pattern has
   grown the most clearly in this round.

Newly added and (not yet) a pattern, but noted: **format descriptions** (L-14)
— an ELF header, an archive entry, a page table entry are all "bytes at fixed
offsets"; read out by hand three times.

Five new entries in one round, all from the same occasion (preemption, ring 3).
Fifteen entries are not yet a compulsion towards a language of our own — but
the direction is by now unambiguous.

## What speaks against a language of our own (that belongs here too)

* Rust did **not prevent** 413 `unsafe` occurrences, but it **made them
  visible**. Every one of those places is marked, justified and findable. That
  is more than C ever offered.
* The borrow checker really did prevent bugs in `libs/osum-mem` (bitmap, heap,
  regions) and in `libs/osum-abi-native` (the handle table) — that is where the
  logic lies, that is where it is valuable, that is where 151 host tests run.
  The capability rules of this round were fully tested on the host **before**
  the kernel executed them for the first time.
* A language of our own means: a compiler, an optimiser, debugger formats, a
  formatter, editor support, a library. Every hour on that is an hour not spent
  on the operating system.
* As long as the list above has ten entries and not a hundred, the honest
  answer is: **Rust is enough.**

## L-16 · An exception frame belongs to a stack — the language does not know it

* **Files:** `kernel/src/arch/x86_64/user.rs:162` (`const TRAP_GAP`),
  `kernel/src/arch/x86_64/preempt.rs:60` (`static USER_PREEMPTIONS`),
  `kernel/src/kcore/user.rs:510` (`fn run_preemptible`)
* **Problem:** for a program in ring 3 to be preempted and later **resumed**,
  the frame the CPU lays down at the privilege change has to lie on the kernel
  stack of **exactly the thread** that carries the program. If it lies on a
  shared stack, the next switch overwrites it — and the program returns into a
  state it never had. This binding "frame ↔ stack ↔ thread" is nowhere visible
  in the type system: `TSS.rsp0` is a bare number, and whether it currently
  points at the right stack is known only to the human. A mistake here does not
  show up as a crash at the place of the mistake but as a sporadically wrong
  register milliseconds later.
* **Workaround:** the stack is derived at entry into ring 3 from the own stack
  pointer (`TRAP_GAP` distance) and reset afterwards; a switch
  (`set_preemptible`) separates the two operating modes hard, and the proof
  runs along in every boot (12 interruptions, the program still ends with code
  0).
* **A language of our own:** stacks should be **types with a capacity and an
  owner**, not `u64`. "This exception lays its frame down on stack S, S belongs
  to thread T, T is currently scheduled" is a statement a systems language
  should be able to check. As long as it cannot, preemption in ring 3 is hand
  work with very sharp edges.

## L-17 · `swapgs`: global state that no type describes

* **Files:** `kernel/src/arch/x86_64/user.rs:688` (`swapgs` at the entry
  point), `kernel/src/arch/x86_64/preempt.rs:244` (`fn irq0_entry`,
  deliberately **without** `swapgs`)
* **Problem:** whether `gs:` currently points at the kernel block or at the
  program block depends on how often `swapgs` was executed on the way here.
  That is hidden global state: the same Rust function is correct or not
  depending on the call path. Rust has no concept for it, and `unsafe` says
  only "something dangerous happens here", not "state X holds here".
* **Workaround:** exactly one file touches `swapgs`, the module header
  describes every path individually, and the preemption path deliberately does
  without it (it does not use `gs:`).
* **A language of our own:** something like that belongs in the signature as an
  **effect / state index**: `fn dispatch(...) requires gs = kernel`. The
  compiler then refuses the call from a path that has not established the state
  — instead of a human keeping three call paths in his head.
