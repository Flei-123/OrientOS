# OrientOS — Betriebssystem von Grund auf

**OS: `OrientOS` · Kernel: `osum` (Firn, eigenes Repo) · Produkt: `build/orientos.iso`, bootet ueber BIOS und UEFI**

Kein Linux-Fork, kein uebernommener Fremdcode. Der Kernel ist in
[Firn](LANGUAGE.md) geschrieben, einer eigenen Sprache; OrientOS ist das
System darum herum.

## Was es kann

**Starten.** Multiboot ueber Limine, auf BIOS **und** UEFI, aus demselben
Abbild.

**Prozesse.** Jeder Prozess hat einen **eigenen Adressraum**; `fork`,
`execve`, `wait4`. Der Scheduler verdraengt nach Zeitscheibe und laeuft
auf **mehreren Prozessoren** (SMP ueber ACPI/MADT, APIC, Spinlocks).

**Hardware.** PCI-Erkennung, APIC und IOAPIC, **NVMe ueber DMA** (gemessen
140.799 KiB/s gegen 5.910 KiB/s bei ATA PIO), PS/2-Tastatur, serielle
Schnittstelle, **Rahmenpuffer** 800x600x32 mit Textkonsole 100x37,
**virtio-net**.

**Dateien.** Ein eigenes Dateisystem, **OFS** — Superblock, Blockbitmap,
Inodes, Verzeichnisse, direkte und indirekte Bloecke, `getdents64`.
Schreibend, auf RAM-Platte, ATA oder NVMe.

**Programme.** ELF64-Lader: der Kernel liest `/bin/sh` **von der Platte**
und startet es. Darauf eine **POSIX-Schicht** mit den Systemaufrufnummern
von Linux (72 Aufrufe), eine libc in Firn, eine **Shell** mit Roehren,
Umlenkung, Exit-Code und Auftragssteuerung, und **31 Werkzeuge**
(`ls -l`, `cat`, `cp`, `mv`, `rm`, `grep`, `sort`, `wc`, `ps`, `kill`,
`ping`, `wget` ...).

**Was Unix-Programme voraussetzen.** Signale mit eigenen
Behandlungsroutinen in Ring 3 (`sigaction`, `sigreturn`), Terminals und
Pseudoterminals mit Zeilendisziplin und Prozessgruppen (Strg-C beendet
wirklich), Echtzeituhr und monotone Uhr, `getrandom`.

**Netz.** Ein eigener TCP/IP-Stack in Firn — Ethernet, ARP, IPv4, ICMP,
UDP, TCP mit Zustandsautomat — unter virtio-net, mit Steckdosen in
Ring 3. Gemessen gegen den echten Linux-Kern: 6432 KiB/s ueber saubere
Leitung, und bei 20 % kuenstlichem Paketverlust uebertraegt er weiter
korrekt.

**Der Kernel schuetzt sich vor dem Userland.** **SMEP und SMAP** stehen in
CR4, sobald der Prozessor sie meldet — auf **jedem** Kern, denn CR4 ist
pro Prozessor. Ring 0 fuehrt damit keinen Nutzercode aus, und Nutzerdaten
fasst er nur im `stac`-Fenster an. Gemessen wird nicht die Behauptung,
sondern das zurueckgelesene Register: `guard: cr4=0x300020 smep=1 smap=1`.

**Ein ISO mit Userland.** Der Lader legt ein OFS-Dateisystem als
**Boot-Modul** neben den Kern; der Kernel rechnet dessen CRC32 nach und
mountet es als Wurzelplatte. Deswegen laeuft `/bin/sh` in einem ISO, das
gar keine Platte hat. Was darin liegt, stellt **dieses** Repo zusammen
(`userland/PROGRAMME`).

**Rechte.** Eine **capability-basierte ABI**: jeder Zugriff laeuft ueber
ein Handle mit Rechten — Slot und Generation, zehn Rechtebits, acht
Objektarten. Kein Erben, keine Umgebungsautoritaet.

**Eine Oberflaeche.** Ein Zeigegeraet am zweiten Anschluss des
Tastaturbausteins, ein **Fensterserver** mit Stapelreihenfolge,
Eingabefokus und Bereichsverfolgung, und ein **TrueType-Leser samt
Rasterer mit Kantenglaettung**, ganz in Firn. Auf dem Schirm stehen zwei
Fenster: ein Terminal, in dem `/bin/sh` von der Platte laeuft, und ein
zweites, das sich anklicken, verschieben und schliessen laesst. Gemessen
an echten Bildschirmfotos, in die ueber den QEMU-Monitor echte
Mausbewegungen, Klicks und Tastendruecke eingespeist wurden — und der
Text darin nicht gegen eine Flaeche, sondern **je Zeichen** gegen eine
zweite, unabhaengige Rasterung desselben Umrisses.

