# Karstos — Betriebssystem von Grund auf

**Kernel: `karst` · OS: `Karstos` · Sprache: Rust (`no_std`) · Ziel: `x86_64-karst-none`**

Kein Linux-Fork, kein übernommener Fremdcode. Der Kernel bootet auf BIOS **und**
UEFI, baut seine eigenen Seitentabellen, hat einen funktionierenden Heap, prüft
sich beim Start mit 126 eingebauten Zusagen selbst, **verdrängt** Kernel-Threads
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
| `./build.sh` | Release-Build + Userland + `build/initramfs.img` + ISO (`build/karstos.iso`) + Symbolkarte (`build/karst.map`) |
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
| `./test.sh` | alles zusammen, 19 Schritte |
| `./rename.sh <kernel> <os>` | benennt Kernel und OS im ganzen Baum um (siehe [RENAME.md](RENAME.md)) |
| `cargo test --target x86_64-unknown-linux-gnu -p karst-mem -p karst-abi-native -p karst-abi-posix` | **141 Host-Tests** der hardwarefreien Logik |

---

## Was der Kernel wirklich tut

Echter Boot, aufgezeichnet in `build/boot.log` (gekürzt — Memory-Map-Tabelle
und Wiederholungen entfernt, sonst wörtlich). Zeitabhängige Zahlen
(Tickstände, Schlafdauer, Schleifendurchläufe) schwanken zwischen zwei Läufen
um ein bis zwei Einheiten; alles andere ist reproduzierbar:

