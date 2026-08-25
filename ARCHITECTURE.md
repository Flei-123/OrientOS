# Architektur von OrientOS / osum

Stand: Phase 1–3 (bootfähiger Kern, verdrängender Scheduler, Ring 3, ELF-Lader,
capability-basierte ABI). Dieses Dokument beschreibt **den gebauten
Zustand**, nicht Absichten. Alles, was hier steht, ist im Code nachprüfbar; wo
etwas noch fehlt, steht es ausdrücklich als „noch nicht".

---

## 1. Leitgedanke

OrientOS ist ein **modularer Monolith**: ein einziges Kernel-Binary (Performance,
keine IPC-Kosten auf jedem Systemaufruf), aber mit Modulgrenzen, die so scharf
sind, dass sich einzelne Subsysteme später **ohne Umbau des Rests** in den
Userspace ziehen lassen.

Drei Regeln machen das durchsetzbar:

1. **Architekturcode ist eingesperrt.** Register, Instruktionen, Portadressen und
   PTE-Bits existieren ausschließlich unter `kernel/src/arch/<arch>/`.
2. **POSIX ist ein Aufsatz, kein Fundament.** Der Kern kennt kein `errno`, kein
   `fork`, keine Signale und keine Dateideskriptoren.
3. **Reine Logik wandert in host-testbare Crates.** Was ohne Hardware auskommt
   (Allokatoren, Regionenrechnerei, ABI-Definitionen), liegt unter `libs/` und
   wird mit `cargo test` auf dem Entwicklungsrechner geprüft — nicht erst im
   Emulator.

---

## 2. Schichten

```
                    ┌──────────────────────────────────────────┐
                    │  abi/native   (osum-native, capability)  │
   Userspace-Sicht  ├──────────────────────────────────────────┤
                    │  abi/posix    ← nur mit Feature "posix"   │
                    └──────────────────────────────────────────┘
                                      │  benutzt nur osum-native
   ┌───────────────┬───────────────┬──┴────────────┬───────────────┐
   │   kcore       │      mm       │   drivers     │     boot      │
   │ Typen, Traits │ Frames, Heap, │ Framebuffer-  │ Limine →      │
   │ print, panic  │ Adressräume   │ Konsole, Font │ neutrale Typen│
   └───────────────┴───────────────┴───────────────┴───────────────┘
                                      │  ausschließlich über Traits
                    ┌─────────────────┴────────────────────┐
                    │  arch/x86_64  (GDT, IDT, PIC, PIT,   │
                    │  Seitentabellen, UART, CPUID, MSR)   │
                    └──────────────────────────────────────┘
```

| Schicht | Verzeichnis | Darf kennen | Darf **nicht** kennen |
|---|---|---|---|
| `kcore` | `kernel/src/kcore/` | `core`, `alloc`, `libs/*` | jede Architektur, POSIX |
| `kcore/sched`+`preempt` | `kernel/src/kcore/` | Zeitscheiben, Prioritäten, Wechselzähler | Registernamen, Interruptvektoren |
| `kcore/user` | `kernel/src/kcore/user.rs` | *ob* und *was* unprivilegiert läuft | *wie* der Privilegwechsel geht |
| `kcore/elf`+`initramfs` | `kernel/src/kcore/` | Dateiformate, Ladeaufträge | Seitentabellenbits |
| `arch` | `kernel/src/arch/` | alles Hardwarenahe | `abi`, POSIX |
| `mm` | `kernel/src/mm/` | `kcore`, `arch`-Traits | PTE-Bits, CR3, `asm!` |
| `drivers` | `kernel/src/drivers/` | `kcore`, `arch` | `abi`, POSIX |
| `boot` | `kernel/src/boot/` | Bootprotokoll, `kcore` | Hardwareinitialisierung |
| `abi/native` | `kernel/src/abi/native.rs` | `kcore`, `mm` | POSIX |
| `abi/posix` | `kernel/src/abi/posix.rs` | **nur** `osum-abi-native` | Kernelinterna |

### Die arch-Grenze ist echt, nicht behauptet

Die Grenze ist als Trait formuliert (`kernel/src/kcore/arch_iface.rs`):

* `ArchOps` — Konsole, CPU-Init, Interruptflag, Halt, Backtrace, CPU-Name;
  dazu die assoziierten Typen `AddressSpace`, `Timer`, `IntCtl`.
* `AddressSpaceOps` — `map`, `map_large`, `unmap`, `translate`, `activate`.
  Beschrieben wird mit `MapFlags` (READ/WRITE/EXEC/USER/NO_CACHE/GLOBAL) und
  `PhysAddr`/`VirtAddr` — **kein einziges x86-Bit**.
* `TimerOps`, `InterruptCtlOps` — Zeitgeber und Interruptcontroller.

`kernel/src/arch/mod.rs` ist nur eine dünne Fassade, die per `cfg(target_arch)`
die Implementierung wählt und die Traitmethoden als freie Funktionen anbietet.

**Nachprüfen** (liefert im aktuellen Stand keine Treffer):

