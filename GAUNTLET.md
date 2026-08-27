# Gauntlet log — orientos
**Goal:** IMPORTANT, READ FIRST: PREFLIGHT.md, ARCHITECTURE.md, ROADMAP.md, README.md, test.sh. This project IS ALREADY BUILT, compiles without warnings and demonstrably BOOTS in QEMU (BIOS and UEFI). `./test.sh` currently runs green with 15/15 steps. Do NOT start over, do NOT throw anything away, do not delete a working file or replace it with a foreign crate. Whoever breaks the boot has produced a total failure, not progress. Building continues ON THIS STATE.

GOAL OF THIS ROUND: take OrientOS from a mere boot kernel to a system with a REAL USERSPACE. Four milestones:

1) PREEMPTIVE SCHEDULER
   - The timer interrupt (PIT 100 Hz, already present) calls the scheduler. So far there is only a cooperative yield in kcore/sched.rs with context.rs (naked_asm) — that stays as the base but is extended by real preemption.
   - At the interrupt entry the FULL register set has to be saved (so far only callee-saved): all GPRs, a correct stack layout, a clean iretq. The register-saving code belongs in kernel/src/arch/x86_64/, the scheduler trait and the selection logic in kcore.
   - Time slices (a quantum in ticks) and priorities behind THE SAME scheduler trait — no second parallel scheduler.
   - PROOF in the boot log: several kernel threads that NEVER call yield() voluntarily (e.g. pure counting loops) still take turns measurably; the log shows the number of preemptive switches, the distribution of ticks per thread and that priorities take effect (a high priority gets more ticks). As a self-test switch of its own in run-qemu.sh, analogous to the existing --test-*.

2) RING 3 / USERSPACE
   - Enable syscall/sysret: the MSRs STAR/LSTAR/SFMASK/EFER.SCE, swapgs with a per-CPU block (GS base), separate kernel/user stacks, TSS.RSP0 set correctly and updated on a thread switch.
   - User pages: own pages with the USER bit, kernel pages WITHOUT the USER bit (an access from ring 3 has to trigger a #PF — test that negatively!). Use SMEP/SMAP when CPUID reports them (CR4.SMEP/SMAP, stac/clac where needed), otherwise skip cleanly and note it in the log.
   - A first userspace program really runs in ring 3, prints something through a syscall and exits cleanly; the kernel clears it away and keeps running.
   - PROOF in the boot log: the CS selector and CPL of the running user program (e.g. CS=0x2b, CPL=3), RSP in the user area, and a negative test: a ring 3 access to a kernel address ends in a cleanly reported #PF instead of a crash.

3) ELF64 LOADER
   - An initramfs (a simple, documented format or cpio/tar — justify the choice) is embedded into the ISO as a Limine module and found by the kernel.
   - Load statically linked ELF64 binaries out of it: parse the program headers, map the PT_LOAD segments (rights from p_flags: RX/R/RW+NX), zero the .bss, set up the entry address and the user stack, start it.
   - Robust against garbage: wrong magic, wrong class/endianness, overlapping or implausible segments, a segment outside the user address space -> a defined error, NO kernel crash. Tested negatively.
   - The userspace program from (2) is to come out of the initramfs as a real ELF, not as an embedded byte array (the latter at most as an intermediate step).

4) EXTEND THE osum-native ABI (capability/handle based)
   - A handle table PER PROCESS: handles are unforgeable (index + generation/nonce, no raw pointer casting), rights per handle (a bit mask), no global path namespace in the core, NO fork — process creation in spawn style with EXPLICIT handing over of handles.
   - Syscalls in the osum-native style: handle-oriented, error codes as a type of their own (no errno), no ambient authority.
   - A NEGATIVE TEST is mandatory: a process tries to use a handle it does not have (wrong index, stale generation, missing right) -> a defined error, no access, visible in the boot log.
   - The POSIX layer (kernel/src/abi/posix, feature "posix") stays a PURE TRANSLATOR on top and has to remain completely deselectable: the kernel has to build AND boot with --no-default-features. That is a test step of its own in test.sh and must NOT get lost.