```
[karst] boot       karst v0.1.0 — Kernel von Karstos
[karst] boot       Architektur : x86_64 (Basisseite 4096 B)
[karst] boot       ABI         : karst-native + posix (abwaehlbar)
[karst] boot       Konfiguration: Bauart release, POSIX ja, Fehler-Selbsttest keine (normaler Boot)
[karst] boot       CPU         : QEMU Virtual CPU version 2.5+
[karst] boot       Bootloader  : Limine 9.6.7
[karst] mm         HHDM-Fenster: virt = phys + 0xffff800000000000
[karst] mm         Kernelabbild: phys 0x000000001fc30000 -> virt 0xffffffff80000000
[karst] mm           .text  0xffffffff80000000..0xffffffff80020000  (128 KiB, R+X)
[karst] mm           .rodata0xffffffff80020000..0xffffffff80027000  (28 KiB, R)
[karst] mm           .data  0xffffffff80027000..0xffffffff8003d000  (88 KiB, R+W, NX)
[karst] mm           gesamt 244 KiB geladenes Abbild (61 Seiten a 4096 B)
[karst] video      Framebuffer : 1280x800 @ 32 bpp -> Textkonsole 160x50
[karst] cpu        GDT + TSS geladen, Datensegmente flach
[karst] cpu        IDT: 256 Tore, Ausnahmen 0..21 belegt
[karst] cpu        schwerste Ausnahme laeuft auf eigenem Notfallstapel (0xffffffff8002db90)
[karst] cpu        NX aktiv, CR0.WP aktiv
[karst] irq        8259A-PIC (umgeleitet auf 0x20) initialisiert, alle Leitungen maskiert
[karst] trap       #BP Breakpoint bei RIP=0xffffffff8000d25d — setze fort
[karst] mm         Memory-Map  : 21 Regionen, 77 Frames unter 1 MiB gesperrt, 0 Frames wegen Ueberschneidung nachreserviert
[karst] mm         Frames      : 131039 verwaltet, 129846 frei (507 MiB)
[karst] mm         Eigener Adressraum aktiv (CR3 = 0x000000104000)
[karst] mm         Selbsttest MapError: 23/23 Fehlerfaelle nachweisbar ausgeloest
[karst] mm         Selbsttest Frames: 26/26 Zusagen erfuellt, 129827 frei
[karst] heap       Kernel-Heap : 1024 KiB bei virt 0xffffff0000000000 (eigener Free-List-Allocator)
[karst] heap       Testallokation: Vec mit 1024 Elementen, Summe 357389824, String "Karstos lebt"
[karst] heap       Selbsttest Heap: 13/13 Zusagen erfuellt, 0 B belegt, 1 Loecher
[karst] abi        karst-native: Version=1, Handles=1, altes Handle -> BadHandle
[karst] abi        posix-Schicht aktiv: read(2) auf unbekannten Fd -> -9 (erwartet -9 = -EBADF)
[karst] irq        Zeitgeber laeuft mit 100 Hz, Interrupts frei (IF gesetzt)
[karst] sched      Threadwechsel: 16 Wechsel, zurueck in kmain (2 Threads a 16 KiB Stapel, Spur 121212, 5/5 Zusagen)
[karst] sched      Schlafen/Wecken: 4 Ticks geschlafen (>= 3), 1 Zeitweckung(en), 3 Leerlaufwarten, 7/7 Zusagen
[karst] sched      Verklemmungsschutz: run() kehrte nach 2 Wechseln zurueck, 1 Thread(s) blockiert, 3/3 Zusagen
[karst] sched      Praeemption : aktiv, Zeitscheibe 1 Tick(s) je Prioritaetsstufe, voller Registersatz im Zeitgebereinsprung
[karst] sched      praeemptive Wechsel: 30, kooperative Wechsel: 3 (nur die 3 Abmeldungen)
[karst] sched      ohne yield: 30 Wechsel zwischen 3 Threads (reine Zaehlschleifen, kein einziges yield)
[karst] sched      Thread 1: Prio 3, 30 Ticks, 36589059 Schleifendurchlaeufe
[karst] sched      Thread 2: Prio 2, 20 Ticks, 25333236 Schleifendurchlaeufe
[karst] sched      Thread 3: Prio 1, 10 Ticks, 12940059 Schleifendurchlaeufe
[karst] sched      Prioritaet wirkt: Thread 1 (Prio 3) 30 Ticks > Thread 2 (Prio 2) 20 Ticks > Thread 3 (Prio 1) 10 Ticks
[karst] init       Initramfs   : 3 Eintraege, 2448 B
[karst] init         Eintrag 0: hello          1064 B
[karst] elf        ELF-Negativtest: 11/11 Faelle wie erwartet abgewiesen
[karst] init       Archiv-Negativtest: 7/7 Faelle wie erwartet abgewiesen
[karst] elf        ELF-Lader   : hello, 2 Segmente geladen, Einsprung 0x0000000000401000
[karst] elf          Segment 0: 0x401000 1 KiB RX (73 B aus der Datei, 0 B genullt)
[karst] elf          Segment 1: 0x402000 1 KiB RW (37 B aus der Datei, 19 B genullt)
[karst] elf          Stapel: 0x00007ffffffff000 abwaerts, 4 Seiten, insgesamt 6 Seiten abgebildet
[karst] elf        ELF-Ladetest: 4/4 Faelle wie erwartet
[karst] user       CPU-Schutz  : Schnellaufruf ja, Ausfuehrsperre nein (uebersprungen), Zugriffssperre nein (uebersprungen), Per-CPU-Basis ja
[karst] user       Prozess     : pid 1 unprivilegiert, 1 Handle uebergeben (Ausgabe, nur Schreibrecht)
[karst] trap       #BP aus Ring 3: CS=0x0023 (CPL=3), RSP=0x00007ffffff00000, RIP=0x0000000000800004 — setze fort
[karst] abi        Ausgabe eines Aufrufers ueber Handle: "Gruesse aus Ring 3"
[karst] user       Ring 3      : CS=0x0023 (RPL=3), CPL=3, Stapel=0x00007ffffff00000, 4 Systemaufruf(e), Ende 0
[karst] user       Ring-3-Zugriff auf Kerneladresse 0xffffffff80000000: sauber abgewiesen (Schutzverletzung, Userspace)
[karst] trap       #PF aus Ring 3: CS=0x0023 (CPL=3), RSP=0x00007ffffff00000, RIP=0x000000000080006a, Lesezugriff, Schutzverletzung
[karst] abi        Ausgabe eines Aufrufers ueber Handle: "hallo aus der unprivilegierten Ebene."
[karst] user       Aus dem Archiv: Einsprung 0x0000000000401000, CS=0x0023 (CPL=3), Stapel=0x00007ffffffff000, 3 Systemaufruf(e), pid 2 abgemeldet
[karst] abi        Handle-Negativtest: 3/3 abgewiesen (ungueltiger Index ok, veraltete Generation ok, fehlendes Recht ok)
[karst] abi        Capability-Negativtest: 12/12 abgewiesen — fremdes Handle ok, ohne Uebergabe kein Zugriff ok, keine Rechteausweitung ok, ...
[karst] abi        Prozessprobe: spawn ohne fork, Kind "dienst" mit 1 explizit uebergebenen Handle(n); Kanal: 22 B gesendet, 22 B empfangen
[karst] boot       Selbsttestbilanz: 126/126 bestanden (#BP 1/1, Paging 3/3, MapError 36/36, Frames 26/26,
                   Heap 13/13, Praeemption 5/5, ELF 22/22, Ring3 5/5, Handles 15/15)
[karst] boot       Startbilanz : 129281 Frames frei (1758 belegt), Heap 2688 / 2166784 B belegt, 587 Ticks
[karst] boot       Startvorgang abgeschlossen. karst laeuft.
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
[karst] test       loese absichtlich einen Double Fault aus ...
=============== CPU-AUSNAHME ===============
Ausnahme : #DF Double Fault
IST-Stapel: 0xffffffff8001ea20 (eigener Stack, deshalb lebt der Kernel noch)
Kein Weiterlaufen moeglich — CPU wird angehalten.
```