**Man kann darauf arbeiten.** Ein bildschirmorientierter **Editor**
(`/bin/edit`) in Ring 3 ueber den rohen Terminalmodus, und der
Werkzeugkasten dazu: `find`, `sed`, `diff`, `patch`, `tar`, `gzip`,
`gunzip`, `xargs`, `du`, `top`, `mount`, `umount`, `tee`, `cut`, `tr`,
`seq`, `env`, `which`, `basename`, `dirname`. Die Shell ist eine Sprache
geworden — `if`/`elif`/`else`, `while`, `until`, `for`, `case`,
Funktionen, `return`/`break`/`continue`/`shift`, `test` und `[`. Jedes
Werkzeug steht gegen sein GNU-Gegenstueck; `tar` und `gzip` arbeiten in
**beide Richtungen** mit dem echten `tar` und `gzip` des Wirts zusammen.

**Ein Wirt fuer fremde Prozessoren.** Ein **Hypervisor** in Firn:
Steuerblock, Gastzustand, Abfangbits, Eintritt, Austritt, Austrittsgrund,
**verschachtelte Seitentabellen**. Gastmaschinen sind aus Ring 3
erreichbar — ueber Handles mit Rechten, nicht ueber kleine Zahlen: wer
eine Kopie ohne `R_MANAGE` weitergibt, gibt das Recht zu laufen weiter
und das Recht zu toeten nicht. Umgesetzt ist **AMD-V**, nicht Intel VT-x,
und der Grund steht in `docs/ROUNDK12.md`: die Messmaschine ist ein AMD
EPYC ohne `/dev/kvm`, und QEMUs TCG bietet `vmx` nachweislich nicht an —
ein VMX-Wirt waere hier nicht messbar gewesen, und dieses Projekt misst.

**Marken.** Ein Quelltext, mehrere Produkte: `brands/*.toml` beschreibt
alles Austauschbare, der Quelltext kennt keinen dieser Werte
(`./build.sh --brand <name>`).

## Was es nicht kann

- **Keinen Schreibtisch.** Es gibt Fenster, eine Maus und Schriften, aber
  keine Anwendungen darin ausser dem Terminal und einem Testfenster:
  kein Dateimanager, keine Themen, keine Hintergrundbilder, keine
  Einstellungen.
- **Kein ext4.** FAT32 liest und schreibt der Kernel seit K14, ext4 nicht:
  im Kernelbaum steht dazu keine Zeile.
- **`firnc` liegt nicht im Produkt.** Osum uebersetzt seit K16 auf sich
  selbst, aber `vendor/osum/hole-osum.sh` baut nur die Programme aus
  `kernel/user/*.fi` — darunter `fas` und `firun`, nicht `firnc`. Auf
  einem gebooteten OrientOS laesst sich heute ein `.s` assemblieren, aber
  keine `.fi` uebersetzen.
- **Die Paketverwaltung laeuft auf dem Wirt.** Format, Store,
  Generationen und eine signierte Quelle stehen ([PAKETE.md](PAKETE.md));
  das gebootete System liest den Store und startet Pakete, kann aber
  nicht installieren, entfernen oder zurueckrollen.
- **Kein Auslagern**, kein `mmap` auf Dateien, **kein USB**, **kein Ton**,
  **keine Energieverwaltung** (ACPI kann bisher nur abschalten: keine
  P-States, keine Profile, kein Akkustand, keine Helligkeit, kein
  Standby).
- **Keine Gastsysteme von der Stange.** Der Hypervisor laesst eigene,
  kleine Gaeste laufen und misst sie; ein unveraendertes Linux oder gar
  Windows hat er noch nie gebootet, und die virtuellen Geraete dafuer
  (Platte, Netz, Bildschirm) fehlen.
- **Nur x86-64.** Firn selbst uebersetzt auch nach aarch64, der Kernel
  nicht.
- **Keine fremden Programme.** Es laeuft, was in Firn geschrieben und fuer
  Osum uebersetzt wurde. Linux-Binaerdateien laufen nicht; der Weg dahin
  waere die Umsetzung der Linux-Systemaufrufnummern auf die eigenen.

## Kennzahlen

