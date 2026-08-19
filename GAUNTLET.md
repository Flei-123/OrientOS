# Gauntlet-Log — karstos
**Ziel:** WICHTIG ZUERST LESEN: PREFLIGHT.md, ARCHITECTURE.md, ROADMAP.md, README.md, test.sh. Dieses Projekt IST BEREITS FERTIG GEBAUT, kompiliert warnungsfrei und BOOTET nachweislich in QEMU (BIOS und UEFI). `./test.sh` laeuft aktuell mit 15/15 Schritten gruen durch. NICHT neu anfangen, NICHTS wegwerfen, keine funktionierende Datei loeschen oder durch eine Fremd-Crate ersetzen. Wer den Boot kaputt macht, hat einen Totalausfall produziert, keinen Fortschritt. Gebaut wird AUF DIESEM STAND WEITER.

ZIEL DIESER RUNDE: Karstos vom blossen Boot-Kern zum System mit ECHTEM USERSPACE bringen. Vier Meilensteine:

1) PRAEEMPTIVER SCHEDULER
   - Der Timer-Interrupt (PIT 100 Hz, bereits vorhanden) ruft den Scheduler. Bisher gibt es nur kooperatives yield in kcore/sched.rs mit context.rs (naked_asm) — das bleibt als Basis, wird aber um echte Praeemption erweitert.
   - Im Interrupt-Einsprung muss der VOLLE Registersatz gesichert werden (bisher nur callee-saved): alle GPRs, korrektes Stapel-Layout, sauberes iretq. Der Registerrettungs-Code gehoert nach kernel/src/arch/x86_64/, der Scheduler-Trait und die Auswahllogik nach kcore.
   - Zeitscheiben (Quantum in Ticks) und Prioritaeten hinter DEMSELBEN Scheduler-Trait — kein zweiter paralleler Scheduler.
   - NACHWEIS im Boot-Log: mehrere Kernel-Threads, die NIE freiwillig yield() rufen (z.B. reine Zaehlschleifen), wechseln sich trotzdem messbar ab; Log zeigt Anzahl praeemptiver Wechsel, Verteilung der Ticks pro Thread und dass Prioritaeten wirken (hohe Prioritaet bekommt mehr Ticks). Als eigener Selbsttest-Schalter in run-qemu.sh, analog zu den bestehenden --test-*.

2) RING 3 / USERSPACE
   - syscall/sysret aktivieren: MSRs STAR/LSTAR/SFMASK/EFER.SCE, swapgs mit einem Per-CPU-Block (GS-Basis), getrennte Kernel-/User-Stapel, TSS.RSP0 korrekt gesetzt und beim Threadwechsel nachgezogen.
   - User-Pages: eigene Seiten mit USER-Bit, Kernel-Seiten OHNE USER-Bit (Zugriff aus Ring 3 muss #PF ausloesen — negativ testen!). SMEP/SMAP nutzen wenn CPUID sie meldet (CR4.SMEP/SMAP, stac/clac wo noetig), sonst sauber ueberspringen und das im Log vermerken.
   - Ein erstes Userspace-Programm laeuft wirklich in Ring 3, gibt per Syscall etwas aus und beendet sich sauber; der Kernel raeumt es ab und laeuft weiter.
   - NACHWEIS im Boot-Log: CS-Selektor und CPL des laufenden User-Programms (z.B. CS=0x2b, CPL=3), RSP im User-Bereich, und ein negativer Test: Ring-3-Zugriff auf eine Kernel-Adresse endet in einem sauber gemeldeten #PF statt in einem Absturz.

3) ELF64-LADER
   - Ein Initramfs (einfaches, dokumentiertes Format oder cpio/tar — Wahl begruenden) wird per Limine-Module in die ISO eingebettet und vom Kernel gefunden.
   - Statisch gelinkte ELF64-Binaries daraus laden: Programmheader parsen, PT_LOAD-Segmente mappen (Rechte aus p_flags: RX/R/RW+NX), .bss nullen, Einsprungadresse und User-Stapel aufsetzen, starten.
   - Robust gegen Muell: falsche Magic, falsche Klasse/Endianness, ueberlappende oder unplausible Segmente, Segment ausserhalb des User-Adressraums -> definierter Fehler, KEIN Kernel-Absturz. Negativ getestet.
   - Das Userspace-Programm aus (2) soll als echtes ELF aus dem Initramfs kommen, nicht als eingebettetes Byte-Array (letzteres hoechstens als Zwischenschritt).

