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

* ○ VFS als Namespace-Baum aus Handles (kein globaler Root — jeder Prozess bekommt sein Wurzel-Handle übergeben)
* ○ Blockgeräte-Trait, Puffercache
* ○ Treiber: AHCI/SATA, NVMe
* ○ Eigenes Dateisystem `karstfs`: Copy-on-Write, Prüfsummen, Journal; daneben FAT32 zum Datenaustausch
* ○ `initrd` als Speicher-Dateisystem für den frühen Start

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

## Nächster konkreter Schritt

`arch/x86_64/context.rs`: `switch_context(alt: *mut Context, neu: *const Context)`
in `naked_asm!`, dazu ein `Thread` mit eigenem Kernelstapel und ein Test, der
zwei Kernel-Threads abwechselnd ins Boot-Log schreiben lässt. Damit ist Phase 2
angefangen und sofort im Emulator nachweisbar.