```sh
grep -rnE '\b(cr[0-4]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq|swapgs|sysret|STAR|LSTAR|SFMASK|SMEP|SMAP)\b' \
     kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'
```

Einziger Treffer im aktuellen Stand ist ein **Kommentar** in `arch_iface.rs`,
der die Grenze erklärt; `test.sh` Schritt 15 unterscheidet ausdrücklich zwischen
Code (fällt durch) und erklärendem Kommentar (zulässig, wird gemeldet).

Eine Portierung auf aarch64 besteht deshalb aus: `arch/aarch64/` anlegen, die
vier Traits implementieren, in `arch/mod.rs` zwei `cfg`-Zeilen ergänzen. Kein
Modul außerhalb von `arch/` wird angefasst.

---

## 3. Bootweg

### Warum Limine und nicht Multiboot2

Multiboot2 übergibt die CPU im **32-Bit-Protected-Mode**. Der Kernel müsste
dann selbst einen Assembler-Vorlader mitbringen, der Zwischen-Seitentabellen
aufsetzt, nach long mode schaltet und sich in die obere Hälfte umzieht — mehrere
hundert Zeilen Assembler, die zum eigentlichen Ziel nichts beitragen und bei
jeder Änderung Triple Faults produzieren.

Limine (Basisrevision 3) liefert ab: 64-Bit-Modus, higher-half gemappter Kernel,
HHDM-Fenster, sortierte Memory-Map, gesetzter Framebuffer, BIOS **und** UEFI aus
demselben ISO. Der Preis ist eine Abhängigkeit vom Bootprotokoll — die wird
dadurch begrenzt, dass **`kernel/src/boot/limine.rs` die einzige Datei ist, die
Limine kennt**. Sie übersetzt alles sofort in kerneleigene Typen
(`osum_mem::region::MemoryRegion`, `FramebufferInfo`). Ein zweites Protokoll
wäre eine weitere Datei im selben Verzeichnis.

Wichtig: Der Kernel **bleibt nicht** auf den Tabellen des Bootloaders. Er baut in
`mm::build_kernel_space` sofort seine eigene PML4 und schaltet CR3 um.

### Reihenfolge in `kmain`

| # | Schritt | Warum genau hier |
|---|---|---|
| 1 | COM1 initialisieren | ohne Ausgabe ist jeder Fehler danach unsichtbar |
| 2 | Bootloaderdaten einsammeln, HHDM setzen | ohne HHDM ist keine physische Adresse erreichbar |
| 3 | Framebuffer-Konsole | zweiter Ausgabeweg, sobald möglich |
| 4 | GDT/TSS, IDT, Ausnahmen, PIC | ab jetzt sind Fehler diagnostizierbar statt Triple Fault |
| 5 | Frame-Allocator aus der Memory-Map | Voraussetzung für eigene Seitentabellen |
| 6 | eigener Adressraum + CR3-Umschaltung | Bootloaderspeicher wird entbehrlich, Rechte werden korrekt |
| 7 | Heap abbilden, `alloc` aktiv | ab jetzt sind `Vec`, `Box`, `String` benutzbar |
| 8 | PIT starten, Interrupts freigeben | erster periodischer Interrupt, nachweisbar im Log |
| 9 | kooperative Scheduler-Selbsttests | Kontextwechsel muss stehen, bevor er erzwungen wird |
| 10 | verdrängenden Zeitgebereinsprung zuschalten | braucht Heap (Thread-Stapel) und laufenden Zeitgeber |
| 11 | Startdateisystem lesen, ELF prüfen und laden | braucht Frames, Adressraum und Heap |
| 12 | Systemaufrufpfad einrichten, nach Ring 3 springen | braucht geladene Seiten und einen Kernelstapel im Aufgabensegment |

---

## 4. Speicherverwaltung

### Adressraumaufteilung (x86_64)

| Bereich | Adresse | Inhalt |
|---|---|---|
| PML4 256 | `0xffff_8000_0000_0000` | HHDM-Fenster (`virt = phys + offset`), 2-MiB-Seiten, RW+NX |
| PML4 510 | `0xffff_ff00_0000_0000` | Kernel-Heap |
| PML4 511 | `0xffff_ffff_8000_0000` | Kernelabbild, seitengenau mit echten Rechten |

Das Kernelabbild wird **nicht** pauschal als RWX übernommen, wie der Bootloader
es ablegt, sondern anhand der Linkersymbole in drei Bereiche aufgeteilt:
`.text` R+X, `.rodata` nur lesbar (NX), `.data`+`.bss` R+W+NX. Dazu ist
`CR0.WP` gesetzt, damit auch Ring 0 schreibgeschützte Seiten respektiert.

> Fallstrick, der hier gelöst ist: Ohne explizites `*(.got)` im Linkerskript
> legt der Linker die GOT als „Orphan"-Sektion **hinter** `__data_end` ab. Der
> Kernel bildet sie dann nicht ab und stirbt beim ersten Zugriff auf eine
> globale Adresse. Siehe `kernel/linker-x86_64.ld`.

### Frame-Allocator: Bitmap, nicht Buddy

Gewählt: **Bitmap** (`libs/osum-mem/src/bitmap.rs`).

