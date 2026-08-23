# Karstos — Betriebssystem von Grund auf

**Kernel: `osum` · OS: `Karstos` · Sprache: [Firn](LANGUAGE.md) — noch teils Rust (`no_std`) · Ziel: `x86_64-osum-none`**

> **Sprachwechsel im Gange.** osum wird **Firn-only**. Der Umbau läuft modulweise: die serielle Konsole und der Bitmap-Rahmenverwalter sind bereits in Firn, der Rest folgt. Stand und Begründung: [LANGUAGE.md](LANGUAGE.md).

Kein Linux-Fork, kein übernommener Fremdcode. Der Kernel bootet auf BIOS **und**
UEFI, baut seine eigenen Seitentabellen, hat einen funktionierenden Heap, prüft
sich beim Start mit 166 eingebauten Zusagen selbst, **verdrängt** Kernel-Threads
per Zeitgeber, lädt ein statisch gebundenes ELF64 aus einem Startdateisystem und
führt es **wirklich in Ring 3** aus — jeder Zugriff dort läuft über Handles mit
Rechten, nicht über Umgebungsautorität.

> Alles hier ist reproduzierbar: `./test.sh` baut, bootet und prüft in 19
> Schritten. Was noch fehlt, steht ehrlich in
> [ARCHITECTURE.md § 10](ARCHITECTURE.md) und [ROADMAP.md](ROADMAP.md).

---

## Schnellstart

```sh
# 1. Voraussetzungen (Debian/Ubuntu)
rustup toolchain install nightly && rustup default nightly
rustup component add rust-src llvm-tools
sudo apt install qemu-system-x86 xorriso mtools ovmf nasm python3

# 2. Bauen und in QEMU starten
./build.sh          # Kernel + Userland + Initramfs + bootfähiges build/karstos.iso
./run-qemu.sh       # startet, serielle Ausgabe im Terminal (Strg-C beendet)

# 3. Alles prüfen (Host-Tests + 14 echte QEMU-Boots)
./test.sh
```

### Skripte

| Befehl | Wirkung |
|---|---|
| `./build.sh` | Release-Build + Userland + `build/initramfs.img` + ISO (`build/karstos.iso`) + Symbolkarte (`build/osum.map`) |
| `./build.sh --debug` | Debug-Build |
| `./build.sh --no-posix` | Gegenprobe: Kernel ohne POSIX-Schicht (`--no-default-features`) |
| `./run-qemu.sh` | interaktiver Start, serielle Ausgabe auf stdout |
| `./run-qemu.sh --check` | bootet, prüft **38 Merkmale** im Log, **Exitcode** 0/1 (für CI) |
| `./run-qemu.sh --uefi` | Boot über OVMF statt SeaBIOS |
| `./run-qemu.sh --test-pagefault` | löst absichtlich `#PF` aus und prüft die Meldung |
| `./run-qemu.sh --test-doublefault` | löst einen **echten** `#DF` aus (nicht `int 8`) |
| `./run-qemu.sh --test-panic` | prüft Panic-Handler und Backtrace |
| `./run-qemu.sh --test-rodata` | Schreibversuch auf `.rodata` muss `#PF` auslösen |
| `./run-qemu.sh --test-nx` | Sprung in Daten muss `#PF` mit Instruktionsabruf auslösen |
| `./run-qemu.sh --test-gp` | nicht kanonische Adresse muss `#GP` geben, nicht `#PF` |
| `./run-qemu.sh --test-ud` | `ud2` muss als `#UD` gemeldet werden |
| `./run-qemu.sh --test-preempt` | Zählschleifen **ohne** `yield` werden verdrängt, Prioritäten verteilen die Ticks |
| `./run-qemu.sh --test-ring3` | Programm in Ring 3 (`CPL=3`) + Negativtest auf eine Kerneladresse |
| `./run-qemu.sh --test-elf` | ELF64 aus dem Startdateisystem laden, Müll definiert abweisen |
| `./run-qemu.sh --test-handles` | Handle-Negativtests: falscher Index, alte Generation, fehlendes Recht |
| `./test.sh` | alles zusammen, 21 Schritte |
| `./rename.sh <kernel> <os>` | benennt Kernel und OS im ganzen Baum um (siehe [RENAME.md](RENAME.md)) |
| `cargo test --target x86_64-unknown-linux-gnu -p osum-mem -p osum-abi-native -p osum-abi-posix` | **151 Host-Tests** der hardwarefreien Logik |

---

## Was der Kernel wirklich tut

