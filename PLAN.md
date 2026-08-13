# PLAN — Runde 3: vom Boot-Kern zum System mit echtem Userspace

Dieser Lauf **erweitert** einen bereits bootenden Kernel. Ausgangszustand:
`PREFLIGHT.md` (15 Schritte gruen, BIOS + UEFI). Nichts wegwerfen, nichts neu
bauen, keine funktionierende Datei ersetzen. **Ein gruener Boot ist mehr wert
als ein halbes Feature.**

---

## 0. Was der Lead in diesem Lauf schon gebaut hat (steht, ist gruen)

Alle Nahtstellen der vier Meilensteine sind **angelegt, verdrahtet und im
Boot-Log sichtbar**. Kein `todo!()`, keine Warnung, `./test.sh` bleibt gruen.
Damit kann jedes Modul sofort in SEINER Datei loslegen, ohne auf ein anderes
zu warten.

| Naht | Datei | Aufruf aus | Zustand heute |
|---|---|---|---|
| `ArchOps::{set_preemption, preemption_available}` | `kcore/arch_iface.rs` → `arch/x86_64/preempt.rs` | `kcore::preempt` | meldet `nicht verfuegbar` |
| `kcore::preempt::{on_tick, note_*_switch, quantum, enable}` | `kcore/preempt.rs` (neu) | Zeitgeberpfad | Zaehler laufen, `on_tick` liefert Zeitscheibenende |
| `kcore::sched::preempt_selftest()` | `kcore/sched.rs` | `kcore::preempt::boot_selftest` | Platzhalterzeile, liefert `(0,0)` |
| `ArchOps::{cpu_features, init_user_support, user_support_ready, set_kernel_stack, enter_user, code_selector}` + `CpuFeatures`, `UserExit` | `kcore/arch_iface.rs` → `arch/x86_64/user.rs` | `kcore::user` | meldet `nicht verfuegbar` |
| `kcore::user::{USER_BASE, USER_LIMIT, is_user_addr, init, run, report_exit, boot_selftest}` | `kcore/user.rs` (neu) | `kmain` | loggt CPU-Schutzmerkmale ehrlich |
| `kcore::elf::{parse, Image, Segment, ElfError, selftest}` | `kcore/elf.rs` (neu) | `kmain` | **fertig und gruen: 11/11 Negativfaelle** |
| `kcore::initramfs::{set, image, find, count, report}` | `kcore/initramfs.rs` (neu) | `kmain` | meldet `kein Archiv uebergeben` |
| `abi::native::capability_selftest()` | `abi/native.rs` | `kmain` | **fertig und gruen: 3/3 Handle-Negativtests** |
| Selbsttestbilanz erweitert | `main.rs` | — | `93/93 bestanden (… Praeemption 0/0, ELF 11/11, Ring3 0/0, Handles 3/3)` |
| Features `test-preempt/test-ring3/test-elf/test-handles` | `kernel/Cargo.toml` | — | angelegt, Schalter in `run-qemu.sh` vorhanden |
| Log-Vertrag je Feature | `run-qemu.sh` (`case "$FEATURES"`) | CI | Muster stehen, warten auf die Module |
| Testschritte als Einzeldateien | `test.sh` + `tests/step-*.sh` | CI | `test.sh` zaehlt automatisch; jedes Modul legt SEINE Datei an |

Gemessener Stand nach dem Skelett (echter Lauf, `./run-qemu.sh --check`):

```
[karst] elf        ELF-Negativtest: 11/11 Faelle wie erwartet abgewiesen
[karst] abi        Handle-Negativtest: 3/3 abgewiesen (ungueltiger Index ok,
                   veraltete Generation ok, fehlendes Recht ok)
[karst] boot       Selbsttestbilanz: 93/93 bestanden
```

---

## 1. Unverrueckbare Regeln (gelten fuer JEDES Modul)

1. Nach jeder Aenderung: `./build.sh && ./run-qemu.sh --check`. Vor der
   Abgabe: `./test.sh` komplett. Rot = Aenderung zuruecknehmen.
2. **Arch-Grenze.** Kein x86-Begriff ausserhalb `kernel/src/arch/`:
   `grep -rnE '\b(cr[0-4]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq|swapgs|sysret|STAR|LSTAR|SFMASK|SMEP|SMAP)\b' kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'`
   muss leer bleiben (Schritt 15 prueft das). Auch `main.rs`.
3. **Produktname** nur aus `kcore/branding.rs` (Schritt 14).
4. Null Compilerwarnungen (`-D warnings`), kein `todo!()`/`unimplemented!()`.
5. Keine neue externe Crate ohne Begruendung in `ARCHITECTURE.md` § 8.
   Aktuell zwei: `limine`, `spin`. Eigener Heap/Bitmap/Seitentabellen bleiben.
6. `--no-default-features` muss **bauen und booten**. Nichts POSIX-artiges
   (errno, fork, inode) in `kcore/`.
7. Jeder neue Rust-Reibungspunkt (naked_asm, ABI, unsafe-Sorte, Allocator) wird
   in `LANGUAGE.md` protokolliert: Datei, Zeile, Problem, Workaround, was eine
   eigene Sprache anders machen muesste.
8. `vendor/limine/` nicht anfassen. Keine Umbenennungen, keine Umformatierung.
9. **Dateibesitz beachten** (Abschnitt 3). Wer eine fremde Datei anfasst,
   erzeugt einen Konflikt und damit Arbeit fuer alle.

