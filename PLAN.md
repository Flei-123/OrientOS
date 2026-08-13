# PLAN — Haertungslauf von karst / Karstos

Dieser Lauf **erweitert** einen bereits bootenden Kernel. Ausgangszustand siehe
`PREFLIGHT.md`; er war vor Beginn dieses Laufs mit `./test.sh` nachweislich
gruen (9 Schritte, 24 Host-Tests, 6 echte QEMU-Boots).

## 0. Was der Lead in diesem Lauf bereits gebaut hat (steht, ist gruen)

Damit die Module sich nicht ins Gehege kommen, sind die **Nahtstellen bereits
angelegt und in Betrieb**. Jede davon ist echter, laufender Code — kein
Platzhalter. Nachgewiesen im Boot-Log (`./run-qemu.sh --check`):

| Naht | Datei | Aufruf aus | Log-Zeile heute |
|---|---|---|---|
| `mm::selftest::map_errors(&mut space)` | `kernel/src/mm/selftest.rs` (neu) | `kmain` | `Selbsttest MapError: 4/4 Fehlerfaelle nachweisbar ausgeloest` |
| `mm::frame::selftest()` | `kernel/src/mm/frame.rs` | `kmain` | `Selbsttest Frames: 4/4 Zusagen erfuellt, N frei` |
| `mm::heap::selftest()` | `kernel/src/mm/heap.rs` | `kmain` | `Selbsttest Heap: 3/3 Zusagen erfuellt, ...` |
| `#[alloc_error_handler] on_oom` | `kernel/src/mm/heap.rs` | Laufzeit | `AUSSER SPEICHER: ...` + `Heapzustand : ...` |
| `Heap::stats() -> HeapStats` | `libs/karst-mem/src/heap.rs` | ueberall | — |
| `kcore::sched::{Scheduler, ThreadId, ThreadState, boot_selftest}` | `kernel/src/kcore/sched.rs` (neu) | `kmain` | `kein Scheduler aktiv (Phase 2 offen) — ...` |
| `arch::selftest::rodata_write()` + Feature `test-rodata` | `kernel/src/arch/x86_64/selftest.rs`, `arch/selftest.rs` | `kmain` (cfg) | `#PF`, `Ursache : Schutzverletzung, Schreibzugriff` |
| `./run-qemu.sh --test-rodata`, `test.sh` Schritt 9/10 | `run-qemu.sh`, `test.sh` | CI | `[ ok ] Schreiben auf .rodata gibt #PF` |

`./test.sh` hat jetzt **10 Schritte**. Alle Module bauen auf diesem Zustand auf.

## 1. Unverrueckbare Regeln

1. `./build.sh && ./run-qemu.sh --check` nach **jeder** Aenderung; `./test.sh`
   vor der Abgabe. Rot = Abgabe zuruecknehmen, nicht "spaeter reparieren".
2. Keine x86-Begriffe ausserhalb `kernel/src/arch/`. Probe:
   `grep -rnE '\b(cr[0-3]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq)\b' kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'` muss leer sein.
3. Keine neue externe Crate (auch keine dev-dependency: Eigenschaftstests mit
   einem selbst geschriebenen LCG-Zufallsgenerator, nicht mit `proptest`).
4. `std` nur unter `#[cfg(test)]`. `vendor/limine/` nicht anfassen.
5. Nur die dir zugewiesenen Dateien aendern. Keine Umformatierung fremder Teile.
6. Nichts dokumentieren, was nicht im Code steht.

## 2. Dateibesitz (ueberschneidungsfrei)

| Modul | Darf **ausschliesslich** diese Dateien aendern/anlegen |
|---|---|
| `hosttests` | `libs/karst-mem/**`, `libs/karst-abi-native/**`, `libs/karst-abi-posix/**` |
| `mmharden` | `kernel/src/mm/frame.rs`, `kernel/src/mm/heap.rs`, `kernel/src/mm/mod.rs`, `kernel/src/kcore/mem.rs` |
| `pagetest` | `kernel/src/mm/selftest.rs`, `kernel/src/arch/x86_64/paging.rs` |
| `bootdiag` | `run-qemu.sh`, `test.sh`, `kernel/src/main.rs`, `kernel/src/kcore/panic.rs`, `kernel/src/arch/selftest.rs`, `kernel/src/arch/x86_64/selftest.rs`, `kernel/Cargo.toml` |
| `sched` | `kernel/src/kcore/sched.rs`, `kernel/src/arch/x86_64/context.rs` (neu), `kernel/src/arch/x86_64/mod.rs`, `kernel/src/kcore/arch_iface.rs` |
| `docs` | `README.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `PREFLIGHT.md` |

Niemand sonst fasst `kernel/src/main.rs` an: die Hooks stehen bereits drin.
Wer eine neue Funktion braucht, haengt sie an eine **bestehende** Naht (Tabelle
in § 0), statt `kmain` zu editieren.

## 3. Log-Zeilen-Vertrag

`bootdiag` prueft in `run-qemu.sh` auf feste Muster. Diese Praefixe duerfen von
den erzeugenden Modulen **nicht umbenannt** werden (Zahlen dahinter frei):

* `Selbsttest MapError: <ok>/<n> Fehlerfaelle nachweisbar ausgeloest` (pagetest)
* `Selbsttest Frames: <ok>/<n> Zusagen erfuellt` (mmharden)
* `Selbsttest Heap: <ok>/<n> Zusagen erfuellt` (mmharden)
* `Selbsttest Paging: ... schreiben/lesen ok, translate ok, unmap ok` (bestehend)
* `sched` -Zeile: entweder `kein Scheduler aktiv` **oder**, wenn `sched` liefert,
  `Threadwechsel: <n> Wechsel, zurueck in kmain` (sched)

## 4. Modulaufträge (Kurzfassung, Details im JSON-Rueckgabewert)

1. **hosttests** — deutlich mehr Host-Tests inkl. Eigenschaftstests gegen ein
   Referenzmodell (eigener LCG), Grenzfaelle in `bitmap`, `heap`, `region`,
   `handle`, `rights`, `syscall`, `errno`.
2. **mmharden** — Fehlerpfade von Frame-Allocator und Heap im Kernel haerten:
   erschoepfter Speicher, zerrissene/leere Memory-Map, fehlendes HHDM, OOM-Bericht.
3. **pagetest** — Randfaelle von `map`/`unmap`/`translate` und alle `MapError`-
   Varianten (inkl. `ParentHugePage`) im laufenden Kernel nachweisen.
4. **bootdiag** — Boot-Log verdichten, `--check` um weitere Merkmale erweitern,
   `test.sh` sauber halten, Panic-/Selbsttestausgabe pruefbar machen.
5. **sched** — Phase 2: Kontextwechsel, `Thread` mit eigenem Kernelstapel,
   kooperativer Scheduler hinter dem kcore-Trait. **Im Zweifel weglassen.**
6. **docs** — ARCHITECTURE/ROADMAP/README/PREFLIGHT auf den tatsaechlichen
   Stand ziehen, Kennzahlen messen statt schaetzen.

## 5. Reihenfolge / Abhaengigkeiten

Alle sechs Module koennen parallel starten. `docs` misst am Ende (nach den
anderen) und darf nur beschreiben, was dann wirklich im Baum steht.