Echter Boot vom 13.08.2026, **unveraendert** aus `build/boot.log` kopiert
(erzeugt mit `./run-qemu.sh --check`; nur die lange Memory-Map-Tabelle ist
nach drei Eintraegen gekuerzt). Zeitabhaengige Zahlen (Tickstaende,
Schlafdauer, Schleifendurchlaeufe) schwanken zwischen zwei Laeufen; alles
andere ist reproduzierbar:

```text
================================================================
[osum] boot       osum v0.1.0 — Kernel von Karstos
[osum] boot       Architektur : x86_64 (Basisseite 4096 B)
[osum] boot       ABI         : osum-native + posix (abwaehlbar)
[osum] boot       Konfiguration: Bauart release, POSIX ja, Fehler-Selbsttest keine (normaler Boot)
[osum] boot       CPU         : QEMU TCG CPU version 2.5+
[osum] boot       Bootloader  : Limine 9.6.7
[osum] boot       Stapel      : 64 KiB angefordert, gewaehrt
[osum] mm         HHDM-Fenster: virt = phys + 0xffff800000000000
[osum] mm         Kernelabbild: phys 0x000000001fba6000 -> virt 0xffffffff80000000
[osum] mm           .text  0xffffffff80000000..0xffffffff8002b000  (172 KiB, R+X)
[osum] mm           .rodata0xffffffff8002b000..0xffffffff80034000  (36 KiB, R)
[osum] mm           .data  0xffffffff80034000..0xffffffff8004a000  (88 KiB, R+W, NX)
[osum] mm           gesamt 296 KiB geladenes Abbild (74 Seiten a 4096 B)
[osum] video      Framebuffer : 1280x800 @ 32 bpp -> Textkonsole 160x50
[osum] cpu        GDT + TSS geladen, Datensegmente flach
[osum] cpu        IDT: 256 Tore, Ausnahmen 0..21 belegt
[osum] cpu        schwerste Ausnahme laeuft auf eigenem Notfallstapel (0xffffffff8003ad40)
[osum] cpu        NX aktiv, CR0.WP aktiv
[osum] irq        8259A-PIC (umgeleitet auf 0x20) initialisiert, alle Leitungen maskiert
[osum] trap       #BP Breakpoint bei RIP=0xffffffff80010b01 — setze fort
[osum] cpu        Selbsttest: #BP ausgeloest und sauber fortgesetzt
[osum] mm         Memory-Map (21 Eintraege):
[osum] mm           0x000000001000..0x000000052000    324 KiB  bootloader-reclaimable
[osum] mm           0x000000052000..0x00000009f000    308 KiB  usable
[osum] mm           0x00000009fc00..0x0000000a0000      1 KiB  reserved
[osum] mm         ... (weitere Eintraege der Memory-Map ausgelassen)
[osum] mm         Memory-Map  : 21 Regionen, 77 Frames unter 1 MiB gesperrt, 0 Frames wegen Ueberschneidung nachreserviert
[osum] mm         Speicher    : 12 GiB gesamt, davon 507 MiB nutzbar
[osum] mm         Frames      : 131039 verwaltet, 129712 frei (506 MiB)
[osum] mm         Frame-Bitmap: 16 KiB bei phys 0x000000100000
[osum] mm         Vor Umschaltung geprueft: 5/5 Pflichtbereiche im neuen Adressraum erreichbar
[osum] mm         Abbild nachgeprueft: 74 Seiten virt->phys korrekt (.text RX, .rodata R, .data RW), HHDM-Raender ok
[osum] mm         Eigener Adressraum aktiv (CR3 = 0x000000104000)
[osum] mm           HHDM   : 3 GiB als 2026 grosse Seiten
[osum] mm           Abbild : 74 Seiten mit getrennten Rechten
[osum] mm           Stapel : 0 Seiten vom Bootloader uebernommen
[osum] mm         Selbsttest Paging: map 0xffffff0010000000 -> 0x00000010d000, schreiben/lesen ok, translate ok, unmap ok
[osum] mm         Selbsttest MapError: 23/23 Fehlerfaelle nachweisbar ausgeloest (Misaligned 10, AlreadyMapped 3, NotMapped 4, OutOfFrames 4, ParentHugePage 2)
[osum] mm         Selbsttest Grenzfaelle: 13/13 Zusagen erfuellt (translate mit Versatz, 2-MiB-Seite abbilden/uebersetzen/aufloesen, Tabellengrenze, Ruecknahme halber Teilbaeume), 7 Frames als Tabellen im Probebaum
[osum] mm         Selbsttest Adressrechnung: 8/8 Zusagen erfuellt (Ausrichtung, Ueberlauf, Kanonizitaet, HHDM-Fenster, Rechte, Fehlertexte)
[osum] mm         Selbsttest Memory-Map: 6/6 Zusagen erfuellt (4 kaputte Karten abgewiesen, zerrissene Karte: 257 Frames nachreserviert)
[osum] mm         Selbsttest Frames: 26/26 Zusagen erfuellt, 129693 frei
[osum] mm           Frame-Fehlerpfade: 4 abgewiesene Rueckgaben, 22 erschoepfte Anforderungen, Tiefstand 0 frei
[osum] mm           Frame-Buchhaltung: 1346 belegt + 129693 frei = 131039 verwaltet, stimmig: ja
[osum] heap       Kernel-Heap : 1024 KiB bei virt 0xffffff0000000000 (eigener Free-List-Allocator)
[osum] heap       Testallokation: Box@0xffffff0000000010 = 0xdeadbeef
[osum] heap       Testallokation: Vec mit 1024 Elementen, Summe 357389824, String "Karstos lebt"
[osum] heap       belegt 8272 B von 1048576 B
[osum] heap       nach Freigabe: belegt 0 B von 1048576 B
[osum] heap         Heap-Wachstum: 2166784 B abgebildet (vorher 1048576 B), 1 Erweiterung(en), 5 abgelehnt, Grenze 16777216 B, Spielraum 14610432 B
[osum] heap         Heap-Wachstumsfehler nachweisbar: Groesse 0 ok, ueber der Grenze ok, Frame-Allocator besetzt ok (Frame-Allocator gerade gesperrt)
[osum] heap       Selbsttest Heap: 13/13 Zusagen erfuellt, 0 B belegt, 1 Loecher
[osum] heap         Heap-Fehlerpfade: 16781312 B Anforderung abgewiesen (groesstes Loch 2166784 B), Zustand unveraendert: ja
[osum] heap         Heap-Randfaelle: Groesse 0 ok, Loch wiederverwendet ok, alle 8 Zeiger im Fenster 0xffffff0000000000..0xffffff0000211000 ok
[osum] abi        osum-native: Version=1, Handles=1, altes Handle -> BadHandle
[osum] abi        Ausgabe eines Aufrufers ueber Handle: "POSIX-Uebersetzung auf ein Handle"
[osum] abi        Ausgabe eines Aufrufers ueber Handle: "POSIX open(2) auf ein Capability-Handle"
[osum] posix      Uebersetzer: open(2) auf Namensraumknoten -> fd 0, write -> 39 B, unbekannter Name -> -2 (erwartet -2)
[osum] posix      Uebersetzer: write(fd 0) -> 33 B ueber Handle, close(2), danach write -> -9 (erwartet -9), fork(2) -> -38 (erwartet -38)
[osum] abi        posix-Schicht aktiv: read(2) auf unbekannten Fd -> -9 (erwartet -9 = -EBADF)
[osum] irq        Zeitgeber laeuft mit 100 Hz, Interrupts frei (IF gesetzt)
[osum] irq        Tick 1 — 1 s Laufzeit, 100 Ticks gezaehlt
[osum] irq        Tick 2 — 2 s Laufzeit, 200 Ticks gezaehlt
[osum] irq        Tick 3 — 3 s Laufzeit, 300 Ticks gezaehlt
[osum] irq        Tick 4 — 4 s Laufzeit, 400 Ticks gezaehlt
[osum] irq        Tick 5 — 5 s Laufzeit, 500 Ticks gezaehlt
[osum] sched      Thread A (Kennung 1): Durchlauf 1/3
[osum] sched      Thread B (Kennung 2): Durchlauf 1/3
[osum] sched      Thread A (Kennung 1): Durchlauf 2/3
[osum] sched      Thread B (Kennung 2): Durchlauf 2/3
[osum] sched      Thread A (Kennung 1): Durchlauf 3/3
[osum] sched      Thread B (Kennung 2): Durchlauf 3/3
[osum] sched      Threadwechsel: 16 Wechsel, zurueck in kmain (2 Threads a 16 KiB Stapel, Spur 121212, 5/5 Zusagen)
[osum] sched      Thread W (Kennung 1): blockiert bis zum Wecken
[osum] sched      Thread S (Kennung 2): schlaeft 3 Ticks
[osum] sched      Thread S: nach 3 Ticks aufgewacht
[osum] sched      Thread S: weckt Thread W (Kennung 1)
[osum] sched      Thread W: geweckt, laeuft weiter
[osum] sched      Schlafen/Wecken: 3 Ticks geschlafen (>= 3), 1 Zeitweckung(en), 3 Leerlaufwarten, Wartender geweckt: ja, 8/8 Zusagen
[osum] sched      Leerlauf-Thread: 3 Einlastung(en), 3 Tick(s) im Leerlauf, eigener Stapel 8192 B, Wachzone unversehrt
[osum] sched      Stapelnutzung: hoechstens 576 von 16384 B je Thread benutzt, Wachzonen unversehrt
[osum] sched      Thread V (Kennung 1): blockiert ohne Wecker
[osum] sched      Verklemmungsschutz: run() kehrte nach 2 Wechseln zurueck, 1 Thread(s) blockiert, Meldung gesetzt, 3/3 Zusagen
[osum] sched      Praeemption : aktiv, Zeitscheibe 1 Tick(s) je Prioritaetsstufe, voller Registersatz im Zeitgebereinsprung
[osum] sched      praeemptive Wechsel: 30, kooperative Wechsel: 3 (nur die 3 Abmeldungen)
[osum] sched      ohne yield: 30 Wechsel zwischen 3 Threads (reine Zaehlschleifen, kein einziges yield)
[osum] sched      Thread 1: Prio 3, 30 Ticks, 35228346 Schleifendurchlaeufe
[osum] sched      Thread 2: Prio 2, 20 Ticks, 24767718 Schleifendurchlaeufe
[osum] sched      Thread 3: Prio 1, 10 Ticks, 12275670 Schleifendurchlaeufe
[osum] sched      Prioritaet wirkt: Thread 1 (Prio 3) 30 Ticks > Thread 2 (Prio 2) 20 Ticks > Thread 3 (Prio 1) 10 Ticks
[osum] sched      Leerlauf unter Verdraengung: 5 Einlastung(en), 5 Tick(s) auf den Leerlauf-Thread gebucht, 5 Tick(s) gewartet
[osum] sched      Verdraengungssperre: 4 Tick(s) im geschuetzten Abschnitt, 0 Wechsel darin, 1 nachgeholt nach dem Ende
[osum] sched      Verdraengung: 30 erzwungene Wechsel in 60 Ticks, 8/8 Zusagen
[osum] init       Initramfs   : 3 Eintraege, 2464 B
[osum] init         Eintrag 0: hello          1064 B
[osum] init         Eintrag 1: kaputt.elf     1064 B
[osum] init         Eintrag 2: liesmich.txt   130 B
[osum] elf          gueltiges Abbild             -> angenommen (ok)
[osum] elf          leere Datei                  -> Datei kuerzer als der Kopf (ok)
[osum] elf          falsche Kennung              -> falsche Kennung am Dateianfang (ok)
[osum] elf          32-Bit-Klasse                -> keine 64-Bit-Datei (ok)
[osum] elf          falsche Bytereihenfolge      -> falsche Bytereihenfolge (ok)
[osum] elf          fremde Architektur           -> falsche Zielarchitektur (ok)
[osum] elf          Tabelle ausserhalb           -> Programmkopftabelle ausserhalb der Datei (ok)
[osum] elf          Segment ausserhalb der Datei -> Segment ausserhalb der Datei (ok)
[osum] elf          Segment im Kernelbereich     -> Segment ausserhalb des unprivilegierten Bereichs (ok)
[osum] elf          Dateilaenge > Speicherlaenge -> unstimmige Segmentangaben (ok)
[osum] elf          Ausrichtung krumm            -> unstimmige Ausrichtungsangabe (ok)
[osum] elf          Datei-/Speicherlage unpassend -> unstimmige Ausrichtungsangabe (ok)
[osum] elf          dynamisch gebunden           -> dynamisch gebunden, verlangt einen Binder (ok)
[osum] elf          Einsprung ausserhalb         -> Einsprungadresse in keinem Segment (ok)
[osum] elf          Segmente ueberlappen         -> Segmente ueberlappen sich (ok)
[osum] elf          Segmente in einer Seite      -> zwei Segmente laegen in derselben Seite (ok)
[osum] elf        ELF-Negativtest: 16/16 Faelle wie erwartet abgewiesen
[osum] init         gueltiges Archiv             -> angenommen (ok)
[osum] init         leerer Bereich               -> kuerzer als der Archivkopf (ok)
[osum] init         falsche Kennung              -> falsche Kennung am Archivanfang (ok)
[osum] init         falsche Gesamtlaenge         -> Laengenangabe passt nicht zum Bereich (ok)
[osum] init         zu viele Eintraege           -> unplausible Eintragszahl (ok)
[osum] init         Eintrag ausserhalb           -> Eintrag zeigt aus dem Archiv heraus (ok)
[osum] init         leerer Name                  -> unbrauchbarer Name (ok)
[osum] init         verfaelschte Daten           -> Pruefsumme passt nicht zu den Daten (ok)
[osum] init         Daten mit passender Summe    -> angenommen (ok)
[osum] init         zweimal derselbe Name        -> zwei Eintraege mit demselben Namen (ok)
[osum] init       Archiv-Negativtest: 10/10 Faelle wie erwartet abgewiesen
[osum] elf          Abbild geladen und abgeraeumt -> 6 Seiten abgebildet, 6 zurueckgegeben, 6 Zwischentabelle(n) bleiben, Einsprung nicht mehr abgebildet (ok)
[osum] elf        ELF-Lader   : hello, 2 Segmente geladen, Einsprung 0x0000000000401000
[osum] elf          Segment 0: 0x401000 1 KiB RX (73 B aus der Datei, 0 B genullt)
[osum] elf          Segment 1: 0x402000 1 KiB RW (37 B aus der Datei, 19 B genullt)
[osum] elf          Stapel: 0x00007ffffffff000 abwaerts, 4 Seiten, insgesamt 6 Seiten abgebildet
[osum] elf          nachgeprueft: Einsprungseite und Stapelseite abgebildet, Code stimmt mit der Datei ueberein
[osum] elf          kaputt.elf aus dem Archiv    -> Segment ausserhalb des unprivilegierten Bereichs (ok)
[osum] elf          liesmich.txt als Programm    -> falsche Kennung am Dateianfang (ok)
[osum] elf          Abbild zu gross              -> Abbild belegt zu viele Seiten, 256 Datenseiten zurueckgegeben, 2 Zwischentabelle(n) bleiben (ok)
[osum] elf        ELF-Ladetest: 5/5 Faelle wie erwartet
[osum] user       Systemaufrufpfad: eingerichtet
[osum] user       CPU-Schutz  : Schnellaufruf ja, Ausfuehrsperre ja, Zugriffssperre ja, Per-CPU-Basis ja
[osum] user       Abbildung   : Programm 0x0000000000800000 (272 B, RX), Stapel 0x00007fffffefc000..0x00007ffffff00000 (RW, NX), 5 Seiten
[osum] user       Prozess     : pid 1 unprivilegiert, 1 Handle uebergeben (Ausgabe, nur Schreibrecht), 1 Handle(s) in der Tafel
[osum] trap       #BP aus Ring 3: CS=0x0023 (CPL=3), RSP=0x00007ffffff00000, RIP=0x0000000000800004 — setze fort
[osum] abi        Ausgabe eines Aufrufers ueber Handle: "Gruesse aus Ring 3"
[osum] user       Zeiger 0xffffffff80000000 (+8 B) aus Ring 3 abgewiesen: nicht im eigenen Bereich
[osum] user       Ring 3      : CS=0x0023 (RPL=3), CPL=3, Stapel=0x00007ffffff00000, 4 Systemaufruf(e), Ende 0
[osum] user       Abmeldung   : pid 1 beendet mit Code 0, danach 0 Handle(s) in seiner Tafel
[osum] user       Ring-3-Zugriff auf Kerneladresse 0xffffffff80000000: sauber abgewiesen (Schutzverletzung, Userspace)
[osum] trap       #PF aus Ring 3: CS=0x0023 (CPL=3), RSP=0x00007ffffff00000, RIP=0x000000000080006a, Lesezugriff, Schutzverletzung
[osum] user       Negativtest : Programm nach 0 Systemaufruf(en) abgeraeumt, Kernel laeuft weiter
[osum] user       Ring-3-Zugriff auf 0x0000000000800070: sauber abgewiesen (#GP allgemeine Schutzverletzung, Userspace), Programm abgeraeumt
[osum] user       Schutzwall  : privilegierte Instruktion -> abgewiesen, Programm abgeraeumt, Kernel laeuft (ok)
[osum] user       Ring-3-Zugriff auf 0x0000000000800000: sauber abgewiesen (Schutzverletzung, Userspace), Programm abgeraeumt
[osum] trap       #PF aus Ring 3: CS=0x0023 (CPL=3), RSP=0x00007ffffff00000, RIP=0x0000000000800087, Schreibzugriff, Schutzverletzung
[osum] user       Schutzwall  : Schreiben auf die eigene Codeseite -> abgewiesen, Programm abgeraeumt, Kernel laeuft (ok)
[osum] user       Ring-3-Zugriff auf Kerneladresse 0xffffffff80000000: sauber abgewiesen (Schutzverletzung, Userspace)
[osum] trap       #PF aus Ring 3: CS=0x0023 (CPL=3), RSP=0x00007ffffff00000, RIP=0xffffffff80000000, Lesezugriff, Schutzverletzung
[osum] user       Schutzwall  : Sprung in den Kernelbereich -> abgewiesen, Programm abgeraeumt, Kernel laeuft (ok)
[osum] user       Ring-3-Zugriff auf 0x0000000000600000: sauber abgewiesen (Seite nicht praesent, Userspace), Programm abgeraeumt
[osum] trap       #PF aus Ring 3: CS=0x0023 (CPL=3), RSP=0x00007ffffff00000, RIP=0x00000000008000aa, Lesezugriff, Seite nicht praesent
[osum] user       Schutzwall  : Zeiger auf eine nicht abgebildete Seite -> abgewiesen, Programm abgeraeumt, Kernel laeuft (ok)
[osum] trap       Zeitbudget von Ring 3 erschoepft (RIP=0x00000000008000ba) — Programm wird abgeraeumt
[osum] user       Schutzwall  : Rechenschleife ohne Systemaufruf -> vom Zeitgeber unterbrochen und abgeraeumt (ok)
[osum] user       Schutzwall  : 5/5 Uebergriffe aus Ring 3 abgewehrt
[osum] trap       #BP aus Ring 3: CS=0x0023 (CPL=3), RSP=0x00007ffffffff000, RIP=0x0000000000800052 — setze fort
[osum] abi        Ausgabe eines Aufrufers ueber Handle: "hallo aus der unprivilegierten Ebene."
[osum] user       Aus dem Archiv: Einsprung 0x0000000000401000, CS=0x0023 (CPL=3), Stapel=0x00007ffffffff000, 3 Systemaufruf(e), pid 7 abgemeldet
[osum] user       Ring 3 verdraengt: 15 Unterbrechung(en) durch den Zeitgeber, danach fortgesetzt, CS=0x0023 (RPL=3), Ende 0
[osum] user       Ring 3 verdraengt: Zaehlthread kam auf 12428335 Durchlaeufe, 32 erzwungene und 2 freiwillige Wechsel, Ticks 16/16, 4/4 Zusagen
[osum] user       Abgeraeumt  : 5 Seiten zurueckgegeben, 55 B aus Ring 3 ausgegeben, 6 Zugriff(e) abgewiesen
[osum] abi        Aufruf handle_write aus unprivilegiertem Prozess 8 "gast" abgewiesen: BadHandle
[osum] abi        Aufruf handle_write aus unprivilegiertem Prozess 9 "nackt" abgewiesen: BadHandle
[osum] abi        Aufruf handle_write aus unprivilegiertem Prozess 10 "kurzlebig" abgewiesen: BadHandle
[osum] abi        Handle-Negativtest: 3/3 abgewiesen (ungueltiger Index ok, veraltete Generation ok, fehlendes Recht ok)
[osum] abi        Capability-Negativtest: 12/12 abgewiesen — fremdes Handle ok, ohne Uebergabe kein Zugriff ok, keine Rechteausweitung ok, Duplizieren ohne Recht ok, Uebergabe ohne Recht ok
[osum] abi        Capability-Negativtest: unbekannte Aufrufnummer ok, Generation geraten (0 Treffer bei 4096 Versuchen), Benutzung nach Schliessen ok, Handle nach Prozessende ok
[osum] abi        Prozessprobe: spawn ohne fork, Kind "dienst" mit 1 explizit uebergebenen Handle(n); Kanal: 22 B gesendet, 22 B empfangen ("hallo aus dem Erzeuger"), Schreiben ohne Recht -> RightsDenied
[osum] abi        Portprobe (Signals-Ersatz): 7/7 — Bindung eingerichtet ok, Ereignis zugestellt ok, Frist 0 blockiert nicht ok, Ende der Gegenseite ok
[osum] abi        Port-Negativtest: binden ohne MANAGE abgewiesen ok, beobachten ohne WAIT abgewiesen ok, warten auf Nicht-Port abgewiesen ok
[osum] abi        Ausgabe eines Aufrufers ueber Handle: "handle unterwegs"
[osum] abi        Uebergabeprobe: Handle per Kanal umgezogen ok (Sender 0x7f6c280a00000011 -> Empfaenger 0x8b1dce4a00000002), beim Sender ungueltig ok, ohne Platz keine Zustellung ok
[osum] abi        Uebergabe-Negativtest: ohne TRANSFER-Recht abgewiesen ok, dasselbe Handle doppelt abgewiesen ok
[osum] abi        Ausgabe eines Aufrufers ueber Handle: "Ausgabe ueber einen aufgeloesten Namen"
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Aufruf namespace_open aus unprivilegiertem Prozess 13 "namenlos" abgewiesen: InvalidArgs
[osum] abi        Namensraumprobe: 11/11 — Name aufgeloest und benutzt ok (38 B ueber "konsole" geschrieben), Unterknoten ok
[osum] abi        Namensraum-Negativtest: Pfadtrenner abgewiesen ok, unbekannter Name abgewiesen ok, keine Rechteausweitung beim Aufloesen ok, Einhaengen ohne CREATE abgewiesen ok
[osum] abi        Namensraum-Negativtest: Einhaengen ohne DUPLICATE abgewiesen ok, kein Knoten abgewiesen ok, Name doppelt abgewiesen ok, Name zu lang abgewiesen ok, ohne Knotenhandle kein Name ok
[osum] abi        Prozesse    : 14 in der Tafel, 13 per spawn erzeugt, 132 Aufrufe, 90 abgewiesen (davon 67 aus unprivilegierten Prozessen), 24 Objekte
[osum] abi        Prozesse    : 14, davon beendet 12, Speicherobjekte 0 B; Namen: 0:systemkern, 1:ring3, 2:probe, 3:probe, 4:probe, 5:probe, 6:probe, 7:hello, 8:gast, 9:nackt, 10:kurzlebig, 11:dienst, 12:empfaenger, 13:namenlos
[osum] boot       Selbsttestbilanz: 170/170 bestanden (#BP 1/1, Paging 3/3, MapError 36/36, Frames 26/26, Heap 13/13, Praeemption 8/8, ELF 31/31, Ring3 14/14, Handles 38/38)
[osum] boot       Startbilanz : 129147 Frames frei (1892 belegt), Heap 6288 / 2166784 B belegt, 650 Ticks
[osum] boot       Startvorgang abgeschlossen. osum laeuft.
================================================================
```

