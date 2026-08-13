# Roadmap von Karstos

Ziel: ein eigenständiges, langfristig linuxfähiges Betriebssystem — kein Fork,
kein übernommener Code. Zeithorizont: Jahre. Reihenfolge ist nach Abhängigkeit
sortiert, nicht nach Attraktivität.

Legende: **✔ fertig** · **▶ als Nächstes** · **○ geplant**

---

## Phase 1 — Bootfähiger Kern ✔ *(abgeschlossen)*

* ✔ Build-System: eigenes Target `x86_64-karst-none`, `build.sh`, `run-qemu.sh`, `test.sh`
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

## Phase 2 — Scheduler und Zeit ▶

* ✔ Kontextwechsel in Assembler (`arch/x86_64/context.rs`, `naked_asm!`),
  Kernel-Threads mit eigenem Heap-Stapel und Wachzone
* ✔ Kooperativer Round-Robin-Scheduler hinter dem `Scheduler`-Trait in
  `kcore/sched.rs`; `run()` kehrt nach `kmain` zurück, wenn niemand mehr
  lauffähig ist, und meldet eine Verklemmung statt hängen zu bleiben
* ✔ `yield_now`, `block_current`/`unblock`, `sleep_ticks` — Wecken durch den
  Zeitgeber (der Scheduler wartet derweil mit `wait_for_interrupt`); im Boot-Log
  als drei Selbsttests nachweisbar (Wechselspiel, Schlafen/Wecken, Verklemmung)
* ▶ Idle-Thread statt Warteschleife im Scheduler, Präemption durch den Zeitgeber
* ○ APIC statt PIC: LAPIC-Timer, IOAPIC-Umleitungen, TSC-Deadline
* ○ Monotone Uhr in Nanosekunden (`ClockRead`), Kalibrierung gegen HPET/PIT

## Phase 3 — Userspace und Syscalls ○

* ○ `syscall`/`sysret` verdrahten (`STAR`/`LSTAR`/`SFMASK`), Per-CPU-Kernelstapel via `swapgs`
* ○ Adressräume je Prozess, Kernelhälfte geteilt, User-Mappings mit `USER`-Bit
* ○ ELF-Lader für statische Binaries
* ○ `karst-native` implementieren: Handle-Tabelle, `MemoryCreate/Map`, `ChannelCreate/Send/Recv`, `PortCreate/Wait/Bind`, `ProcessSpawn`, `ThreadCreate`
* ○ Erstes Userspace-Programm: `init`, das über einen Channel auf die Konsole schreibt
* ○ POSIX-Schicht real anbinden (`sys_read`/`write`/`open`/`close` auf echte Handles)

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

Erledigt ist inzwischen der **kooperative** Teil von Phase 2:
`arch/x86_64/context.rs` (`switch_context` in `naked_asm!`), `kcore/sched.rs`
mit dem `Scheduler`-Trait und ein Boot-Selbsttest, der zwei Kernel-Threads
abwechselnd ins Log schreiben lässt und sauber nach `kmain` zurückkehrt.

Als Nächstes ansetzen:

1. **Präemption.** Der Timer-Interrupt ruft den Scheduler auf, statt nur zu
   zählen. Dazu muss der Interrupt-Handler den vollen Registersatz sichern
   (bisher rettet `switch_context` nur die vom SysV-ABI verlangten
   Callee-saved-Register — das genügt für freiwillige Wechsel, nicht für
   erzwungene). Nachweis: ein Thread ohne `yield` wird trotzdem verdrängt.
2. **Zeitscheiben und Prioritäten** hinter demselben `Scheduler`-Trait, damit
   der kooperative Planer als Vergleichsmaß erhalten bleibt.
3. **Danach erst Ring 3**: `syscall`/`sysret` verdrahten (`STAR`/`LSTAR`/
   `SFMASK`), Per-CPU-Kernelstapel über `swapgs`, ELF-Lader, erstes
   Userspace-`init` über einen Channel.