CONSTRAINTS (not negotiable):
- Everything architecture-specific strictly in kernel/src/arch/x86_64/, behind it the arch trait interface in kernel/src/kcore/arch_iface.rs. In kernel/src outside arch/ NO x86 detail may turn up (cr0-3, PML4, PTE, rdmsr/wrmsr, lgdt/lidt, in/out, asm!, invlpg, iretq, swapgs, syscall MSRs). test.sh step 15 checks that with grep — it has to stay green.
- The product name comes EXCLUSIVELY from kernel/src/kcore/branding.rs (fed from the Cargo metadata via build.rs). No hard-coded "osum"/"OrientOS" in kernel/src outside branding.rs — test.sh step 14 checks that.
- LIGHTWEIGHT: currently only 2 external crates (limine, spin). Every new external crate needs a hard justification in ARCHITECTURE.md. No replacing of the own heap/bitmap allocator or the own page tables by foreign crates (e.g. NOT the x86_64 crate, NOT linked_list_allocator).
- NO todo!()/unimplemented!() in the tree. ZERO compiler warnings (build.sh enforces -D warnings).
- Do not touch vendor/limine/. No cosmetic renamings, no reformatting of the whole tree.
- Nightly features only when unavoidable — and EVERY new case is logged in LANGUAGE.md (file, line, problem, workaround, what a language of our own would have to do differently). The same for every new kind of unsafe block.
- Bring the documentation along: ARCHITECTURE.md (new layers: sched, user, elf, handle), ROADMAP.md (tick off finished items, name the next ones honestly), README.md (the boot log excerpt has to match a REAL current run — measure the figures, do not guess them), keep PACKAGING.md/FILESYSTEM.md/LANGUAGE.md current.
- test.sh grows by steps for: preemptive switches, the ring 3 program, the ELF loader including negative tests, the handle negative test. At the end ./test.sh has to run COMPLETELY green.

IF SOMETHING DOES NOT COME TO RUN STABLY: better leave it out and list it honestly as open in ROADMAP.md than deliver a broken or non-booting kernel. A green boot is worth more than half a feature.
**The bar:** the measurement is taken against an ALREADY WORKING, booting kernel. Making it worse is the worst failure.

(0) KNOCKOUT CRITERION: `./test.sh` has to be COMPLETELY green at the end (currently 15 steps, meant to grow). The jury MUST run `./test.sh` ITSELF and quote the output; "should run" does not count. If a step is red or the kernel no longer boots: at most 25 points, no matter how good the rest looks.

(a) RING 3 FOR REAL? A program really running in ring 3 has to be demonstrable in the boot log: the CS selector with RPL=3 and CPL=3 logged in plain text, the user RSP in the user address range, a return through syscall/sysret. A "user program" that in truth runs in ring 0 is a total failure of this criterion (0 points for it). A negative test is mandatory: a ring 3 access to a kernel address -> a cleanly reported #PF, no crash.

(b) PREEMPTION FOR REAL? Context switches have to happen WITHOUT a voluntary yield — threads that only count take turns measurably. The log shows the number of preemptive switches (from the timer handler) separately from the cooperative ones. The full GPR set is saved (verifiable in the source text, not just claimed). Priorities have to take effect MEASURABLY (tick distribution in the log).

(c) CAPABILITIES FOR REAL? A handle table per process, handles with a generation/nonce (not forgeable by guessing an index), rights per handle. A NEGATIVE TEST has to be in the boot log: an invalid index, a stale generation and a missing right are each refused individually. If a process reaches a resource without a matching handle: a massive deduction. No global path namespace in the core, no fork.

(d) IS POSIX SEPARABLE? `cargo build --no-default-features` AND a boot without POSIX have to work, and the log reports "posix-Schicht NICHT einkompiliert". A test.sh step of its own. If POSIX semantics leak (errno, fork, inode/dentry) into kcore/, that is a serious failure.