Die drei Nachweise, auf die es in dieser Phase ankommt, im Klartext:

* **Verdrängung ist echt:** `praeemptive Wechsel: 30, kooperative Wechsel: 3` —
  die drei Threads sind reine Zählschleifen und rufen nie `yield`. Die
  Tickverteilung 30 : 20 : 10 folgt exakt den Prioritätsstufen 3 : 2 : 1.
* **Ring 3 ist echt:** `CS=0x0023 (RPL=3), CPL=3` und ein Stapel bei
  `0x00007ffffff00000` — beides an einem echten Ausnahmerahmen der Hardware
  abgelesen, nicht vom Kernel behauptet. Der Zugriff auf `0xffffffff80000000`
  endet in einem gemeldeten `#PF`, und der Kernel läuft weiter.
* **Capabilities sind echt:** falscher Index, veraltete Generation und
  fehlendes Recht werden **einzeln** abgewiesen; 4096 geratene Generationen
  ergeben 0 Treffer.

### Fehlerfälle sind nachweisbar, nicht behauptet

```
$ ./run-qemu.sh --test-doublefault
[osum] test       loese absichtlich einen Double Fault aus ...
=============== CPU-AUSNAHME ===============
Ausnahme : #DF Double Fault
IST-Stapel: 0xffffffff8003ad10 (eigener Stack, deshalb lebt der Kernel noch)
Kein Weiterlaufen moeglich — CPU wird angehalten.
```