4) osum-native ABI AUSBAUEN (capability-/handle-basiert)
   - Handle-Tabelle JE PROZESS: Handles sind unfaelschbar (Index + Generation/Nonce, kein rohes Zeiger-Casting), Rechte je Handle (Bitmaske), kein globaler Pfad-Namensraum im Core, KEIN fork — Prozesserzeugung im spawn-Stil mit EXPLIZITER Handle-Uebergabe.
   - Syscalls im osum-native-Stil: handle-orientiert, Fehlercodes als eigener Typ (kein errno), keine ambient authority.
   - NEGATIVER TEST als Pflicht: ein Prozess versucht ein Handle zu benutzen, das er nicht hat (falscher Index, veraltete Generation, fehlendes Recht) -> definierter Fehler, kein Zugriff, im Boot-Log sichtbar.
   - Die POSIX-Schicht (kernel/src/abi/posix, Feature "posix") bleibt REINER UEBERSETZER obendrauf und muss weiterhin komplett abwaehlbar sein: der Kernel muss mit --no-default-features bauen UND booten. Das ist ein eigener Testschritt in test.sh und darf NICHT verloren gehen.

RAHMENBEDINGUNGEN (nicht verhandelbar):
- Alles Architekturspezifische strikt in kernel/src/arch/x86_64/, dahinter das arch-Trait-Interface in kernel/src/kcore/arch_iface.rs. In kernel/src ausserhalb arch/ darf KEIN x86-Detail auftauchen (cr0-3, PML4, PTE, rdmsr/wrmsr, lgdt/lidt, in/out, asm!, invlpg, iretq, swapgs, syscall-MSRs). test.sh Schritt 15 prueft das per grep — er muss gruen bleiben.
- Der Produktname kommt AUSSCHLIESSLICH aus kernel/src/kcore/branding.rs (gespeist aus Cargo-Metadaten via build.rs). Kein hartkodiertes "osum"/"Karstos" in kernel/src ausserhalb branding.rs — test.sh Schritt 14 prueft das.
- LIGHTWEIGHT: derzeit nur 2 externe Crates (limine, spin). Jede neue externe Crate braucht eine harte Begruendung in ARCHITECTURE.md. Kein Ersetzen des eigenen Heap-/Bitmap-Allocators/der eigenen Seitentabellen durch Fremd-Crates (z.B. NICHT x86_64-crate, NICHT linked_list_allocator).
- KEINE todo!()/unimplemented!() im Baum. NULL Compilerwarnungen (build.sh erzwingt -D warnings).
- vendor/limine/ nicht anfassen. Keine kosmetischen Umbenennungen, keine Umformatierung des ganzen Baums.
- Nightly-Features nur wenn zwingend — und JEDER neue Fall wird in LANGUAGE.md protokolliert (Datei, Zeile, Problem, Workaround, was eine eigene Sprache anders machen muesste). Ebenso jeder neue unsafe-Block-Typ.
- Doku nachziehen: ARCHITECTURE.md (neue Schichten: sched, user, elf, handle), ROADMAP.md (erledigte Punkte abhaken, naechste ehrlich benennen), README.md (Boot-Log-Auszug muss zu einem ECHTEN aktuellen Lauf passen — Zahlen messen, nicht raten), PACKAGING.md/FILESYSTEM.md/LANGUAGE.md aktuell halten.
- test.sh waechst um Schritte fuer: praeemptive Wechsel, Ring-3-Programm, ELF-Lader inkl. Negativtests, Handle-Negativtest. Am Ende muss ./test.sh KOMPLETT gruen durchlaufen.