* 1 Bit je 4-KiB-Frame = 32 KiB Metadaten pro GiB RAM, konstant, ohne Zeiger.
* Ein Buddy-Allocator braucht Freilisten — Freilisten brauchen einen Heap — der
  Heap braucht Frames. Die Bitmap durchschlägt dieses Henne-Ei-Problem, indem
  sie sich selbst in den erstbesten freien Bereich legt.
* Nachteil: große zusammenhängende Anforderungen sind O(n). Für Phase 1
  (Seitentabellen, Heap-Backing) irrelevant. Ein Buddy-Aufsatz *über* der
  Bitmap ist in der ROADMAP vorgesehen, wenn DMA-Puffer nötig werden.

Die Bitmap deckt nur `ram_top` ab — die höchste Adresse **echten** Speichers.
Nähme man stattdessen die höchste Adresse überhaupt, kostete ein PCI-Loch bei
1 TiB eine 32-MiB-Bitmap für Speicher, den es gar nicht gibt (real gemessen:
32768 KiB → 16 KiB nach der Korrektur).

### Heap: eigener Free-List-Allocator, keine externe Crate

`libs/osum-mem/src/heap.rs`, ca. 150 Zeilen: First-Fit über eine
adresssortierte Freiliste mit sofortiger Verschmelzung benachbarter Löcher.
Jede Belegung trägt einen 16-Byte-Kopf mit tatsächlicher Blockgröße und dem
Abstand zum Blockanfang; dadurch ist `dealloc` auch bei übergroßen
Ausrichtungen korrekt, ohne sich auf das übergebene `Layout` zu verlassen.

Der Grund, das selbst zu schreiben statt `linked_list_allocator` einzubinden:
Kernziel *lightweight*, und weil die Logik hardwarefrei ist, läuft sie in sechs
Host-Tests (Ausrichtung bis 4096, Überlappungsfreiheit, Erschöpfung,
Wiederherstellung nach Freigabe, Nachbarschaftsschutz) — **bevor** sie im
Kernel jemals Speicher vergibt.

---

## 5. CPU-Grundlagen

* **GDT**: null, Kernel-Code (`0x08`), Kernel-Daten (`0x10`), User-Daten
  (`0x18`), User-Code (`0x20`), TSS (`0x28`). Die Reihenfolge Userdaten vor
  Usercode ist bereits auf `syscall`/`sysret` ausgelegt.
* **TSS** mit drei IST-Stapeln zu je 20 KiB: `#DF` (IST 1), `#PF` (IST 2),
  `NMI` (IST 3). Ein Double Fault landet damit garantiert auf einem gültigen
  Stapel und kann noch berichten, statt in einen Triple Fault zu laufen.
* **IDT**: 256 Tore, Ausnahmen 0–21 belegt, IRQ 0–15 auf `0x20`–`0x2F`.
  `#BP` ist ein Trap-Tor mit DPL 3 und **kehrt zurück** — `int3` ist ein
  Werkzeug, kein Absturz.
