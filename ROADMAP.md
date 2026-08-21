# Roadmap von Karstos

Ziel: ein eigenständiges, langfristig linuxfähiges Betriebssystem — kein Fork,
kein übernommener Code. Zeithorizont: Jahre. Reihenfolge ist nach Abhängigkeit
sortiert, nicht nach Attraktivität.

Legende: **✔ fertig** · **◐ teilweise** · **▶ als Nächstes** · **○ geplant**

---

## Phase 1 — Bootfähiger Kern ✔ *(abgeschlossen)*

* ✔ Build-System: eigenes Target `x86_64-osum-none`, `build.sh`, `run-qemu.sh`, `test.sh`
* ✔ Limine-Boot (BIOS **und** UEFI), higher-half Kernel bei −2 GiB
* ✔ Serielle Konsole (COM1) + Framebuffer-Textkonsole mit eigenem 8×16-Font
* ✔ `println!`/`serial_println!`/`klog!`, Panic-Handler mit Frame-Pointer-Backtrace
* ✔ GDT + TSS mit drei IST-Stapeln, IDT mit 256 Toren, Ausnahmen 0–21 im Klartext
* ✔ Echter Double-Fault-Test (nicht `int 8`) auf eigenem IST-Stapel
* ✔ PIC umgeleitet, PIT mit 100 Hz, Timer-Interrupts nachweisbar im Log
* ✔ Memory-Map-Auswertung, Bitmap-Frame-Allocator
* ✔ Eigene 4-Ebenen-Seitentabellen, CR3-Umschaltung, Rechte je Sektion, NX, CR0.WP
* ✔ Eigener Heap-Allocator, `alloc` funktioniert (Box/Vec/String im Boot-Log)
* ✔ arch-Trait-Grenze, POSIX per Feature abwählbar (Gegenprobe im Testlauf)

## Phase 2 — Scheduler und Zeit ✔ *(bis auf APIC und Nanosekundenuhr)*

* ✔ Kontextwechsel in Assembler (`arch/x86_64/context.rs`, `naked_asm!`),
  Kernel-Threads mit eigenem Heap-Stapel und Wachzone
* ✔ Kooperativer Round-Robin-Scheduler hinter dem `Scheduler`-Trait in
  `kcore/sched.rs`; `run()` kehrt nach `kmain` zurück, wenn niemand mehr
  lauffähig ist, und meldet eine Verklemmung statt hängen zu bleiben
* ✔ `yield_now`, `block_current`/`unblock`, `sleep_ticks` — Wecken durch den
  Zeitgeber (der Scheduler wartet derweil mit `wait_for_interrupt`); im Boot-Log
  als drei Selbsttests nachweisbar (Wechselspiel, Schlafen/Wecken, Verklemmung)
* ✔ **Präemption durch den Zeitgeber**: eigener IRQ0-Einsprung mit vollem
  Registersatz (`arch/x86_64/preempt.rs`, `naked_asm!`), Buchung und
  Wechselentscheidung in `kcore/preempt.rs` + `kcore/sched.rs`. Nachweis im
  Boot-Log: 30 erzwungene Wechsel zwischen drei Zählschleifen **ohne** ein
  einziges `yield`, getrennt von 3 kooperativen Wechseln
* ✔ **Zeitscheiben und Prioritäten** hinter demselben `Scheduler`-Trait
  (`spawn_prio`, `charge_tick`, `preempt_current`); Tickverteilung 30 : 20 : 10
  bei Prioritäten 3 : 2 : 1, im Log gemessen
* ✔ **Leerlauf-Thread statt `wait_for_interrupt`**: eigener Thread mit eigenem
  Stapel (8 KiB, Wachzone), wird von `pick()` nie gewählt, sondern nur
  eingelastet, wenn sonst niemand lauffähig ist; fällt ohne Heap sauber auf
  `wait_for_interrupt` zurück. Im Boot-Log: `Leerlauf-Thread: 3 Einlastung(en),
  3 Tick(s) im Leerlauf, eigener Stapel 8192 B, Wachzone unversehrt` sowie
  `Leerlauf unter Verdraengung: 5 Einlastung(en), 5 Tick(s) …`