WENN ETWAS NICHT STABIL ZUM LAUFEN KOMMT: lieber weglassen und ehrlich in ROADMAP.md als offen fuehren, als einen kaputten oder nicht bootenden Kernel abliefern. Ein gruener Boot ist mehr wert als ein halbes Feature.
**Messlatte:** Gemessen wird an einem BEREITS FUNKTIONIERENDEN, bootenden Kernel. Verschlechterung ist der schlimmste Fehler.

(0) K.-o.-KRITERIUM: `./test.sh` muss am Ende VOLLSTAENDIG gruen sein (aktuell 15 Schritte, soll wachsen). Die Jury MUSS `./test.sh` SELBST ausfuehren und die Ausgabe zitieren; "sollte laufen" zaehlt nicht. Ist ein Schritt rot oder bootet der Kernel nicht mehr: hoechstens 25 Punkte, egal wie gut der Rest aussieht.

(a) RING 3 ECHT? Im Boot-Log muss ein wirklich in Ring 3 laufendes Programm nachweisbar sein: CS-Selektor mit RPL=3 und CPL=3 im Klartext geloggt, User-RSP im User-Adressbereich, Rueckkehr per syscall/sysret. Ein "User-Programm", das in Wahrheit in Ring 0 laeuft, ist ein Totalausfall dieses Kriteriums (0 Punkte dafuer). Negativtest Pflicht: Ring-3-Zugriff auf eine Kernel-Adresse -> sauber gemeldeter #PF, kein Absturz.

(b) PRAEEMPTION ECHT? Kontextwechsel muessen OHNE freiwilliges yield stattfinden — Threads, die nur zaehlen, wechseln sich messbar ab. Log zeigt Anzahl praeemptiver Wechsel (aus dem Timer-Handler) getrennt von kooperativen. Voller GPR-Satz gesichert (im Quelltext nachpruefbar, nicht nur behauptet). Prioritaeten muessen MESSBAR wirken (Tick-Verteilung im Log).

(c) CAPABILITIES ECHT? Handle-Tabelle je Prozess, Handles mit Generation/Nonce (nicht faelschbar durch Raten eines Index), Rechte je Handle. NEGATIVTEST muss im Boot-Log stehen: ungueltiger Index, veraltete Generation und fehlendes Recht werden je einzeln abgewiesen. Kommt ein Prozess an eine Ressource ohne passendes Handle: massiver Abzug. Kein globaler Pfad-Namensraum im Core, kein fork.

(d) POSIX ABTRENNBAR? `cargo build --no-default-features` UND ein Boot ohne POSIX muessen funktionieren, das Log meldet "posix-Schicht NICHT einkompiliert". Eigener test.sh-Schritt. Leckt POSIX-Semantik (errno, fork, inode/dentry) in kcore/, ist das ein schwerer Fehler.

(e) ARCH-GRENZE DICHT? `grep -rnE '\b(cr[0-4]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq|swapgs|sysret|STAR|LSTAR|SFMASK|SMEP|SMAP)\b' kernel/src --include='*.rs' | grep -v '^kernel/src/arch/'` muss LEER sein (erklaerende Kommentare in arch_iface.rs ausgenommen und als solche kenntlich). Auch main.rs darf nichts Architekturspezifisches direkt aufrufen. Jedes Leck kostet drastisch.

(f) NAME NUR AUS branding.rs? `grep -rniE 'osum' kernel/src --include='*.rs' | grep -v branding.rs` muss leer sein. Und `./rename.sh` muss in einer Kopie unter /tmp nachweislich funktionieren.

(g) SAUBERKEIT: keine todo!()/unimplemented!(), NULL Compilerwarnungen (Jury prueft den Build-Log), keine neuen externen Crates ohne Begruendung in ARCHITECTURE.md, kein Ersetzen eigener Kernstuecke (Heap, Bitmap-Allocator, Seitentabellen) durch Fremd-Crates.