```
$ ./run-qemu.sh --test-pagefault
Ausnahme : #PF Page Fault
Adresse  : 0xffffff4000000000 (CR2)
Ursache  : Seite nicht praesent, Schreibzugriff, ausgeloest im Kernel
=============== KERNEL PANIC ===============
Backtrace (Frame-Pointer, Adressen via addr2line aufloesbar):
  #0  ip=0xffffffff8000610c
  #1  ip=0xffffffff80000f20
```

Backtrace-Adressen auflösen:

```sh
llvm-addr2line -e target/x86_64-karst-none/release/karst -f -C 0xffffffff8000610c
# oder in build/karst.map nachschlagen
```

---

## Verzeichnisse

```
karstos/
├── build.sh  run-qemu.sh  test.sh      Bauen, Starten, Prüfen
├── tests/step-16..19.sh                je ein Nachweis (test.sh hängt sie an)
├── x86_64-karst-none.json              eigenes Target
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
│       │   ├── initramfs.rs            Archivformat „IRFS0001“
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
│   ├── karst-mem/                      Bitmap-Allocator, Heap, Regionen — host-getestet
│   ├── karst-abi-native/               capability-basierte ABI + Handle-Tabelle
│   └── karst-abi-posix/                abtrennbare POSIX-Übersetzung
└── vendor/limine/                      Bootloader-Binaries (offline nutzbar)
```

---

## Kennzahlen

Alle Zahlen sind gemessen, nicht geschätzt — die Befehle stehen daneben.

| | | gemessen mit |
|---|---|---|
| Rust-Zeilen (`kernel/` + `libs/`) | 15 479 | `find kernel/src libs -name '*.rs' -exec cat {} + \| wc -l` |
| Externe Crates | **2** (`limine`, `spin`; `bitflags` transitiv) | `Cargo.lock` |
| Geladene Kernelgröße | **242,0 KiB** (`.text` 127,1 + `.rodata` 27,4 + `.data` 6,3 + `.bss` 80,7 + Requests 0,4) | `size -A target/x86_64-karst-none/release/karst` |
| Startdateisystem | **2448 B**, 3 Einträge (`hello`, `kaputt.elf`, `liesmich.txt`) | `stat -c%s build/initramfs.img` |
| Host-Tests | **141**, alle grün (75 + 45 + 21) | `cargo test --target x86_64-unknown-linux-gnu -p ...` |
| Prüfmerkmale je QEMU-Boot | **38** | `run-qemu.sh --check` |
| Selbsttest-Zusagen im Kernel | **126** | Boot-Log, Zeile „Selbsttestbilanz" |
| Schritte in `./test.sh` | **19** (davon 14 echte QEMU-Boots) | `./test.sh` |
| Compilerwarnungen | **0** (erzwungen in `test.sh` Schritt 15) | `build/cargo-build.log` |
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