* **Ausnahmeausgabe**: Name, RIP, CS, RSP, SS, RFLAGS, Fehlercode; bei `#PF`
  zusätzlich CR2 und die Ursache im Klartext („Seite nicht präsent,
  Schreibzugriff, ausgelöst im Kernel").
* **Interruptcontroller**: 8259-PIC, umgeleitet auf `0x20`. Kein APIC in Phase 1,
  weil der APIC ACPI-Tabellen, MMIO-Abbildung und Timer-Kalibrierung braucht —
  der PIC funktioniert sofort und liefert die früheste nachweisbare
  Interrupt-Zustellung. Der Umstieg tauscht genau eine Datei aus.
* **Zeitgeber**: 8253-PIT, 100 Hz, Ticks als `AtomicU64`.

### Der Double-Fault-Test ist echt

`--test-doublefault` simuliert **nicht** per `int 8`. Er markiert das
`#PF`-Tor als *nicht präsent* und löst dann einen Page Fault aus. Die dabei
entstehende `#NP` während der Zustellung einer Ausnahme ist laut Intel SDM
Vol. 3, Tabelle 6-5 genau die Bedingung für `#DF`. Der Handler auf IST 1
berichtet anschließend und hält an.

### Kontextwechsel und kooperativer Scheduler (Phase 2)

Die Arbeit ist genau zweigeteilt:

* **`kernel/src/arch/x86_64/context.rs`** ist die einzige Datei, die weiß,
  *welche* Register beim Wechsel zu retten sind. `switch_stacks` ist ein
  `naked_asm!`-Rumpf: callee-saved Register (`rbx`, `rbp`, `r12`–`r15`) und
  `RFLAGS` auf den eigenen Stapel, Stapelzeiger nach `*from` sichern, den aus
  `*to` laden, zurückholen, `ret`. Ein gesicherter Kontext ist deshalb genau
  **ein Wort** (der Stapelzeiger). Ein frischer Stapel wird so vorbelegt, dass
  das `ret` beim ersten Einlasten in den Einsprung springt; darunter liegt eine
  Landeadresse, die per `panic!` meldet, falls ein Rumpf trotz `-> !` zurückkehrt.
* **`kernel/src/kcore/sched.rs`** entscheidet nur, *wer* läuft: `Scheduler`-Trait
  (`spawn`, `yield_now`, `block_current`, `unblock`, `exit`, `run`) und die
  Implementierung `CoopScheduler` — Round-Robin, **kooperativ**, ohne Sperren.
  In dieser Datei kommt kein Registername vor; sie kennt nur `ContextOps` und
  `TimerOps`.

Jeder Thread bekommt einen eigenen Kernelstapel aus dem Heap, gefüllt mit `0xa5`.
Daraus folgen zwei prüfbare Aussagen: die untersten 64 Byte müssen nach dem Lauf
noch das Muster tragen (Wachzone), und die tiefste berührte Stelle ist die
gemessene Stapelnutzung (`peak_stack_bytes`, im Boot-Log gemeldet).

`sleep_ticks(n)` blockiert mit einer Weckzeit auf dem Tickzähler des Zeitgebers;
der Scheduler weckt Fällige, bevor er wählt, und wartet ansonsten mit
`wait_for_interrupt`. Blockieren alle übrigen Threads **ohne** Weckzeit, kehrt
`run` mit gesetzter Verklemmungsmeldung zurück, statt hängen zu bleiben.

Drei Selbsttests laufen bei jedem Boot und stehen im seriellen Log:
Wechselspiel zweier Threads (`Threadwechsel: … Spur 121212`), Schlafen/Wecken
(`Schlafen/Wecken: … Zusagen`) und der Verklemmungsschutz
(`Verklemmungsschutz: run() kehrte nach 2 Wechseln zurueck …`). Danach läuft
`kmain` normal weiter — der Scheduler ist noch **nicht** dauerhaft aktiv.

### 5a. Präemption: derselbe Planer, erzwungene Wechsel (Phase 3)

Es gibt **keinen zweiten Scheduler**. Derselbe `Scheduler`-Trait bekam drei
Methoden dazu — `spawn_prio`, `charge_tick`, `preempt_current` —, und ein Thread
trägt jetzt `prio`, `left` (Restscheibe) und `ticks` (verbrauchte Ticks).

| Ort | Aufgabe |
|---|---|
| `arch/x86_64/preempt.rs` | IRQ0-Einsprung `irq0_entry` als `naked_asm!`-Rumpf: **alle 15 GPR** plus den von der CPU gelegten Rahmen (`FullFrame`), Verteiler `irq0_dispatch`, Rückkehr per Interrupt-Rücksprung. Ein-/Ausschalten durch Austausch **eines** IDT-Tors |
| `kcore/preempt.rs` | `on_tick` bucht den Tick über `sched::charge_tick` auf den laufenden Thread und zählt erzwungene und freiwillige Wechsel **getrennt**; `switch_now()` führt den Wechsel aus |
| `kcore/sched.rs` | `quantum_for(prio)` = Grundscheibe × Prioritätsstufe; Auswahl bleibt Round-Robin; `tick_shares()` liefert die Verteilung für das Log |

Zwei Regeln hält der Pfad strikt ein: der Interrupt wird **vor** dem Wechsel
quittiert (sonst bliebe der nächste Tick aus, während der verdrängte Thread
schläft), und gewechselt wird nur, wenn privilegierter Code unterbrochen wurde —
ein Tick, der ein Ring-3-Programm trifft, wird nur gezählt.

Nachweis (`./run-qemu.sh --test-preempt`, im Boot-Log wörtlich): drei Threads,
die **nichts** tun außer zu zählen und nie `yield` rufen, teilen sich 60 Ticks
im Verhältnis 30 : 20 : 10 bei den Prioritäten 3 : 2 : 1, bei
`praeemptive Wechsel: 30, kooperative Wechsel: 3`.

### 5b. Ring 3: der Privilegwechsel (Phase 3)

Auch hier ist die Arbeit zweigeteilt, und die Grenze ist die interessante Stelle:

* **`arch/x86_64/user.rs`** — und nur dort: Freischaltung des schnellen
  Systemaufrufpfads (die drei Modellregister und das SCE-Bit), Per-CPU-Block
  samt Basisregisterwechsel, getrennte Kernel-/User-Stapel, der Ring-0-Eintrag
  im Aufgabensegment (wird beim Threadwechsel nachgezogen), der Einsprung nach
  Ring 3 und der Rücksprung aus einem Systemaufruf, der Notausstieg bei einer
  Ausnahme im Programm — und die Schutzbits SMEP/SMAP, **sofern CPUID sie
  meldet**. Auf der QEMU-Standard-CPU meldet CPUID sie nicht; das Log sagt das
  ehrlich: `Ausfuehrsperre nein (uebersprungen), Zugriffssperre nein (uebersprungen)`.
* **`kcore/user.rs`** — entscheidet, *ob* und *was* unprivilegiert startet, legt
  die Seiten an (User-Seiten **mit** Benutzerbit, Kernelseiten ohne), nimmt die
  Systemaufrufe entgegen, prüft jeden Zeiger aus Ring 3 gegen den eigenen
  Bereich (`USER_BASE`…`USER_LIMIT`) und räumt den Prozess ab.

Der Nachweis kommt **aus der Hardware**, nicht aus einer Behauptung: das
Programm löst ein `int3` aus, und der Ausnahmerahmen liefert Codesegment und
Privilegstufe. Im Log steht `CS=0x0023 (RPL=3), CPL=3` und ein Stapel im
unprivilegierten Bereich. Der Negativtest greift von Ring 3 auf
`0xffffffff80000000` zu; das Ergebnis ist ein sauber gemeldeter `#PF`
(Schutzverletzung, Userspace), der Prozess wird abgeräumt, der Kernel läuft
weiter.

### 5b'. Verdrängung bis nach Ring 3 — und warum der Stapel darüber entscheidet

Ein Programm in Ring 3 zu **unterbrechen** ist einfach: der Zeitgeber feuert,
die CPU wechselt nach Ring 0. Es später **fortzusetzen** ist die eigentliche
Aufgabe — und sie hängt an genau einer Frage: *auf welchem Stapel liegt der
Rahmen, den die CPU beim Privilegwechsel ablegt?*

Die CPU nimmt dafür `TSS.rsp0`. Zeigt der auf einen **gemeinsamen** Stapel
(so läuft der Systemaufrufpfad), darf der Zeitgeberpfad dort nicht einfach den
Thread wechseln: der nächste Privilegwechsel eines anderen Kontrollflusses
schriebe genau über diesen Rahmen. Das Programm käme in einen Zustand zurück,
den es nie hatte — ein Fehler, der nicht an der Fehlerstelle knallt, sondern
irgendwann später mit einem falschen Register.

Deshalb gibt es zwei Betriebsarten, hart getrennt durch einen Schalter
(`user::set_preemptible`):

| | Rahmen liegt auf | Tick aus CPL 3 bewirkt |
|---|---|---|
| Programm läuft **aus dem Bootweg** | gemeinsamer Systemaufrufstapel | Wachhund: Zeitbudget, dann abräumen |
| Programm läuft **in einem Thread** (`spawn_prio`) | Kernelstapel **dieses** Threads | echter Kontextwechsel, später `iretq` zurück ins Programm |

Im zweiten Fall setzt `user::enter` `TSS.rsp0` auf den eigenen Stapelzeiger
minus `TRAP_GAP` (512 B Abstand zu dem, was der Einsprung selbst noch ablegt)
und stellt den alten Wert danach wieder her. Der Rahmen liegt damit auf dem
Stapel des Threads, überlebt jeden Wechsel und wird erst beim `iretq` am Ende
des Interrupt-Einsprungs wieder abgebaut.

Nachgewiesen wird das in **jedem** Boot, nicht in einem Sondermodus: zwei
gleichrangige Threads, einer trägt ein Rechenprogramm in Ring 3, das
30 Mio. Durchläufe lang keinen einzigen Systemaufruf absetzt, der andere zählt.
Keiner gibt freiwillig ab. Ergebnis im Log:

```text
[osum] user  Ring 3 verdraengt: 12 Unterbrechung(en) durch den Zeitgeber,
              danach fortgesetzt, CS=0x0023 (RPL=3), Ende 0
[osum] user  Ring 3 verdraengt: Zaehlthread kam auf 12040431 Durchlaeufe,
              26 erzwungene und 2 freiwillige Wechsel, Ticks 13/13, 4/4 Zusagen
```

`Ende 0` ist der entscheidende Wert: das Programm hat sich **selbst regulär
beendet**, nachdem es zwölfmal unterbrochen wurde. Ein abgeräumtes Programm
hätte einen anderen Code.

Grenze, die bewusst so bleibt: es läuft immer nur **ein** unprivilegiertes
Programm zur Zeit — `SAVED_RSP`, der Zustand „läuft unprivilegiert" und
`TSS.rsp0` sind je CPU geführt, nicht je Thread. Das ist der nächste Schritt
(ROADMAP, Punkt 2), kein Versehen.

### 5c. Startdateisystem und ELF64-Lader (Phase 3)

`limine.conf` reicht `build/initramfs.img` als Bootloader-Modul herein.
Das Archivformat ist **eigenbau** (`IRFS0002`): 16-Byte-Kopf, danach eine
Tabelle fester Breite (56 B je Eintrag: Name 32 B, Offset, Länge, CRC32 der
Daten, 4 B reserviert). Die Prüfsumme wird beim Öffnen **nach** der
Bereichsprüfung und **vor** jedem Datenzugriff geprüft. Begründung gegen cpio/tar
steht im Modulkopf von `kcore/initramfs.rs`: beide transportieren POSIX-Metadaten
(Modus, uid/gid, Pfade), die dieser Kern nicht kennt und sofort wegwerfen
müsste — Code, der nur Angriffsfläche ist; außerdem sind beide stromorientiert,
während hier jede Datei in O(1) auffindbar ist.

Der Lader in `kcore/elf.rs` prüft **vor** dem ersten Zugriff: Kennung, Klasse,
Bytereihenfolge, Typ, Zielarchitektur, Lage der Programmkopftabelle, jedes
Segment gegen Dateilänge, gegen den unprivilegierten Adressbereich, gegen
Überlappung und gegen sich selbst (`filesz > memsz`), zuletzt die
Einsprungadresse. Alle Additionen sind `checked_*`. Rechte kommen aus `p_flags`
(RX / R / RW+NX), `.bss` wird genullt, und schlägt ein Segment mittendrin fehl,
werden die schon vergebenen Frames **zurückgerollt**.

Nachweis: 11 Negativfälle für ELF, 7 für das Archiv, dazu vier echte Ladefälle
(gutes Abbild, absichtlich kaputtes Abbild, Textdatei als Programm, zu großes
Abbild). Das Programm aus dem Archiv (`userland/hello.asm`, `nasm` + `ld`,
statisch, ohne Laufzeitbibliothek) läuft anschließend wirklich in Ring 3.

---

## 6. Panic und Backtrace

Der Panic-Handler sperrt Interrupts, schützt sich per Flag gegen Doppel-Panics,
gibt Ort und Grund aus und läuft dann die **Frame-Pointer-Kette** ab
(`-C force-frame-pointers=yes` in `.cargo/config.toml`; `[rbp]` = voriger RBP,
`[rbp+8]` = Rücksprungadresse). Ungültige oder absteigende Ketten brechen die
Ausgabe ab, statt einen Fehler im Fehlerpfad zu erzeugen.

Adressen werden bewusst **nicht** im Kernel aufgelöst — ein Symbolauflöser im
Kernel wäre Ballast. `build.sh` legt `build/osum.map` ab; damit oder mit
`llvm-addr2line -e target/.../osum <adresse>` wird die Zeile bestimmt.

---

## 7. Die zwei ABIs

### `osum-native` (dauerhaft)

Definiert in `libs/osum-abi-native/`, bewusst ohne POSIX-Altlasten:

| POSIX | osum-native | Grund |
|---|---|---|
| `int fd`, implizites Erben | `Handle` (32 Bit Slot + 32 Bit Generation) | Generation verhindert Use-after-close-Verwechslung |
| Zugriff über globalen Pfad | Handle mit `Rights` | Capability statt Umgebungsautorität |
| `errno` (thread-lokal) | `Error` als Rückgabewert | keine versteckten Globals |
| `fork` + `exec` | `ProcessSpawn` mit expliziter Handle-Liste | kein Adressraumkopieren, kein implizites Erben |
| Signale | `Port` + `PortWait` | Nachrichten statt Stack-Hijacking |
| `open("/pfad")` | `NamespaceOpen(handle, name)` | kein prozessglobaler Root |

23 Syscall-Nummern sind festgelegt und stabil; Rechte können beim Weitergeben
nur **verkleinert** werden (`Rights::restrict`). Nicht Gebautes liefert sauber
`NotSupported` — **kein `todo!()`**, der Kernel bleibt in jedem Zustand
lauffähig.

#### Handle-Tabelle je Prozess

`libs/osum-abi-native/src/table.rs` (host-getestet) hält die Regeln, der Kernel
nur die Verbindung zu echten Objekten:

* Ein Handle ist **Slot + Generation**, dazu ein **prozesseigener Würfelwert**
  im Generationsfeld. Derselbe Slot ergibt in zwei Prozessen also verschiedene
  Handle-Werte — ein abgehörter Wert aus einem fremden Prozess trifft hier nie.
* Die Generation lebt weiter, auch wenn der Platz frei ist: ein geschlossenes
  Handle trifft nach Wiederverwendung des Slots **niemals** das neue Objekt.
* **Kein Erben.** Eine frische Tabelle ist leer; `spawn`/`spawn_with` übergeben
  Handles namentlich, Rechte dabei nur kleiner. Es gibt kein `fork`.
* Ein Handle trägt eine **Objektnummer**, nie einen Zeiger. Nachgeschlagen wird
  in der Objekttafel (`abi/native.rs`) mit Verweiszähler; Objektarten sind
  Console, Channel, Process, Memory.
* Ein Platzlimit je Prozess (1024) verhindert, dass ein Prozess den
  Kernelspeicher durch Handle-Erzeugung erschöpft.

Der Verteiler prüft bei jedem Aufruf aus Ring 3 zusätzlich, dass alle Puffer im
eigenen Adressbereich liegen. **Negativtests im Boot-Log** (Pflicht, nicht Kür):
ungültiger Index, veraltete Generation und fehlendes Recht werden je einzeln
abgewiesen; dazu fremdes Handle, Zugriff ohne Übergabe, versuchte
Rechteausweitung, Duplizieren/Übergeben ohne Recht, unbekannte Aufrufnummer,
Benutzung nach `close`, Handle nach Prozessende — und 4096 geratene Generationen
mit **0** Treffern.

### `posix` (abtrennbar)

`libs/osum-abi-posix/` hängt **ausschließlich** von `osum-abi-native` ab und
kennt keine Kernelinterna. `errno`, `fd` und die Regel „kleinste freie Zahl"
existieren nur dort. `fork` ist dauerhaft `-ENOSYS`; portierte Programme müssen
`posix_spawn` benutzen.

**So verschwindet POSIX ersatzlos:**

1. `libs/osum-abi-posix/` löschen,
2. in `kernel/Cargo.toml` das Feature `posix` und die optionale Dependency streichen,
3. `kernel/src/abi/posix.rs` löschen und die zwei `#[cfg(feature = "posix")]`-Zeilen
   in `kernel/src/abi/mod.rs` streichen.

Danach existiert im Baum kein `errno`, kein `fd`, kein `open`. **Gegenprobe, die
im Testlauf wirklich läuft:** `./run-qemu.sh --check --no-posix` baut mit
`--no-default-features`, bootet und gibt aus:
`posix-Schicht NICHT einkompiliert — Core läuft ohne sie`.

---

## 8. Abhängigkeiten — jede einzeln begründet

```
osum
├── osum-abi-native   (eigen)
├── osum-abi-posix    (eigen, optional)
├── osum-mem          (eigen)
├── limine 0.5  ── bitflags 2
└── spin 0.9
```

| Crate | Warum | Warum nicht selbst |
|---|---|---|
| `limine` | reine `#[repr(C)]`-Strukturen des Bootprotokolls, kein Laufzeitcode | Abschreiben der Strukturen wäre stille Fehlerquelle bei Protokollrevisionen |
| `spin` | `Mutex`/`Once` ohne `std`, `default-features = false` | könnte man schreiben, aber Sperren sind der falsche Ort für Eigenbau |
| `bitflags` | transitiv über `limine`, nur Makros | — |

Bewusst **nicht** benutzt: `x86_64` (die Register-/Tabellenzugriffe stehen
selbst im Baum, ca. 400 Zeilen, dafür ohne Fremdabstraktion in der arch-Schicht),
`linked_list_allocator` (eigener Heap), `bootloader`, `uart_16550`, `pic8259`,
`log`.

Geladene Größe des Kernels (Release, gemessen mit
`size -A target/x86_64-osum-none/release/osum`):

| Sektion | Größe |
|---|---|
| `.text` | 152,9 KiB |
| `.rodata` | 31,1 KiB |
| `.limine_requests` | 0,4 KiB |
| `.data` | 6,6 KiB |
| `.bss` | 80,7 KiB |
| **Summe im Speicher** | **271,7 KiB** (auf Seiten gerundet 276 KiB = 69 Seiten) |

Der größte Einzelposten in `.bss` sind die drei IST-Stapel zu je 20 KiB. Die
ELF-Datei auf der Platte ist deutlich größer, weil Debug-Informationen für
`addr2line` mitgeliefert werden; geladen wird davon nichts.

> Diese Zahlen werden bei jeder Änderung nachgemessen, nicht geschätzt. Wer sie
> anfasst, führt vorher `size -A` aus.

---

## 9. Nightly-Features

| Feature | Wo | Warum zwingend |
|---|---|---|
| `abi_x86_interrupt` | `kernel/src/main.rs` | einzige Möglichkeit, Ausnahmehandler ohne handgeschriebene Assembler-Stubs korrekt aufzurufen (Fehlercode, `iretq`) |
| `build-std` | `build.sh` | `core`/`alloc` müssen für das eigene Target neu gebaut werden |
| `alloc_error_handler` | `kernel/src/main.rs` | ohne das meldet erschöpfter Heap nur ein nacktes Panic; mit ihm gibt der Kernel Heap-Statistik und Anforderungsgröße aus |
| `json-target-spec` | `.cargo/config.toml` | rustc 1.99 verlangt das Flag für `.json`-Target-Specs |

`#[unsafe(naked)]` + `naked_asm!` für den Kontextwechsel
(`arch/x86_64/context.rs`) brauchen **kein** Feature mehr — seit Rust 1.88
stabil. Alles andere ist stabiler Rust. `build-std` steht bewusst **nicht** in
`.cargo/config.toml`, weil `[unstable]` für jedes Ziel gälte und `cargo test`
auf dem Host dann an einem zweiten `core` scheitert
(`duplicate lang item: sized`).

---

## 9b. Warnungen sind Fehler — und der Nachweis muss echt sein

Zwei Dinge, die getrennt gehören:

1. **Der Bau selbst hält dagegen.** `Cargo.toml` setzt
   `[workspace.lints.rust] warnings = "deny"`, jede eigene Crate übernimmt das
   mit `[lints] workspace = true`. Eine Warnung bricht damit den Bau ab, statt
   nur im Log zu stehen. Bewusst über `[lints]` und nicht über `RUSTFLAGS`:
   `RUSTFLAGS` würde auch das mitgebaute `core`/`compiler_builtins` treffen,
   die uns nicht gehören.
2. **Der Nachweis darf kein No-op sein.** Ein Folgebau schreibt nur
   `Finished ... in 0.09s` — in so einem Log nach `^warning:` zu suchen ist
   wertlos. `./build.sh --fresh` fasst deshalb die eigenen Quellen an, erzwingt
   eine echte Übersetzung und legt das Ergebnis als `build/cargo-build-fresh.log`
   ab; die letzte Zeile jedes Bau-Logs sagt ausdrücklich, ob übersetzt wurde
   (`BUILD-EVIDENZ: echte Uebersetzung (4 Crate(s))` bzw. `No-op`). `test.sh`
   baut mit `--fresh` und lässt den Schritt fallen, wenn dieses Log fehlt oder
   keine `Compiling`-Zeile enthält.

Gegenprobe (einmal von Hand gefahren, damit die Mechanik nicht nur behauptet
ist): eine absichtlich unbenutzte Variable in `libs/osum-mem` führte zu
`error: unused variable: 'ungenutzt'` und einem abgebrochenen Bau.

## 9a. Der Name ist Konfiguration, kein Code

Im Quelltext steht **kein Produktname**. Kernel- und OS-Name kommen aus
Cargo-Metadaten, werden von `kernel/build.rs` als `env!`-Variablen eingespeist
und ausschließlich in `kernel/src/kcore/branding.rs` sichtbar:

```
kernel/Cargo.toml  name = "osum"                     ─┐
                   [package.metadata.branding]         │ build.rs
                   os-name = "OrientOS"                ─┘   ↓
kcore/branding.rs  KERNEL_NAME · OS_NAME · VERSION · LOG_TAG · NATIVE_ABI · banner()
                                    ↓
             klog!(), Panic-Handler, Boot-Banner, ABI-Beschreibung
```

`./test.sh` Schritt 14 lässt den Build durchfallen, sobald jemand einen
Produktnamen als Literal in `kernel/src` schreibt. Vollständiges Umbenennen
(Crates, Verzeichnisse, Target-JSON, Doku) macht `./rename.sh <kernel> <os>`;
verifiziert mit `./rename.sh nova Novaos` in einer Kopie unter `/tmp`, Ergebnis
gebootet. Anleitung: [RENAME.md](RENAME.md).

Grund: Justin will sich den endgültigen Namen offenhalten. Ein Name, der über
200 Dateien verteilt ist, ist eine Einbahnstraße.

---

## 10. Was noch nicht da ist

Ehrliche Liste, damit niemand mehr erwartet, als der Code hält:

* **Kein dauerhaft laufender Userspace.** Ring 3 wird echt betreten, aber als
  Selbsttest im Bootweg: ein Programm läuft, gibt aus, endet, wird abgeräumt.
  Danach läuft `kmain` weiter. Es gibt kein `init`, das die Kontrolle behält.
* **Ein Adressraum.** Die unprivilegierten Seiten liegen im Kerneladressraum
  (mit Benutzerbit); es gibt noch **keinen Adressraum je Prozess** und damit
  keine Isolation zwischen zwei unprivilegierten Prozessen.
* **Präemption nur für Threads aus `spawn_prio`.** Kooperative Threads bleiben
  absichtlich unverdrängt. Ein Ring-3-Programm **wird** verdrängt und
  fortgesetzt, sobald es in einem solchen Thread läuft (Rahmen auf dessen
  Kernelstapel); läuft es dagegen direkt aus dem Bootweg, greift statt des
  Wechsels der Wachhund.
* **Nur ein unprivilegiertes Programm zur Zeit.** `SAVED_RSP`, „läuft
  unprivilegiert" und `TSS.rsp0` sind je CPU geführt, nicht je Thread. Zwei
  Ring-3-Programme gleichzeitig würden sich ins Gehege kommen — der Kernel
  startet deshalb bewusst nur eines.
* kein Idle-Thread; der Scheduler wartet mit `wait_for_interrupt`
* SMEP/SMAP werden nur gesetzt, wenn CPUID sie meldet. `run-qemu.sh` startet
  deshalb standardmäßig mit `-cpu max` (dieses Modell meldet sie), sonst wäre
  die CR4-Logik in jedem Testlauf toter Code. Der Überspringen-Pfad wird eigens
  mit `--cpu-basic` geprüft (Schritt „Ring 3")
* die POSIX-Schicht übersetzt Fd → Handle durch **denselben** Verteiler, hat
  aber noch keinen `open`-Weg — ohne VFS gibt es nichts zu öffnen
* kein VFS, keine Blockgeräte, keine Tastatur (IRQ 1 wird nur quittiert)
* kein SMP — genau eine CPU; die serielle Ausgabe ist deshalb ungesperrt
* kein Rückgewinnen des Bootloader-Speichers (bleibt reserviert)
* kein APIC, kein ACPI-Parser, keine PCI-Aufzählung
* von den 23 Syscall-Nummern sind gebaut: `Version`, `ThreadYield`,
  `ThreadSleep`, `ClockRead`, `HandleClose/Duplicate/Inspect/Write/Read`,
  `ChannelCreate/Send/Recv`, `MemoryCreate`, `ProcessExit`,
  `NamespaceCreate/Open`, `PortCreate/Bind/Wait`. Nur noch
  `MemoryMap/Unmap`, `ProcessSpawn` und `ThreadCreate` melden `NotSupported`
  (Prozesserzeugung geht bislang nur kernelseitig über `spawn`/`spawn_with`)

Der Weg dahin steht in [ROADMAP.md](ROADMAP.md).