(h) DOKU DECKT SICH MIT CODE: ARCHITECTURE.md, ROADMAP.md, README.md, PACKAGING.md, FILESYSTEM.md, LANGUAGE.md muessen den TATSAECHLICHEN Stand beschreiben. Die Jury prueft STICHPROBEN: jede Zahl im README (Kernelgroesse, Frames, Zeilen, Testanzahl) und jeder Boot-Log-Auszug muss zu einem ECHTEN aktuellen Lauf passen. Behauptungen ohne Code dahinter = massiver Punktabzug. LANGUAGE.md muss die in DIESER Runde neu aufgetretenen Rust-Reibungspunkte (naked_asm, syscall-ABI, unsafe, Allocator) enthalten.

(i) FORTSCHRITT: gemessen an praeemptiven Wechseln, echtem Ring 3, ELF-Lader mit Negativtests, Handle-Negativtests — alles im Boot-Log belegt. Reine Kommentar-Kosmetik, Umformatierung oder neue Doku ohne Code zaehlt NICHT als Fortschritt.
**Bester Score:** 72/100 (Runde 3)
**Agenten:** 22 · **Dauer:** 13967s
**Runden-Snapshots:** je Runde ein Git-Commit + Tag (gauntlet-r<N>-score<S>). Bester Stand: Branch `gauntlet-best` (Runde 3) — Wechsel mit `git checkout gauntlet-best`, zurueck mit `git checkout -`.
**Runde 0 — Architektur**: 4 Module (teil-1, teil-2, teil-3, teil-4) — bestehendes Projekt
**Runde 1** — Score n/a/100 (Ziel 88)
  ⚠ 1 Builder fehlgeschlagen: teil-2 (Claude Code process aborted by user)
**Runde 2** — Score n/a/100 (Ziel 88)
**Runde 3** — Score 72/100 (Ziel 88)
  Maengel: • K.-o.-Nachweis fehlt fuer den AKTUELLEN Baum: der einzige vollstaendige test.sh-Lauf (build/FINAL-test.log, 19/19 gruen) stammt von 15:51, danach wurden kernel/src/abi/native.rs, kcore/sched.rs, kcore/preempt.rs, arch/x86_64/user.rs, main.rs u.a. geaendert (16:33-16:52) und libs/osum-abi-native/src/name.rs neu (untracked) hinzugefuegt; `git status` ist komplett dirty, es gibt kein Log eines test.sh-Laufs gegen diesen Stand  • Warnungsfreiheit (g) ist nur scheinbar belegt: build/cargo-build.log enthaelt genau eine Zeile 'Finished `release` profile ... in 0.09s' — ein inkrementeller No-op-Build. test.sh Zeile 122-132 grept in genau dieser Datei nach '^warning:' und ist damit trivial gruen; faellt die Datei weg, wird die Pruefung sogar stillschweigend uebersprungen  • Praeemption deckt Ring 3 NICHT ab: arch/x86_64/preempt.rs:169-199 wechselt bei CS&3==3 gar nicht den Kontext, sondern zaehlt nur bzw. raeumt das Programm per Wachhund ab ('Ein Wechsel MIT spaeterer Fortsetzung ... ist noch nicht moeglich'). Unprivilegierte Programme sind also nicht verdraengbar-fortsetzbar  • SMEP/SMAP-Pfad ist in jedem Testlauf toter Code: run-qemu.sh setzt kein `-cpu ...,+smep,+smap` (kein einziges -cpu-Vorkommen), Boot-Log meldet 'Ausfuehrsperre nein (uebersprungen), Zugriffssperre nein (uebersprungen)' — die CR4-Schutzlogik in arch/x86_64/user.rs wird nie ausgefuehrt  • LANGUAGE.md deckt sich nicht mit dem Code: 'preempt.rs:158 (irq0_entry)' -> tatsaechlich Zeile 217; 'FullFrame ab Zeile 47' -> Zeile 76; 'user.rs:453 (syscall_entry)' -> Zeile 697; 'Per-CPU-Block als static mut (user.rs:91)' -> Zeile 117; 'kcore/user.rs:57 (is_user_addr)' -> Zeile 66; 'Messwerte: 413 unsafe-Vorkommen in kernel/src' -> aktuell 428