```
$ ./run-qemu.sh --test-pagefault
Ausnahme : #PF Page Fault
Adresse  : 0xffffff4000000000 (CR2)
Ursache  : Seite nicht praesent, Schreibzugriff, ausgeloest im Kernel
=============== KERNEL PANIC ===============
Backtrace (Frame-Pointer, Adressen via addr2line aufloesbar):
  #0  ip=0xffffffff800275dc
  #1  ip=0xffffffff8000c23d
  #2  ip=0xffffff4000000000
Backtrace: 3 Frame(s) aufgeloest
```

Backtrace-Adressen auflösen:

```sh
llvm-addr2line -e target/x86_64-osum-none/release/osum -f -C 0xffffffff800275dc
# oder in build/osum.map nachschlagen
```

---

## Verzeichnisse

```
karstos/
├── build.sh  run-qemu.sh  test.sh      Bauen, Starten, Prüfen
├── tests/step-16..19.sh                je ein Nachweis (test.sh hängt sie an)
├── x86_64-osum-none.json              eigenes Target
├── limine.conf                         Bootmenü + Modul „initramfs“
├── kernel/
│   ├── linker-x86_64.ld                higher-half bei -2 GiB
│   └── src/
│       ├── main.rs                     kmain — der ganze Bootweg an einer Stelle
│       ├── kcore/                      Typen und Traits, architekturneutral
│       │   ├── arch_iface.rs           ← die arch-Grenze
│       │   ├── sched.rs                Scheduler-Trait, Threads, Zeitscheiben, Prioritäten
│       │   ├── preempt.rs              Tickbuchung, getrennte Wechselzähler
│       │   ├── user.rs                 was unprivilegiert läuft (nicht wie)
│       │   ├── elf.rs                  ELF64-Prüfung und Ladeaufträge
│       │   ├── initramfs.rs            Archivformat „IRFS0002“ (CRC32 je Eintrag)
│       │   └── mem.rs  print.rs  panic.rs  branding.rs
│       ├── arch/x86_64/                ← einzige Stelle mit x86-Details
│       │   ├── gdt.rs idt.rs interrupts.rs paging.rs pic.rs timer.rs
│       │   ├── context.rs              kooperativer Kontextwechsel (naked_asm)
│       │   ├── preempt.rs              IRQ0-Einsprung mit vollem Registersatz (naked_asm)
│       │   ├── user.rs                 Privilegwechsel, Systemaufrufpfad, Per-CPU-Block
│       │   └── serial.rs cpu.rs msr.rs port.rs selftest.rs
│       ├── mm/                         Frames, Adressräume, Heap, Selbsttests
│       ├── drivers/                    Framebuffer-Konsole + eigener 8×16-Font
│       ├── boot/limine.rs              einzige Datei, die das Bootprotokoll kennt
│       └── abi/                        native.rs + posix.rs (Feature-gated)
├── userland/                           hello.asm, user.ld, mkinitramfs.py, mkbroken.py
├── libs/
│   ├── osum-mem/                      Bitmap-Allocator, Heap, Regionen — host-getestet
│   ├── osum-abi-native/               capability-basierte ABI + Handle-Tabelle
│   └── osum-abi-posix/                abtrennbare POSIX-Übersetzung
└── vendor/limine/                      Bootloader-Binaries (offline nutzbar)
```