(e) IS THE ARCH BOUNDARY TIGHT? `grep -rnE '\b(cr[0-4]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq|swapgs|sysret|STAR|LSTAR|SFMASK|SMEP|SMAP)\b' kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'` has to be EMPTY (explanatory comments in arch_iface.rs excepted and marked as such). main.rs must not call anything architecture-specific directly either. Every leak costs drastically.

(f) THE NAME ONLY FROM branding.rs? `grep -rniE 'osum' kernel/src --include='*.rs' | grep -v branding.rs` has to be empty. And `./rename.sh` has to work demonstrably in a copy under /tmp.

(g) CLEANLINESS: no todo!()/unimplemented!(), ZERO compiler warnings (the jury checks the build log), no new external crates without a justification in ARCHITECTURE.md, no replacing of own core pieces (heap, bitmap allocator, page tables) by foreign crates.

(h) DOES THE DOCUMENTATION MATCH THE CODE: ARCHITECTURE.md, ROADMAP.md, README.md, PACKAGING.md, FILESYSTEM.md, LANGUAGE.md have to describe the ACTUAL state. The jury takes SAMPLES: every figure in the README (kernel size, frames, lines, test count) and every boot log excerpt has to match a REAL current run. Claims without code behind them = a massive deduction. LANGUAGE.md has to contain the Rust friction points that newly turned up in THIS round (naked_asm, syscall ABI, unsafe, allocator).

(i) PROGRESS: measured by preemptive switches, real ring 3, the ELF loader with negative tests, handle negative tests — all of it evidenced in the boot log. Pure comment cosmetics, reformatting or new documentation without code does NOT count as progress.
**Best score:** 72/100 (round 3)
**Agents:** 22 · **Duration:** 13967s
**Round snapshots:** one git commit + tag per round (gauntlet-r<N>-score<S>). Best state: branch `gauntlet-best` (round 3) — switch with `git checkout gauntlet-best`, back with `git checkout -`.
**Round 0 — architecture**: 4 modules (teil-1, teil-2, teil-3, teil-4) — an existing project
**Round 1** — score n/a/100 (target 88)
  ⚠ 1 builder failed: teil-2 (Claude Code process aborted by user)
**Round 2** — score n/a/100 (target 88)
**Round 3** — score 72/100 (target 88)
  Defects: • The knockout proof is missing for the CURRENT tree: the only complete test.sh run (build/FINAL-test.log, 19/19 green) is from 15:51, after which kernel/src/abi/native.rs, kcore/sched.rs, kcore/preempt.rs, arch/x86_64/user.rs, main.rs and others were changed (16:33-16:52) and libs/osum-abi-native/src/name.rs was added as new (untracked); `git status` is completely dirty, there is no log of a test.sh run against this state  • Freedom from warnings (g) is only apparently evidenced: build/cargo-build.log contains exactly one line 'Finished `release` profile ... in 0.09s' — an incremental no-op build. test.sh lines 122-132 grep in exactly this file for '^warning:' and are thereby trivially green; if the file goes away, the check is even silently skipped  • Preemption does NOT cover ring 3: arch/x86_64/preempt.rs:169-199 does not switch the context at all when CS&3==3, it only counts, or clears the program away with a watchdog ('Ein Wechsel MIT spaeterer Fortsetzung ... ist noch nicht moeglich'). Unprivileged programs are therefore not preemptible-and-resumable  • The SMEP/SMAP path is dead code in every test run: run-qemu.sh sets no `-cpu ...,+smep,+smap` (not a single occurrence of -cpu), the boot log reports 'Ausfuehrsperre nein (uebersprungen), Zugriffssperre nein (uebersprungen)' — the CR4 protection logic in arch/x86_64/user.rs is never executed  • LANGUAGE.md does not match the code: 'preempt.rs:158 (irq0_entry)' -> actually line 217; 'FullFrame ab Zeile 47' -> line 76; 'user.rs:453 (syscall_entry)' -> line 697; 'Per-CPU-Block als static mut (user.rs:91)' -> line 117; 'kcore/user.rs:57 (is_user_addr)' -> line 66; 'Messwerte: 413 unsafe-Vorkommen in kernel/src' -> currently 428