* ✔ **Verdrängung von Ring-3-Programmen mit Fortsetzung.** Läuft das Programm
  in einem Thread mit eigenem Kernelstapel (`user::set_preemptible`), legt die
  CPU den Rahmen des Privilegwechsels auf genau diesen Stapel — der Zeitgeber
  darf dann mitten im Rahmen umschalten, und das `iretq` am Ende des Einsprungs
  setzt das Programm später exakt dort fort. Nachweis in **jedem** Boot: ein
  Rechenprogramm ohne einen einzigen Systemaufruf wird unterbrochen und endet
  trotzdem regulär, während ein zweiter Thread vorankommt —
  `Ring 3 verdraengt: 12 Unterbrechung(en) durch den Zeitgeber, danach
  fortgesetzt, CS=0x0023 (RPL=3), Ende 0`
* ✔ Wachhund für Ring-3-Code, der NICHT in einem eigenen Thread läuft (Rahmen
  liegt dann auf dem gemeinsamen Systemaufrufstapel): Zeitbudget, danach wird
  abgeräumt statt gewechselt. Im Boot-Log: `Zeitbudget von Ring 3 erschoepft
  (RIP=…) — Programm wird abgeraeumt` und `Schutzwall: Rechenschleife ohne
  Systemaufruf -> vom Zeitgeber unterbrochen und abgeraeumt (ok)`;
  damit 5/5 statt 4/4 abgewehrten Übergriffen
* ○ APIC statt PIC: LAPIC-Timer, IOAPIC-Umleitungen, TSC-Deadline
* ○ Monotone Uhr in Nanosekunden (`ClockRead`), Kalibrierung gegen HPET/PIT

## Phase 3 — Userspace und Syscalls ▶ *(Grundlage steht, bootet, ist getestet)*

* ✔ `syscall`/`sysret` verdrahtet (Modellregister und SCE-Bit), Per-CPU-Block
  mit Basisregisterwechsel, getrennte Kernel-/User-Stapel, Ring-0-Eintrag im
  Aufgabensegment wird beim Threadwechsel nachgezogen — alles in
  `arch/x86_64/user.rs`, nichts davon außerhalb von `arch/`
* ✔ User-Seiten mit Benutzerbit, Kernelseiten ohne; Negativtest im Boot-Log:
  Ring-3-Zugriff auf eine Kerneladresse endet in einem gemeldeten `#PF`, der
  Kernel läuft weiter
* ✔ SMEP/SMAP werden gesetzt, **wenn** CPUID sie meldet, sonst sauber
  übersprungen und im Log vermerkt (QEMU-Standard-CPU meldet sie nicht)
* ✔ ELF-Lader für statisch gebundene Binaries samt Rückrollen bei Fehlern;
  14 ELF- und 8 Archiv-Negativfälle im Boot-Log
* ✔ Startdateisystem als Limine-Modul, eigenes Format `IRFS0001`
  (Begründung gegen cpio/tar in `kcore/initramfs.rs` und ARCHITECTURE.md § 5c)
* ✔ Erstes unprivilegiertes Programm (`userland/hello.asm`) läuft **wirklich**
  in Ring 3: `CS=0x0023 (RPL=3), CPL=3`, gibt über ein explizit übergebenes
  Handle aus und beendet sich per Systemaufruf; der Kernel räumt es ab
* ✔ `osum-native` gebaut: Handle-Tabelle je Prozess (Slot + Generation +
  prozesseigener Würfelwert), Objekttafel mit Verweiszähler,
  `HandleWrite/Read/Close/Duplicate/Inspect`, `ChannelCreate/Send/Recv`,
  `MemoryCreate`, `ProcessExit`, `spawn`/`spawn_with` **statt `fork`**;
  Negativfälle im Boot-Log: 3 Handle- + 12 Capability-Fälle (Selbsttestgruppe
  „Handles" insgesamt 38/38, inklusive der Port-, Übergabe- und
  Namensraum-Negativfälle)
* ▶ **Adressraum je Prozess** — heute liegen die unprivilegierten Seiten im
  Kerneladressraum (mit Benutzerbit); zwei unprivilegierte Prozesse sind
  voneinander noch nicht isoliert. Das ist der nächste echte Brocken
* ▶ Ein `init`, das nach dem Boot **die Kontrolle behält**, statt als
  Selbsttest zu enden
* ✔ **`Port*` als Ereignisweg statt Signalen** (`PortCreate/Bind/Wait`):
  Bindung verlangt `MANAGE` am Port **und** `WAIT` am Objekt; Ereignisse aus
  Kanal-Senden, Kanalende und Prozessende. Im Boot-Log `Portprobe
  (Signals-Ersatz): 7/7` und ein Port-Negativtest mit drei Fällen