---

## Kennzahlen

Alle Zahlen sind gemessen, nicht geschätzt — die Befehle stehen daneben.

| | | gemessen mit |
|---|---|---|
| Rust-Zeilen (`kernel/` + `libs/`) | 18 290 | `find kernel/src libs -name '*.rs' -exec cat {} + \| wc -l` |
| Externe Crates | **2** (`limine`, `spin`; `bitflags` transitiv) | `Cargo.lock` |
| Geladene Kernelgröße | **289,4 KiB** (`.text` 168,6 + `.rodata` 33,0 + `.data` 6,6 + `.bss` 80,8 + Requests 0,4); auf Seiten gerundet 296 KiB = 74 Seiten | `size -A target/x86_64-osum-none/release/osum` |
| Startdateisystem | **2464 B**, 3 Einträge (`hello`, `kaputt.elf`, `liesmich.txt`) | `stat -c%s build/initramfs.img` |
| Host-Tests | **151**, alle grün (75 `osum-mem` + 55 `osum-abi-native` + 21 `osum-abi-posix`) | `cargo test --target x86_64-unknown-linux-gnu -p ...` |
| Prüfmerkmale je QEMU-Boot | **38** | `run-qemu.sh --check` |
| Selbsttest-Zusagen im Kernel | **170** | Boot-Log, Zeile „Selbsttestbilanz" |
| Schritte in `./test.sh` | **21** (davon 15 echte QEMU-Boots) | `./test.sh` |
| Compilerwarnungen | **0** — erzwungen im Bausystem (`[workspace.lints.rust] warnings = "deny"`), geprüft an einem echten Neuübersetzungs-Log | `build/cargo-build-fresh.log` |
| Nightly-Features | 4, einzeln begründet | ARCHITECTURE.md § 9 |
| Produktname im Quelltext | **0 Stellen** ausser `branding.rs` | `./test.sh` Schritt 14 |

