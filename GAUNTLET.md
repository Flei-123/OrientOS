# Gauntlet-Log — karstos
**Ziel:** WICHTIG ZUERST LESEN: PREFLIGHT.md im Projektwurzelverzeichnis. Das Projekt ist BEREITS GEBAUT, kompiliert und bootet nachweislich in QEMU (BIOS und UEFI). `./test.sh` laeuft komplett gruen durch (9 Schritte, 24 Host-Tests, 6 echte QEMU-Boots). NICHT neu anfangen, NICHTS wegwerfen, keine Datei loeschen die schon funktioniert.

AUFGABE DIESES LAUFS: den bestehenden Kernel "karst" (Betriebssystem "Karstos", Rust no_std, Ziel x86_64-karst-none) HAERTEN und gezielt erweitern, ohne den funktionierenden Boot zu zerstoeren.

Vorgehen fuer jeden Builder:
1. `cat PREFLIGHT.md ARCHITECTURE.md ROADMAP.md README.md` und den Code lesen.
2. `./test.sh` EINMAL ausfuehren, damit der gruene Ausgangszustand belegt ist.
3. Erst dann aendern — in kleinen Schritten, nach jeder Aenderung `./build.sh && ./run-qemu.sh --check`.
4. Vor der Abgabe zwingend `./test.sh` — muss gruen sein.

Konkrete Verbesserungsziele, nach Wichtigkeit:
A) ROBUSTHEIT DES BESTEHENDEN: Fehlerpfade pruefen und haerten. Frame-Allocator: Verhalten bei erschoepftem Speicher, bei zerrissener Memory-Map, bei fehlendem HHDM. Seitentabellen: map/unmap/translate gegen Randfaelle (nicht ausgerichtete Adressen, doppeltes Mapping, Huge-Page im Weg) — die Fehlerfaelle von MapError muessen im Kernel wirklich erreichbar und getestet sein, nicht nur definiert. Heap: Verhalten bei OOM (derzeit panickt der Standard-Handler — pruefen, ob eine lesbare Meldung mit Heap-Statistik besser ist).
B) MEHR HOST-TESTS fuer alles, was ohne Hardware auskommt (libs/karst-mem, libs/karst-abi-native, libs/karst-abi-posix). Ziel: deutlich mehr als die aktuellen 24 Tests, inklusive Grenzfaellen und Eigenschaftstests (z.B. zufaellige alloc/free-Folgen gegen ein Referenzmodell). Nur std im #[cfg(test)].
C) BOOT-DIAGNOSE: die Ausgabe des Startvorgangs weiter verdichten und pruefbar machen; run-qemu.sh --check um weitere Merkmale erweitern (z.B. dass die Rechte des Kernelabbilds wirklich gelten: Schreibversuch auf .rodata muss #PF ausloesen — als eigener Selbsttest --test-rodata analog zu --test-pagefault).
D) PHASE 2 ANFANGEN (nur wenn A-C sitzen und der Boot gruen bleibt): Kontextwechsel in `kernel/src/arch/x86_64/context.rs` (naked_asm), ein `Thread` mit eigenem Kernelstapel, ein minimaler kooperativer Scheduler hinter einem Trait in kcore, und ein Selbsttest der zwei Kernel-Threads abwechselnd ins Boot-Log schreiben laesst und danach sauber zurueck in kmain kommt. Der Scheduler-Trait gehoert nach kcore, die Registerrettung nach arch. Wenn das nicht stabil zum Laufen kommt: LIEBER WEGLASSEN als einen kaputten Kernel abgeben.
E) DOKU NACHZIEHEN: jede Aenderung in ARCHITECTURE.md/ROADMAP.md/README.md nachfuehren. Nichts behaupten, was nicht im Code steht. Kennzahlen (Zeilen, Crates, Testanzahl) aktualisieren statt raten.

Nicht erwuenscht: neue externe Crates ohne zwingenden Grund, kosmetische Umbenennungen, Umformatierung des ganzen Baums, Ersetzen des eigenen Heap-/Bitmap-Allocators durch Fremd-Crates, Aenderungen an vendor/limine/.
**Messlatte:** Gemessen wird an einem BEREITS FUNKTIONIERENDEN Kernel — Verschlechterung ist der schlimmste Fehler.

(0) K.-o.-Kriterium: `./test.sh` muss am Ende vollstaendig gruen sein (9 Schritte, darunter Boot ueber BIOS und UEFI, Boot ohne POSIX-Schicht, Selbsttests fuer Page Fault, echten Double Fault und Panic). Ist der Kernel am Ende nicht mehr bootfaehig oder ein Schritt rot: hoechstens 25 Punkte, egal wie gut der Rest aussieht. Die Jury MUSS `./test.sh` selbst ausfuehren und das Ergebnis zitieren; ein blosses "sollte laufen" zaehlt nicht.