* ✔ **`ThreadSleep`** an `kcore::sched::sleep_ticks` angebunden
* ✔ **Handle-Übergabe per Kanal**: alles vorab geprüft, Duplikate abgewiesen,
  ohne Platz bleibt die Nachricht liegen (kein Handle geht verloren) —
  `Uebergabeprobe` und `Uebergabe-Negativtest` im Boot-Log
* ✔ **`NamespaceCreate`/`NamespaceOpen`** gebaut: Namensknoten sind selbst
  Objekte hinter einem Handle — es gibt **keinen prozessglobalen Wurzelpfad**.
  Einhängen verlangt `CREATE` am Knoten und `DUPLICATE` an der Quelle; beim
  Auflösen können Rechte nur schrumpfen. Namensregeln in
  `libs/osum-abi-native/src/name.rs` (`NAME_MAX=32`, kein `/`, `\`, `.`,
  `..`, keine Steuerzeichen), host-getestet über alle 256 Bytes.
  Im Boot-Log: `Namensraumprobe: 11/11` und neun Negativfälle
* ✔ **Archivprüfsummen**: `IRFS0002` führt CRC32 je Eintrag, geprüft **vor**
  jedem Datenzugriff — ein verfälschtes Byte wird abgewiesen
* ○ `MemoryMap/Unmap`, `ThreadCreate`, `ProcessSpawn` als Syscall (heute
  `NotSupported`; Prozesserzeugung weiterhin nur kernelseitig)
* ◐ POSIX-Schicht: `open`/`close` übersetzen bereits auf echte Handles, aber nur
  gegen einen **übergebenen Namensraumknoten** (`open(2)` liefert einen Fd,
  unbekannter Name `-ENOENT`) — ein durchgängiger Pfadbaum braucht das VFS aus
  Phase 4

## Phase 4 — Dateisysteme ○

Reihenfolge und Begründung ausführlich in [FILESYSTEM.md](FILESYSTEM.md).

* ○ **VFS** als Namespace-Baum aus Handles, FS-Treiber hinter einem Trait —
  kein globaler Root, keine erzwungene inode/dentry-Semantik
* ○ Blockgeräte-Trait, Puffercache; Treiber AHCI/SATA, dann NVMe
* ○ **FAT32 lesen und schreiben** — Pflicht, die EFI System Partition ist FAT;
  ohne das kein eigener Bootloader und kein Selbst-Update auf UEFI
* ○ **ext4 nur lesend** — Interoperabilität ohne Risiko für fremde Daten
* ○ `initrd` als Speicher-Dateisystem für den frühen Start
* ○ Eigenes Dateisystem **erst danach** (Phase 8): Copy-on-Write-Snapshots für
  Systemgenerationen, Prüfsummen, blockweises Dedup für den App-Store

## Phase 5 — Treiber und Geräte ○

* ○ PCIe-Aufzählung (MCFG aus ACPI), MSI/MSI-X
* ○ ACPI-Parser (RSDP/XSDT/MADT/FADT), sauberes Herunterfahren, Neustart
* ○ Tastatur (PS/2 und USB-HID), Maus
* ○ USB-Stack: XHCI, dann Massenspeicher und HID
* ○ Grafik: Modussetzung über GOP hinaus, später eigene KMS-Entsprechung

## Phase 6 — SMP ○

* ○ Anwendungsprozessoren starten (INIT-SIPI-SIPI), Per-CPU-Datenbereiche
* ○ Sperrenhierarchie festlegen; serielle Ausgabe bekommt einen Per-CPU-Puffer
* ○ Lastverteilung, CPU-Affinität, TLB-Shootdown über IPIs
* ○ Lockfreie Pfade dort, wo Messungen es rechtfertigen — nicht vorher

### Regel für IPIs: warten und Preemption sperren sind zwei Dinge, nie eins

**Festgelegt, bevor die erste Zeile SMP-Code steht.** Wer einen anderen Kern
per IPI zu etwas auffordert und auf dessen Rückmeldung wartet, hält währenddessen
**nicht** die Verdrängung gesperrt. Wer noch aussteht, wird **je Task** vermerkt
(eine eigene CPU-Maske), nicht in einer gemeinsamen Struktur — nur dann darf der
Scheduler den Wartenden verdrängen, während die Antworten noch unterwegs sind.

Warum das hier steht und nicht später auffällt:

* Die Wartezeit **wächst mit der Kernzahl**. Auf einem Kern ist der Fehler
  unsichtbar, auf 64 Kernen bestimmt er die Latenzspitzen des ganzen Systems.
* Auf x86-64 trifft es besonders hart, weil der **TLB-Shootdown** über genau
  diese IPIs läuft. Prozessende und Seitenrückgewinnung stapeln mehrere Runden.
* Es ist **kein Fehler, den man später wegoptimiert** — es ist eine Annahme, die
  sich durch jeden Aufrufer zieht, sobald sie einmal drinsteckt.
* Der Preis ist bekannt und wird **bewusst gezahlt**: etwas mehr Speicher je Task.

**Beleg, warum das kein theoretisches Bedenken ist:** Linux hatte genau diese
Sperre in `smp_call_function*()` — über die gesamte Operation, das Warten
eingeschlossen. Der Umbau kam erst mit **Linux 7.3** (Merge-Fenster 17.08.2026,
Pull Request von Thomas Gleixner, Code von Bytedance) und senkte die Latenz
hochpriorer Tasks in Bytedances laufender Serverflotte von **rund 17 ms auf
etwa 1,5 ms**, die P99-Latenz unter DPDK um **90 %**. Einordnung: die Zahlen
stammen von den Entwicklern selbst, unabhängige Nachmessungen liegen nicht vor,
und es geht um **Ausreißer, nicht um Durchsatz**. Für die Regel reicht das —
sie kostet uns jetzt nichts und später sehr viel.

Dieselbe Denkweise steht schon in `kernel/firn/serial.fi`: die serielle Ausgabe
ist bewusst **ohne** Sperre, damit der Panic-Handler nicht genau dann verklemmt,
wenn ein Interrupt mitten in einer Ausgabe zugeschlagen hat.

## Phase 7 — Netzwerk ○

* ○ NIC-Treiber (virtio-net zuerst, dann e1000/Intel)
* ○ Eigener TCP/IP-Stack: Ethernet, ARP, IPv4/IPv6, UDP, TCP, DHCP, DNS
* ○ Sockets als `Channel`-Objekte der nativen ABI — BSD-Sockets nur in der POSIX-Schicht
* ○ TLS im Userspace, nicht im Kernel

## Phase 8 — Reife ○

* ○ Speicherrückgewinnung: Bootloader-Bereiche, Slab-Allocator über der Bitmap, DMA-taugliche zusammenhängende Anforderungen
* ○ Auslagerung/Demand-Paging, Copy-on-Write für `MemoryMap`
* ○ Benutzerverwaltung rein über Capabilities (keine uid/gid im Kern)
* ○ Paketformat und Aktualisierung mit atomarem Zurückrollen
* ○ Werkzeugkette: Karstos-Target für rustc/LLVM, Portierung von coreutils-Ersatz
* ○ Selbst-Hosting: Karstos baut Karstos

---

## Phase 9 — aarch64: Karstos auf ARM ○

Ab hier wird aus einem Kernel ein **Ökosystem**. Ziel ist ausdrücklich nicht
„läuft auch auf einem Raspberry Pi", sondern Karstos als eigenständige
Alternative über alle Geräteklassen: Desktop und Server auf x86_64, **mobil auf
aarch64**. Deshalb ist die `arch/`-Grenze in diesem Projekt geschäftskritisch
und nicht Kosmetik — sie ist der Grund, warum Phase 9 überhaupt möglich ist,
ohne den Kern umzubauen.

### 9.1 Der leichte Teil — der Kernel selbst

* ○ `arch/aarch64/` anlegen und die vier Traits aus `kcore/arch_iface.rs`
  implementieren: `ArchOps`, `AddressSpaceOps`, `TimerOps`, `InterruptCtlOps`,
  dazu `ContextOps`
* ○ Boot über Limine (unterstützt aarch64) oder direkt per UEFI-Stub
* ○ Ausnahmevektoren (`VBAR_EL1`), EL1/EL0 statt Ring 0/3, `SPSR`/`ELR`
* ○ MMU: TTBR0/TTBR1 statt CR3 — die Zweiteilung in „unten Nutzer, oben Kernel"
  ist in Hardware, das passt besser zum bestehenden Entwurf als bei x86
* ○ GICv3 statt PIC/APIC, Generic Timer statt PIT
* ○ `switch_context` in aarch64-Assembler (x19–x30, SP, Gleitkommaregister)
* ○ **Device Tree** statt ACPI: kein BIOS, das die Hardware beschreibt. Ein
  DT-Parser ist Pflicht, kein Zusatz

Diese Liste ist überschaubar, und genau das ist der Punkt: Sie berührt **kein
Modul außerhalb von `arch/`**. Der Nachweis dafür läuft heute schon in jedem
Testlauf (`test.sh` Schritt 15).

### 9.2 Der harte Teil — die Gerätewirklichkeit auf ARM-SoCs

Ehrlich, ohne Beschönigung: Der Kernel ist das kleinste Problem.

| Brocken | Warum er schwer ist |
|---|---|
| **GPU / Display** | Auf x86 gibt es einen Framebuffer von der Firmware. Auf Mobil-SoCs gibt es Display-Controller (DPU/MDSS), MIPI-DSI-Panels mit gerätespezifischen Initialisierungssequenzen und Grafikkerne (Adreno, Mali), deren Treiber teils reverse-engineert sind. Ohne Display kein Telefon |
| **Touch-Eingabe** | Kapazitive Controller über I²C, jeder mit eigenem Protokoll, oft mit Firmware-Blob. Dazu Gestenerkennung, Multitouch, Palm Rejection — Dinge, die auf dem Desktop niemand braucht |
| **Modem / Baseband** | Der schwerste Punkt. Das Modem ist ein eigener Prozessor mit proprietärer Firmware, angebunden über QMI/RPMSG o. ä. Telefonie und Mobilfunkdaten sind ohne Herstellerunterlagen praktisch nicht sauber zu bauen. Realistische Zwischenstufe: Karstos als **Tablet-/WLAN-Gerät**, Telefonie später oder gar nicht |
| **Energieverwaltung** | Auf Mobilgeräten entscheidet sie über Brauchbarkeit. Regler, Takt- und Spannungsstufen (DVFS), Schlafzustände, Aufweckquellen, Akkuladung (PMIC), Temperaturdrosselung. Ein System, das den Akku in drei Stunden leert, ist kein Ersatz für Android |
| **Treiberrealität** | Es gibt keine PCI-Aufzählung. Jedes Gerät hängt an I²C/SPI/MMIO und muss aus dem Device Tree bekannt sein. Jeder SoC ist eine eigene Portierung — „ARM" ist keine Plattform, sondern eine Befehlssatzfamilie |
| **Bootketten** | Verriegelte Bootloader, signierte Abbilder, herstellerspezifische Partitionslayouts. Ohne entsperrbares Gerät kein Test |
| **Ökosystem** | Ein Telefon ohne Anwendungen ist ein Briefbeschwerer. Das Verteilmodell aus [PACKAGING.md](PACKAGING.md) muss auf Mobil genauso tragen — capability-basiert, damit Apps nicht nach Belieben auf Nutzerdaten kommen |

### 9.3 Ehrliche Reihenfolge bis dorthin

1. **QEMU `virt`** (aarch64) — reine Portierungsarbeit, kein Hardwarerisiko.
   Ziel: dasselbe Boot-Log wie auf x86_64, dieselben Selbsttests grün
2. **Ein Entwicklerboard** mit offener Dokumentation und offenem Bootloader
   (Raspberry Pi oder Rockchip). Ziel: serielle Konsole, Timer, MMU, Speicher
3. **Grafische Ausgabe** auf diesem Board — der erste echte Treiberbrocken
4. **Ein Tablet oder Telefon mit entsperrbarem Bootloader**, Display + Touch.
   Ab hier ist es ein Gerät, das man in die Hand nehmen kann
5. **WLAN und Energieverwaltung** — ab hier alltagstauglich als Tablet
6. **Modem** — die offene Frage. Vielleicht nie, und das wäre ein akzeptables
   Ergebnis

Diese Phase liegt Jahre entfernt und hängt an Phase 3 bis 8. Sie steht hier,
weil sie **jede Entscheidung davor prägt**: Deshalb gibt es die arch-Traits,
deshalb steht kein Register außerhalb von `arch/`, deshalb ist die POSIX-Schicht
abtrennbar. Wer erst beim Portieren anfängt zu abstrahieren, portiert nicht.

---

## Laufende Regeln (gelten in jeder Phase)

1. **Kein Linux-Code, kein Fork.** Nachbauen aus Spezifikationen ist erlaubt,
   Übernehmen nicht.
2. **Jede neue externe Crate braucht eine Begründung in ARCHITECTURE.md.** Im
   Zweifel selbst schreiben.
3. **Keine x86-Details außerhalb von `arch/`.** Die `grep`-Probe aus
   ARCHITECTURE.md § 2 gehört in die CI.
4. **POSIX bleibt abtrennbar.** `./run-qemu.sh --check --no-posix` muss in jeder
   Phase grün bleiben.
5. **Was ohne Hardware auskommt, wird host-getestet.** Erst wenn `cargo test`
   grün ist, darf es in den Kernel.
6. **Kein `todo!()` in einem Pfad, den der Boot durchläuft.** Nicht Gebautes
   meldet `NotSupported`.
7. **Kein Produktname im Quelltext.** Namen kommen aus
   `kcore/branding.rs`; `./test.sh` Schritt 14 prüft das. Umbenennen geht mit
   `./rename.sh` — siehe [RENAME.md](RENAME.md).
8. **Jede Stelle, an der Rust im Weg steht, wird protokolliert** —
   [LANGUAGE.md](LANGUAGE.md). Kein Eintrag ohne Datei und Zeile.

## Verwandte Entwurfsdokumente

* [PACKAGING.md](PACKAGING.md) — unveränderliche, content-addressed Apps,
  drei getrennte Datentöpfe, Systemgenerationen. Setzt den capability-basierten
  Kern voraus und begründet, warum
* [FILESYSTEM.md](FILESYSTEM.md) — Reihenfolge VFS → FAT32 → ext4 (lesend) →
  eigenes Dateisystem, mit den Anforderungen an letzteres
* [LANGUAGE.md](LANGUAGE.md) — Logbuch für die spätere eigene Systemsprache
* [RENAME.md](RENAME.md) — Kernel und OS in unter 15 Minuten umbenennen

## Nächster konkreter Schritt

Erledigt ist inzwischen der Sprung vom Boot-Kern zum System mit echtem
unprivilegiertem Code: **Verdrängung** (30 erzwungene Wechsel ohne `yield`,
Prioritäten messbar), **Ring 3** (`CPL=3`, Systemaufruf, Negativtest auf eine
Kerneladresse), **ELF64 aus einem Startdateisystem** (16 ELF- + 10 Archiv-Negativfälle) und
die **capability-basierte ABI** (Handle-Tabelle je Prozess, 3 + 12 Negativfälle,
dazu Port-, Übergabe- und Namensraum-Negativfälle).
Dazu kommt seit dieser Runde die **Verdrängung von Ring 3 mit Fortsetzung**
(12 Unterbrechungen, Programm endet trotzdem mit Code 0, Zählthread kommt
parallel auf 12 Mio. Durchläufe). Alles im Boot-Log belegt, `./test.sh` läuft
mit 21 Schritten grün.

Ehrlich offen geblieben — und deshalb als Nächstes dran:

1. **Adressraum je Prozess.** Der unprivilegierte Code läuft heute im
   Kerneladressraum auf Seiten mit Benutzerbit. Solange das so ist, sind zwei
   unprivilegierte Prozesse nicht voneinander isoliert. Das ist die wichtigste
   offene Lücke dieser Phase und wird nicht schöngeredet.
2. **Zwei Ring-3-Programme gleichzeitig.** Ein einzelnes Programm wird
   inzwischen wirklich verdrängt und fortgesetzt (Rahmen auf dem Kernelstapel
   seines Threads). Was noch fehlt: **mehrere** unprivilegierte Programme
   nebeneinander. Dafür müssen der gemerkte Kernelstapelzeiger (`SAVED_RSP`),
   der Zustand „läuft unprivilegiert" und `TSS.rsp0` je Thread geführt werden
   statt einmal pro CPU — heute lässt der Kernel deshalb bewusst nur ein
   unprivilegiertes Programm zur Zeit zu. Braucht Punkt 1 für echte Isolation.
3. **Ein `init`, das bleibt.** Bisher endet der unprivilegierte Teil als
   Selbsttest im Bootweg; danach läuft `kmain` weiter. Ein Prozess, der die
   Kontrolle behält, braucht Punkt 1 und einen Weg, Handles nachzureichen.
4. **APIC statt PIC** (LAPIC-Timer, IOAPIC): der Leerlauf-Thread steht
   inzwischen, die Warteschleife ist damit erledigt.