---

## Grundregeln

1. **Kein Linux-Code, kein Fork.** Nachbauen aus Spezifikationen ja, übernehmen nein.
2. **x86-Details ausschließlich unter `arch/x86_64/`** — nachgeprüft in `test.sh` Schritt 15.
3. **POSIX ist abtrennbar** und wird in jedem Testlauf ohne sich selbst gebootet.
4. **Keine Crate ohne Begründung**, keine `todo!()` in Pfaden, die der Boot durchläuft.
5. **Keine Behauptung ohne Code dahinter.** Kennzahlen werden nachgemessen, wenn sie sich ändern.

## Entwurfsdokumente

| Datei | Inhalt |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Schichten, arch-Grenze, POSIX-Abtrennung, Abhängigkeiten |
| [ROADMAP.md](ROADMAP.md) | Phasen 1–9, inklusive aarch64/Mobil als eigenes Ökosystem |
| [PACKAGING.md](PACKAGING.md) | unveränderliche content-addressed Apps, drei Datentöpfe, Systemgenerationen |
| [FILESYSTEM.md](FILESYSTEM.md) | VFS → FAT32 → ext4 (lesend) → eigenes Dateisystem |
| [LANGUAGE.md](LANGUAGE.md) | Logbuch: wo Rust im Kernel im Weg steht |
| [RENAME.md](RENAME.md) | Kernel und OS in unter 15 Minuten umbenennen |