(a) Bootet es wirklich? Serielles Log muss Memory-Map, Frame-Statistik, eigene CR3-Umschaltung, Heap-Init mit sichtbarer Testallokation und mindestens 5 Timer-Ticks zeigen.
(b) Sind Ausnahmen nachweisbar? #BP kehrt zurueck, #PF meldet CR2 und Ursache im Klartext, #DF ist ein ECHTER Double Fault auf eigenem IST-Stapel (keine Simulation per `int 8`), Panic-Handler mit Backtrace.
(c) Funktioniert der Heap? Testallokation (Box/Vec/String) im Boot-Log, Bytes belegt/frei vor und nach Freigabe.
(d) Ist die arch-Abstraktion echt? `grep -rnE '\b(cr[0-3]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq)\b' kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'` muss LEER sein. Traits in kcore/arch_iface.rs, nicht nur freie Funktionen mit x86-Semantik. Jedes Leck kostet drastisch.
(e) Ist POSIX wirklich abtrennbar? `./run-qemu.sh --check --no-posix` muss bauen, booten und "posix-Schicht NICHT einkompiliert" melden.
(f) Lightweight? Externe Crates zaehlen (aktuell 2: limine, spin). Jede neue Crate braucht eine Begruendung in ARCHITECTURE.md; unbegruendete Crates kosten Punkte. Geladene Kernelgroesse und Zeilenzahl im Blick behalten.
(g) Doku: ARCHITECTURE.md und ROADMAP.md muessen den TATSAECHLICHEN Stand beschreiben. Jede Behauptung ohne Code dahinter ist massiver Punktabzug — die Jury soll stichprobenartig Behauptungen im README gegen den Quelltext pruefen.
(h) Fortschritt gegenueber dem Ausgangszustand: mehr belastbare Tests, gehaertete Fehlerpfade, zusaetzliche nachweisbare Selbsttests. Reine Umformatierung oder Kommentar-Kosmetik zaehlt NICHT als Fortschritt.
**Bester Score:** 80/100 (Runde 3)
**Agenten:** 22 · **Dauer:** 13539s
**Runden-Snapshots:** je Runde ein Git-Commit + Tag (gauntlet-r<N>-score<S>). Bester Stand: Branch `gauntlet-best` (Runde 3) — Wechsel mit `git checkout gauntlet-best`, zurueck mit `git checkout -`.
**Runde 0 — Architektur**: 4 Module (teil-1, teil-2, teil-3, teil-4)
**Runde 1** — Score n/a/100 (Ziel 85)
  ⚠ 4 Builder fehlgeschlagen
**Runde 2** — Score n/a/100 (Ziel 85)
  ⚠ 4 Builder fehlgeschlagen
**Runde 3** — Score 80/100 (Ziel 85)
  ⚠ 4 Builder fehlgeschlagen
  Maengel: • Kein Nachweis eines vollstaendigen Laufs der AKTUELLEN 14-Schritt-Suite: build/integrator-test.log endet in Schritt 1/14 mit drei haengenden Tests (Zeilen 152-154, 'has been running for over 60 seconds'); build/testlauf.log ist die alte 9-Schritt-Fassung von 07:33 — die 1471 Zeilen unversionierter Aenderungen (git diff --stat: bitmap.rs, heap.rs, sched.rs, mm/*) sind nur durch einen einzelnen --check-Boot (build/boot.log 11:14) abgedeckt  • ARCHITECTURE.md Zeile 317-320 behauptet '.text 32 KiB, .rodata 10 KiB, .data 6 KiB, .bss 64 KiB — rund 113 KiB' geladene Groesse; das echte Log (build/boot.log Z.14-17) sagt 76/20/72 KiB = 168 KiB. Doku widerspricht dem eigenen Boot-Log  • README.md Z.57-108 verkauft einen 'Auszug aus einem echten Boot', dessen Zahlen zu KEINEM aktuellen Lauf passen: 16 Memory-Map-Eintraege / 12 GiB / 'Startbilanz : 129483 Frames frei' / IST 0xffffffff80011990 gegen tatsaechlich 31 Eintraege / 771 MiB / 117934 Frames / IST 0xffffffff8001ea20  • ROADMAP.md Z.104-109 nennt als 'Naechster konkreter Schritt' die Umsetzung von arch/x86_64/context.rs samt Zwei-Thread-Test — genau das ist laut Z.27-34 derselben Datei bereits '✔ fertig' und im Boot-Log nachweisbar; die Roadmap beschreibt den Stand also nicht korrekt  • Der Release-Build laeuft mit ungefixten Compilerwarnungen durch (testlauf.log Z.68-79: 'direct cast of function item into an integer', kernel/src/arch/x86_64/interrupts.rs:147 ff., ein Fall je IDT-Tor); build.sh setzt kein -D warnings, test.sh Schritt 14 prueft Warnungen nicht