---

## 2. Log-Vertrag (das misst die Jury, das prueft `run-qemu.sh`)

Die Muster stehen bereits in `run-qemu.sh` im Block `case "$FEATURES"`. Wer
sein Modul baut, baut gegen genau diese Zeilen:

**preempt** (`--test-preempt`)
```
Praeemption : aktiv, Zeitscheibe N Tick(s)
praeemptive Wechsel: N, kooperative Wechsel: M
ohne yield: N Wechsel zwischen 3 Threads
Thread 1: Prio 2, 41 Ticks
Prioritaet wirkt: Thread 1 (Prio 2) 41 Ticks > Thread 2 (Prio 1) 20 Ticks
```

**ring3** (`--test-ring3`)
```
Ring 3      : CS=0x002b (RPL=3), CPL=3, Stapel=0x00007fff..., N Systemaufruf(e), Ende 0
Ring-3-Zugriff auf Kerneladresse 0xffffffff8...: sauber abgewiesen (Schutzverletzung, Userspace)
```

**elf** (`--test-elf`)
```
Initramfs   : 2 Eintraege, 8192 B
ELF-Lader   : hello, 2 Segmente geladen, Einsprung 0x0000000000401000
  Segment 0: 0x401000 4 KiB RX
ELF-Negativtest: 11/11 Faelle wie erwartet abgewiesen
```

**handles** (`--test-handles`)
```
Handle-Negativtest: 3/3 abgewiesen (ungueltiger Index ok, veraltete Generation ok, fehlendes Recht ok)
```

Zusaetzlich gilt fuer JEDEN Lauf: keine Zeile mit `FEHLER` oder `WARNUNG`, die
Selbsttestbilanz muss `N/N` sein, und `Startvorgang abgeschlossen` muss kommen.

---

## 3. Dateibesitz — wer was anfassen darf

**Gesperrt (Lead, niemand sonst aendert sie):**
`kernel/src/main.rs`, `kernel/src/kcore/mod.rs`, `kernel/src/kcore/arch_iface.rs`,
`kernel/src/arch/mod.rs`, `kernel/src/arch/x86_64/mod.rs`, `kernel/Cargo.toml`,
`Cargo.toml`, `run-qemu.sh`, `test.sh`, `PLAN.md`.
Fehlt dort etwas, wird es beim Lead angemeldet — nicht selbst geaendert.

| Modul | darf ausschliesslich diese Dateien aendern/anlegen |
|---|---|
| **preempt** | `kernel/src/arch/x86_64/preempt.rs`, `.../idt.rs`, `.../timer.rs`, `kernel/src/kcore/preempt.rs`, `kernel/src/kcore/sched.rs`, `kernel/src/arch/x86_64/context.rs`, `tests/step-16-preempt.sh` |
| **ring3** | `kernel/src/arch/x86_64/user.rs`, `.../gdt.rs`, `.../interrupts.rs`, `.../msr.rs`, `.../cpu.rs`, `kernel/src/kcore/user.rs`, `tests/step-17-ring3.sh` |
| **elf** | `kernel/src/kcore/elf.rs`, `kernel/src/kcore/initramfs.rs`, `kernel/src/boot/limine.rs`, `kernel/src/boot/mod.rs`, `kernel/src/mm/mod.rs`, `userland/**` (neu), `build.sh`, `limine.conf`, `tests/step-18-elf.sh` |
| **handles** | `kernel/src/abi/**`, `libs/karst-abi-native/**`, `libs/karst-abi-posix/**`, `tests/step-19-handles.sh` |
| **doku** | `ARCHITECTURE.md`, `ROADMAP.md`, `README.md`, `PACKAGING.md`, `FILESYSTEM.md`, `LANGUAGE.md`, `RENAME.md`, `RUN.md` — und **keine** Quelldatei |

Ueberschneidungen sind bewusst ausgeschlossen: `preempt` besitzt den
Interrupt-EINSPRUNG (`idt.rs`, eigener Einsprung in `preempt.rs`), `ring3`
besitzt die Ausnahme-HANDLER (`interrupts.rs`) und die Deskriptortabellen
(`gdt.rs`).

---

## 4. Reihenfolge und Abhaengigkeiten

* **preempt**, **ring3**, **handles** koennen sofort und unabhaengig starten.
* **elf** liefert `kcore::initramfs::find("hello")`; **ring3** benutzt das,
  sobald es da ist. Bis dahin darf **ring3** ein eingebettetes Byte-Array
  als Zwischenschritt benutzen — am Ende muss das Programm aus dem Archiv
  kommen, sonst wird der Zwischenschritt in ROADMAP.md offen gefuehrt.
* **handles** braucht von **ring3** nur den Verteilereinstieg
  `abi::native::dispatch(nr, args) -> i64` (existiert bereits, Signatur bleibt).
* **doku** misst am Ende nach: jede Zahl im README (Kernelgroesse, Frames,
  Zeilen, Testanzahl) und jeder Log-Auszug muss aus einem ECHTEN Lauf stammen.

## 5. Wenn etwas nicht stabil laeuft

Weglassen, im Log ehrlich `nicht verfuegbar` melden, den Punkt in `ROADMAP.md`
als offen fuehren — und den zugehoerigen `tests/step-*.sh` NICHT anlegen.
Ein nicht bootender Kernel ist ein Totalausfall, ein ehrlich offener Punkt
nicht.