| | |
|---|---|
| Kernel (Osum, festgenagelter Commit) | 34.459 Zeilen Firn + 1.884 Zeilen Assembler |
| Rust im Kernel | **0** |
| Rust in DIESEM Repo | **0** (+ 378 Zeilen Vorlage, nicht uebersetzt) |
| C in diesem Repo (ausser dem Bootlader) | **0** |
| Externe Bibliotheken im Kernel | **0** |
| Systemaufrufe | 79 deklariert, 74 im Verteiler |
| Programme in Ring 3 | **52** (kernel/user/*.fi mit eigenem `_start`; dazu zwei Bibliotheken ohne) |
| Zusagen der Kernel-Abnahme (Osum `./test.sh`) | **1486**, in 18 Abschnitten, 0 Fehler |
| Zusagen der System-Abnahme (hier, `./test.sh`) | **151**, in 11 Schritten, jede mit Gegenprobe |

Alles nachpruefbar: `./test.sh` baut, bootet und misst. Jede Zusage hat
eine **Gegenprobe** — denselben Kernel mit abgeschalteter Eigenschaft, wo
die Messung zusammenbrechen muss. Ein gruener Test ohne Gegenprobe zaehlt
in diesem Projekt nicht.

## Schnellstart

```sh
# 1. Voraussetzungen (Debian/Ubuntu)
sudo apt install qemu-system-x86 xorriso mtools ovmf python3 binutils build-essential git
#    ...dazu rustc/cargo: Osums Firn-Übersetzer (Stufe 0) ist in Rust geschrieben.
#    In DIESEM Repo wird kein Rust übersetzt.

# 2. Das Produkt bauen und starten
./build.sh          # Kernel aus vendor/osum + Userland-Modul -> build/orientos.iso
./run-osum.sh       # startet es über SeaBIOS, serielle Ausgabe im Terminal
./run-osum.sh --uefi                     # dasselbe Abbild über OVMF
./run-osum.sh --script 'ls /bin;exit'    # ein Shell-Skript hineinreichen

# 3. Alles prüfen (echte QEMU-Boots, BIOS und UEFI, Shell in Ring 3)
./test.sh
```

### Skripte

| Befehl | Wirkung |
|---|---|
| `./build.sh` | **Produkt-ISO** (`build/orientos.iso`): holt den festgenagelten Osum-Kernel **und** seine Programme, stellt aus `userland/PROGRAMME` ein OFS-Dateisystem zusammen, rechnet dessen CRC32 und packt beides mit Limine (Multiboot, BIOS + UEFI) |
| `./build.sh --brand xoffi` | zweite Marke, gleicher Quelltext ([BRANDING.md](BRANDING.md)) |
| `./build.sh --cmdline "…"` | eigene Kommandozeile für den Kernel |
| `./build.sh --ohne-userland` | **Gegenprobe:** ISO ohne das Boot-Modul |
| `./build.sh --kaputte-summe` | **Gegenprobe:** falsche CRC32 im Aufruf — der Kernel muss das Modul liegen lassen |
| `./run-osum.sh [--uefi] [--script "…"]` | startet das Produkt-ISO; Rückgabe 0, wenn der Kernel sich sauber mit 21 beendet |
| `./run-osum.sh --cpu qemu64` | **Gegenprobe:** ein Prozessor ohne SMEP/SMAP |
| `./test.sh` | die Abnahme des Gesamtsystems |
| `./rename.sh <kernel> <os>` | benennt Kernel und OS im ganzen Baum um ([RENAME.md](RENAME.md)) |
| `./vendor/osum/hole-osum.sh` | baut Kernel + Programme aus `vendor/osum/COMMIT` |

---

## Was das Produkt wirklich tut

Echter Boot vom 26.08.2026, **unverändert** aus `./run-osum.sh --script
'ls /bin;uname;df;exit'` kopiert; nur die `elf:`-Zeilen des Laders sind
weggelassen, weil es je gestartetem Programm vier sind.

```text
firn kernel r62, profile kernel
mb: flags=0x126d  cmd=osum nokbd nosched noproc nofs noring3 modfs modcrc=5d92f65d script=ls /bin;uname;df;exit
idt: 48 gates, vector 8 IST
pic: master 0x20, slave 0x28
pit: 100 Hz, divisor 11931
kernel end: 0x225040
mmap: 9 entries, 523771 KiB usable, top=0x1ffdf000
frames: 131072 covered, 129721 free
heap: 0x552000  size=262144
frame test: ok
heap test: ok
mod: base=0x252000  bytes=3145728  blocks=6144  crc=0x5d92f65d  want=0x5d92f65d  ok=1
pci: 00:00.0 8086:29c0 class=06:00:00 host-bridge
pci: 00:01.0 1234:1111 class=03:00:00 vga  bar0=0xfd000000/0x1000000
pci: 00:02.0 8086:10d3 class=02:00:00 network  bar0=0xfeb80000/0x20000  irq=11  msi  msix
pci: 00:1f.2 8086:2922 class=01:06:01 ahci  bar5=0xfebd5000/0x1000  irq=10  msi
pci: devices=6
apic: id=0  hz=62537300  ioapic=24  tick=100  window=2
guard: cr4=0x300020  smep=1  smap=1  cpu=1/1
ticks: 20  traps=20
osum: from module 0x252000  blocks=6144
osum: mount=1
osum: bin .:2 ..:2 sh:1 ls:1 cat:1 cp:1 mv:1 rm:1 mkdir:1 rmdir:1 touch:1 head:1
      tail:1 wc:1 grep:1 sort:1 uniq:1 true:1 false:1 sleep:1 ps:1 kill:1 uname:1
      date:1 df:1 hello:1 ping:1 wget:1
sh: ready, osum
osum$ ls /bin
./ ../ sh ls cat cp mv rm mkdir rmdir touch head tail wc grep sort uniq true false
sleep ps kill uname date df hello ping wget
osum$ uname
osum
osum$ df
blocks total=6144 free=2442 used=3702 size=512
inodes total=128 used=31
osum$ exit
sh: bye
osum: kstack deepest=13520 of 16384
osum: sh exit=0
osum: frames_free=129653 of 129653
osum: syscalls=144 forks=0 execves=0 pipes=0 maps=0 opens=1
smp: online=1 of 1  failed=0
guard: aps=0  windows=253
mod: recheck crc=0x5d92f65d  same=1
kernel: done
--- Beendigungscode 21 ---
```

**Die vier Zeilen, auf die es ankommt:**

* `mod: … crc=0x5d92f65d want=0x5d92f65d ok=1` — der Kernel hat das
  Boot-Modul gefunden und **selbst nachgerechnet**, dass es heil
  ankam. Der Wirt hat dieselbe Summe in die Kommandozeile geschrieben.
* `osum: from module … blocks=6144` — die **Wurzelplatte ist das Modul**.
  Ein ISO hat keine Platte; was ein Multiboot-Lader weiterreichen kann,
  ist ein Modul. Deswegen hat dieses Produkt überhaupt ein Userland.
* `guard: cr4=0x300020 smep=1 smap=1` — Bit 20 und Bit 21 in CR4, aus dem
  Register **zurückgelesen**. Ring 0 führt keinen Nutzercode aus und
  fasst Nutzerdaten nur im `stac`-Fenster an (`windows=253` sagt, wie oft
  es aufging).
* `mod: recheck … same=1` — nach dem ganzen Lauf ist das Modul noch
  dasselbe. Hätte der Rahmenverwalter seinen Speicher hergegeben, stünde
  hier eine andere Summe.

---

## Wie die beiden Repos zusammenhängen

```
osum (eigenes Repo, Firn)                orientos (dieses Repo)
├── kernel/*.fi   der Kern       ──┐     ├── vendor/osum/COMMIT   ← festgenagelt
├── kernel/user/  Shell, 25 Werkz. │     ├── brands/*.toml        Marken
├── lib/libc/     libc in Firn     └──►  ├── userland/PROGRAMME   was ins ISO kommt
├── vendor/firn/COMMIT (eigener          ├── userland/dateien/    was OrientOS beisteuert
│                       Übersetzer)      ├── PACKAGING.md         Entwurf: Pakete
├── assets/apps/  .osp-Bündel (K15)      ├── PAKETE.md            gebautes Format
                                         ├── pkg/opk.py          die Paketverwaltung
└── test.sh       15 Abschnitte,         ├── ASSISTENT.md         Schnittstelle
                  >1100 Zusagen          ├── build.sh             Kernel + Userland → ISO
                                         └── test.sh              Abnahme des Systems
```

**Eingecheckt ist nur der Hash.** Kein Kernelabbild, kein Programm, kein
`mkfs.py` — `vendor/osum/hole-osum.sh` baut alles aus dem genannten
Commit, und `test.sh` prüft nach, dass im Repo wirklich nichts Gebautes
liegt. Die beiden Repos nageln **unterschiedliche** Firn-Commits fest,
und das soll so bleiben: zwei Projekte, zwei Nägel, keine stille
Vermischung.

---

## Ein Quellbaum, zwei Produkte

`brands/*.toml` beschreibt alles, was am Produkt austauschbar ist;
`brands/STANDARD` nennt die Standardmarke. Der Kernel heißt in **jeder**
Marke `osum` — so wie NT in jeder Windows-Ausgabe NT heißt.

```sh
./build.sh                 # build/orientos.iso, Bootmenü "/OrientOS"
./build.sh --brand xoffi   # build/xoffi.iso,    Bootmenü "/XoffiOS"
```

`test.sh` baut die zweite Marke wirklich und liest nach, dass ihr
Bootmenü den anderen Namen trägt — und dass eine **erfundene** Marke
abbricht, statt still auf die Standardmarke zurückzufallen. Einzelheiten:
[BRANDING.md](BRANDING.md).

---

## Was noch fehlt

Ehrlich und vollständig in [KERNELWECHSEL.md § 4](KERNELWECHSEL.md) und
[ROADMAP.md](ROADMAP.md). Die Kurzfassung:

* **Keine Architekturgrenze.** Osum ist x86-64-only, obwohl Firn auch
  aarch64 kann. Der Rust-Kernel hatte diese Grenze als Traits samt einem
  Testschritt, der sie erzwang; das ist **nicht** portiert, weil es eine
  Umbauarbeit an jedem Modul wäre. Die Vorlage steht als
  `vorlage/arch_iface.rs` im Baum — nicht gebaut, mit Begründung im Kopf.
* **Kanäle, Ports, Namensräume, `ProcessSpawn`, Speicherobjekte** der
  nativen ABI antworten `NotSupported` (−9). Portiert ist das
  Handle-Modell darunter, nicht die Objekte daran.
* **Vom Verteilmodell** ist der Wirtsteil gebaut, der Systemteil nicht:
  Store, Generationen, Zurueckrollen und eine signierte Quelle laufen als
  `pkg/opk.py` auf dem Wirt ([PAKETE.md](PAKETE.md)), und das ISO traegt
  das Ergebnis (`/store`, `/apps`, `/system`). Ein `/bin/opk` in Firn
  gibt es nicht.
* **Das Modul ist eine RAM-Platte.** Änderungen überleben den Lauf nicht.
  Für ein System, das sich installiert, fehlen ein Schreibpfad auf eine
  echte Platte und ein Partitionstabellen-Leser.
* Keine Namensauflösung, kein USB, kein SATA/AHCI, kein Journal, keine
  dynamische Bindung. (Fenstersystem, Benutzer und Rechte hat der Kernel
  seit K10 bzw. K13 — diese Aufzählung stand bis zum 26.08.2026 auf dem
  Stand von `7a53ac3`, siehe [ROADMAP.md](ROADMAP.md) § 10.)
* **Getestet wird in QEMU**, nicht auf echter Hardware.

---

## Doku

| Datei | Inhalt |
|---|---|
| [KERNELWECHSEL.md](KERNELWECHSEL.md) | **Was gilt.** Der Abgleich Modul für Modul, was portiert wurde, was offen ist, was gelöscht wurde |
| [ARCHITECTURE.md](ARCHITECTURE.md) | die Architektur des Systems: wer was macht, und wo die Grenze liegt |
| [ROADMAP.md](ROADMAP.md) | wohin es geht, nach Abhängigkeit sortiert |
| [LANGUAGE.md](LANGUAGE.md) | das Logbuch der Reibungspunkte zwischen Rust und einem Kernel — Geschichte, und weiterhin Anforderungen an Firn |
| [BRANDING.md](BRANDING.md) · [RENAME.md](RENAME.md) · [NAMEN.md](NAMEN.md) | Marken, Umbenennen, Namensfindung |
| [PACKAGING.md](PACKAGING.md) · [FILESYSTEM.md](FILESYSTEM.md) | Verteilmodell und Dateisystem — der **Entwurf** |
| [PAKETE.md](PAKETE.md) · [docs/RUNDE-PAKETE.md](docs/RUNDE-PAKETE.md) | das **gebaute** Paketformat, seine Abweichungen vom Entwurf, und die Zahlen |
| [ASSISTENT.md](ASSISTENT.md) | die Schnittstelle für einen Assistenten (statt Bildschirmfotos) |
| [NETZWERK.md](NETZWERK.md) | Umleitschicht, Kernel/Userspace-Grenze, Tor/WireGuard |
| [userland/README.md](userland/README.md) | wie das Userland ins ISO kommt und was daraus werden soll |
| [tests/README.md](tests/README.md) | wie die Abnahme aufgebaut ist |
