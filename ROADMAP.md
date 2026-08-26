# Roadmap von OrientOS

Ziel: ein eigenstaendiges Betriebssystem — kein Fork, kein uebernommener
Fremdcode, Kernel und System in [Firn](LANGUAGE.md). Zeithorizont: Jahre.
Sortiert nach Abhaengigkeit, nicht nach Attraktivitaet.

Legende: **fertig** · **teilweise** · **offen**

Was heute laeuft, steht in [README.md](README.md) unter „Was es kann" und
„Was es nicht kann". Diese Datei sagt, was noch zu tun ist.

**Stand der Zahlen: 26.08.2026, Osum `c5fe12f`.** Jede Zahl in dieser Datei
stammt aus einem Lauf, der wirklich gefahren wurde — die Kernelzahlen aus
den Laeufern der jeweiligen Runde (`tools/k13/run.sh`, `tools/k14/run.sh`,
`tools/k15/run.sh`, `tools/k16/run.sh` im Osum-Repo, protokolliert in
dessen `docs/ROUNDK*.md`), die Zeilenzahlen mit `wc -l` am Baum von
`c5fe12f` nachgezaehlt und nicht aus den Logbuechern uebernommen. Wo eine
Zahl aus einem Logbuch von der nachgezaehlten abweicht, steht die
nachgezaehlte da.

**Was diese Zahlen NICHT sind: eine Gesamtsumme.** Die vier Runden K13
bis K16 sind jede fuer sich gegen dieselben 1486 Zusagen von `main`
gemessen worden, in vier getrennten Laeufen. `./test.sh` im Osum-Repo am
Stueck ueber den verschmolzenen Stand ist damit **nicht** gefahren, und
eine addierte Summe waere deshalb eine Behauptung. Es stehen hier die
Zahlen je Runde.

---

## 1 — Das System benutzbar machen

Ohne diese Punkte ist Osum ein Vorfuehrsystem, kein Arbeitsgeraet.

| | Was | Stand |
|---|---|---|
| 1.1 | **Editor** — bildschirmorientiert, roher Terminalmodus, Suchen/Ersetzen, Rueckgaengig. Ohne ihn laesst sich auf dem System keine Datei aendern. | **fertig** (K11, `/bin/edit`) |
| 1.2 | **Werkzeugkasten** — `find`, `sed`, `diff`, `patch`, `tar`, `gzip`, `xargs`, `du`, `top`, `mount`, `cut`, `tr`, `tee` | **fertig** (K11, 20 Werkzeuge gegen ihre GNU-Gegenstuecke gemessen) |
| 1.3 | **Shell-Skripte** — `if`, `for`, `while`, `case`, Funktionen, Variablenersetzung, `test` | **fertig** (K11) |
| 1.4 | **Benutzer und Rechte** — `uid`/`gid`, `chmod`, `chown`, `setuid`, Anmelden, `passwd` | **fertig** (K13). Die Kennungen liegen im Aufgabensatz und werden in `sched.create` vererbt, also fuer `fork`, `execve`, `SYS_OSUM_SPAWN` und `proc.create` gleichermassen. Die Rechtepruefung ist **eine** Funktion (`kernel/perm.fi`, 285 Zeilen, `may_ids`), gerufen aus fuenf Toren — `open`, `mkdir`, `unlink`, `chdir`, `execve`; eine zweite Fassung der Regel gibt es im Baum nicht. OFS traegt `mode`, `uid`, `gid` im Inode. Werkzeuge in Ring 3: `id` (120 Z.), `whoami` (30), `chmod` (192, oktal **und** Buchstabenform), `chown` (104), `su` (114), `passwd` (115), `login` (123). 103 deklarierte Systemaufrufe (79 + 24). Gemessen: `tools/k13/run.sh` **99 Zusagen, 0 Fehler**; im Lauf 177 Rechtefragen, davon 2 abgelehnt, 6-mal das setuid-Bit gegriffen |
| 1.5 | **`init` und Dienste** — ein erster Prozess, der Dienste startet, ueberwacht, neu startet und beim Herunterfahren aufraeumt | **fertig** (K13). Der Kern startet `/sbin/init` als Prozess 1 statt der Shell (`kernel/kmain.fi`, Zeile 1819 ff.); `init` (474 Z.) liest `/etc/inittab`, startet Dienste, startet abgestuerzte neu, nimmt Waisen an und faehrt ueber echtes ACPI herunter. Dazu `svc` (137 Z.) zum Nachsehen und Steuern. Der Notweg ist ausdruecklich da und benannt: liegt kein `/sbin/init` auf der Platte, nimmt der Kern `/bin/sh` — sonst waere jedes Abbild aus den Runden bis K12 unbootbar geworden (`kernel/kstate.fi`, Wort `initsh`) |
| 1.6 | **VFS** — mehrere Dateisysteme nebeneinander einhaengen | **fertig** (K14). Ein Dateisystem meldet sich mit **neun** Verrichtungen an (`lookup readdir attr read write create unlink rename trunc`) plus einer Maske, welche davon es wirklich kann; `vfs.fi` prueft das Bit, **bevor** es den Zeiger benutzt. Das sind echte indirekte Aufrufe (`call *%rax`) — Firn Stufe 0 kann einen Funktionszeiger weder in eine Zahl wandeln noch aus einem Tafelfeld heraus aufrufen, ein Strukturfeld aufrufen aber schon. Dateien: `vfsops.fi` 138 Z. (die Form), `mnt.fi` 327 (Einhaengetafel, 8 Eintraege zu 128 Oktetten), `vfs.fi` 530 (der Namensraum), `ofs.fi` 178 (OFS als Treiber dieser Schicht), `procfs.fi` 1226 (`/proc`, im Speicher erzeugt), `devfs.fi` 486 (`/dev` als echtes Dateisystem). Damit ist auch der Sonderaufruf `SYS_OSUM_PSTAT` weg, ueber den `ps` und `top` bis dahin fragten. Gemessen: `tools/k14/run.sh` **152 Zusagen, 0 Fehler** |
| 1.7 | **FAT32** — damit Osum Platten liest, die ein anderes System beschrieben hat | **fertig, und mehr als verlangt** (K14): `kernel/fat.fi`, 2007 Zeilen, 93 Funktionen, **lesend UND schreibend**, mit langen Namen; gemessen gegen `mkfs.vfat`, `mcopy`, `mdir` und `fsck.fat`. Dazu `kernel/part.fi` (498 Z.): MBR und GPT mit **beiden** CRC32-Pruefsummen — vorher sah Osum in Block 0 nach einem OFS-Superblock und fand auf jeder fremden Platte eine Partitionstafel. Benannte Grenzen: nur 512 Oktette je Sektor (ein FAT16 wird abgelehnt), Namen bis 63 Zeichen, ein NEUER Name nur in US-ASCII, hoechstens 16 gleichzeitig offene Knoten, kein `fsync` |
| 1.7b | **ext4 (lesend)** — der zweite Teil des alten Punktes 1.7, hier ausdruecklich abgetrennt | **offen**. Im Kernelbaum von `c5fe12f` gibt es dazu **keine Zeile**: `grep -ril ext4 kernel/` findet nichts. Das ist kein Nebensatz von 1.7, sondern ein eigenes Dateisystem mit Extents, Journal und Merkmalsbits |

## 2 — Oberflaeche

| | Was | Stand |
|---|---|---|
| 2.1 | **Maus** (PS/2, spaeter USB-HID) | **fertig** (K10, PS/2 an IRQ 12) |
| 2.2 | **Fensterserver** — Fenster anlegen, verschieben, stapeln, Fokus, Ereigniszustellung, nur schmutzige Bereiche neu zusammensetzen. Ueber Capability-Handles. | **fertig** (K10) |
| 2.3 | **Echte Schriften** — TrueType lesen und rastern, Kantenglaettung, Unterschneidung. Der 8x16-Zeichensatz reicht nur fuer eine Konsole. | **fertig** (K10, TrueType mit Kantenglaettung, je Zeichen gegen eine zweite Rasterung geprueft) |
| 2.4 | **Widget-Bibliothek** — Knoepfe, Listen, Bildlaufleisten, Textfelder, Menues | **fertig** (K15), und zwar **in Ring 3**: `kernel/user/wlib.fi` 2843 Zeilen (Widgets, Anordnung, Ereignisschleife) und `kernel/user/wlibc.fi` 862 (Zeichenflaeche, Grundformen, Glyphen, Farbschema) gegen **499** Zeilen Naht im Kernel (`kernel/wig.fi`, sieben Aufrufe 1800..1806, die kein Widget kennen). Die Zeichenflaeche ist ein **Streifen** von 800 x 81 Bildpunkten (259 200 Oktette aus `mmap`) und nicht der Fensterpuffer — ein Prozess hat hier 832 KiB zwischen `brk` und `mmap`, ein Fenster von 640 x 420 braucht 1 075 200 Oktette. Gemessen wird **je Zeichen** und nicht je Flaeche (die Lehre aus K7B, wo Text „zu 87 Prozent" stimmte, waehrend jeder Buchstabe fehlte). Gegenprobe `nodirty`: 18 966 648 statt 40 800 000 zusammengesetzte Bildpunkte fuer denselben Klick |
| 2.4a | **KEIN grafischer Oberflaechen-Entwerfer.** Am 26.08.2026 ausdruecklich verworfen. Die Widget-Bibliothek wird benutzt wie Windows Forms OHNE Designer: Fenster im Quelltext zusammenbauen, keine Ziehflaeche, kein Eigenschaftenfenster, kein Codegenerator. Steht hier, damit die Frage nicht in einem halben Jahr wiederkommt. | verworfen |
| 2.5 | **Terminalfenster** — die Shell in einem Fenster statt auf der ganzen Flaeche | **fertig** (K10, `/bin/sh` laeuft darin) |
| 2.6 | **Datei-Explorer** — `/bin/explorer`, angezeigt als Datei-Explorer, zweiter Name `/bin/files`. KEIN Eigenname: der Name IST die Beschreibung, wie bei Windows. Dazu ein Programmverzeichnis `/apps/<name>.prog/` (Buendel wie bei Apple, aber NICHT `.app` — zu sehr Apple) und eine sofortige Suche ueber das ganze Dateisystem nach dem Vorbild von Everything. | **fertig** (K15). `kernel/user/explorer.fi` 1013 Zeilen; `/bin/files` ist ein **zweiter Verzeichniseintrag auf dieselbe Inode**, kein zweites Exemplar. Buendel: `kernel/user/appdir.fi` 503 Z., fuenf `.prog`-Verzeichnisse mit `INFO`, `start`, `symbol` (Format OSYM, 12 Oktette Kopf) und `daten/`; `start` ist wieder ein zweiter Name auf `/bin/<prog>`, fuenf Buendel fuer 896 KiB Programme kosten deshalb 5 x 1036 Oktette Symbol und **keinen** Block Code. Starter mit Schluesselwortsuche: `starter.fi` 443 Z. Namensindex nach dem Everything-Verfahren: `kernel/nidx.fi` 191 Z. (Journalring im Kernelspeicher, beschrieben an genau zwei Stellen — `fs.dir_add` und `fs.dir_remove`), `kernel/user/nidx.fi` 531 Z., `suchen.fi` 356 Z. Zahlen unten in der eigenen Tabelle. Gemessen: `tools/k15/run.sh` **251 Zusagen, 0 Fehler**, 31 QEMU-Laeufe, 26 davon mit Bildschirmfoto |
| 2.6b | **Der Namensindex, in Zahlen** — steht hier und nicht in einer Fussnote, weil er die einzige Zusage dieser Gruppe ist, die eine Groessenordnung behauptet | **gemessen** (K15, `tools/k15/gross.py`, Abbild mit 4000 leeren Dateien in siebzehn Ordnern, `--inodes=4096`). Siehe die Tabelle unter dieser Gruppe |
| 2.7 | **Einstellungen, Themen, Hintergrundbilder** — Farbschema als Datei, Bild laden, ein Programm dafuer. Fleissarbeit, sobald der Fensterserver steht. | offen |
| 2.8 | **Aufgabenverwaltung** — `top` auf der Konsole, danach grafisch: Prozesse, Speicher, Last, Beenden | `top` fertig (K11), grafisch offen |

### Der Namensindex gegen den Verzeichnisdurchlauf (K15)

Ein Abbild mit **4000 leeren Dateien** in einem Baum aus siebzehn Ordnern,
im selben Prozess gemessen: erst der Index, dann derselbe Suchbegriff als
rekursiver Baumdurchlauf.

| | |
|---|---:|
| Namen aus der Inode-Tabelle | **4021** |
| Systemaufrufe fuer den Aufbau | 65 (63 Saetze je Aufruf) |
| Zeit fuer den Aufbau | 820 123 µs, also 203 µs je Name |
| abgeschnitten | 0 |

| Wort | Treffer | Index | Baumdurchlauf | Namen ungleich | schneller um |
|---|---:|---:|---:|---:|---:|
| `kupfer` | 1 | 6 966 µs | 1 966 404 µs | **0** | **282x** |
| `07` | 179 | 9 228 µs | 1 766 744 µs | **0** | 191x |
| `quaste` | 0 | 6 892 µs | 1 804 693 µs | **0** | 261x |

**Warum hier 282 und nicht 332 steht.** Im Auftrag zu dieser Nachziehrunde
stand „Faktor 332". Im Logbuch der Runde (Osum `docs/ROUNDK15.md`,
Abschnitt 6c) stehen 282, 191 und 261 — je nach Suchwort. Eingetragen ist,
was gemessen wurde. Festgenagelt sind im Laeufer ohnehin nicht die
Mikrosekunden, sondern die **Trefferzahlen** und die **Namensgleichheit**:
beide Wege liefern nicht nur dieselbe Zahl, sondern dieselben Namen,
sortiert und Oktett fuer Oktett verglichen (`ungleich=0`). Die Zeiten
schwanken mit der Last der Maschine um etwa ein Fuenftel, die
Groessenordnung nicht.

**Und die Gegenprobe war zuerst kaputt, das gehoert dazu.** Der
Baumdurchlauf merkte sich anfangs *jeden* gelesenen Namen und war nach
sechzig Namen voll — er stieg dann in kein Unterverzeichnis mehr ab und
fand zu wenig. Ausserdem war `getdents64` quadratisch (31 000
Leseoperationen fuer 250 Dateien), der erste Messlauf ueber 4000 Dateien
lief zehn Minuten und war nicht fertig. Eine Gegenprobe, die man gewinnt,
weil man dem Gegner ein Bein stellt, ist keine; beides ist repariert,
bevor die Zahl oben entstand.

**Was der Index NICHT kann**, damit die Zahl nicht mehr verspricht, als
sie deckt: er ueberlebt keinen Neustart (das Journal liegt im
Kernelspeicher, Neuaufbau 0,94 s fuer 4021 Namen), der Journalring fasst
56 Saetze und meldet `lost`, wenn er ueberlaeuft, ueber 4200 Namen meldet
er „abgeschnitten", und er haelt **Namen, keine Pfade**.

## 3 — Geraete

| | Was | Stand |
|---|---|---|
| 3.1 | **USB** — xHCI-Hostcontroller, Geraeteklassen (Tastatur, Maus, Massenspeicher), Anstecken im Betrieb. Der dickste Brocken dieser Gruppe, und Voraussetzung fuer fast jede echte Maschine. | **in Arbeit: Runde K17** (Osum, Zweig `k17-usb`, abgezweigt von `c5fe12f`). Stand am 26.08.2026, 15:30: `kernel/xhci.fi` mit 1105 Zeilen im Arbeitsbaum, noch nicht eingecheckt; dazu Aenderungen an `kmain.fi`, `kstate.fi`, `ps2m.fi`, `boot.s`. Es liegen noch **keine gemessenen Zahlen** vor — was hier steht, ist der Umfang, nicht das Ergebnis |
| 3.2 | **Ton** — AC97 oder Intel HDA, Mischer, Lautstaerke, ein Abspielprogramm | offen |
| 3.3 | **Beruehrungsbildschirm** — ueber USB-HID oder I2C; sinnvoll erst nach 2.2 und 3.1 | offen |
| 3.4 | **Grafikkarte** — heute nur der lineare Rahmenpuffer des Bootladers. Modussetzung und Beschleunigung waeren die naechste Stufe. | offen |
| 3.5 | **Weitere Platten** — AHCI/SATA neben NVMe | offen |

## 4 — Energie und Leistung

Nicht „schneller machen", sondern **steuerbar** machen — wie die
Energieprofile von Windows oder Zorin: **Energiesparen · Ausgeglichen ·
Hoechstleistung**.

| | Was | Stand |
|---|---|---|
| 4.1 | **Taktverwaltung (P-States)** — Frequenz und Spannung ueber `IA32_PERF_CTL`/`HWP` setzen; darauf die drei Profile. Das ist der Kern der Sache. | **in Arbeit: Runde K18** (Osum, Zweig `k18-power`, abgezweigt von `c5fe12f`). Stand am 26.08.2026, 15:32: `kernel/pwr.fi` mit 755 Zeilen und `tools/k18/msrprobe.s` mit 496 Zeilen im Arbeitsbaum, noch nicht eingecheckt. **Keine gemessenen Zahlen** — und bei diesem Punkt ist das besonders zu betonen: eine Taktverwaltung, die nicht auf echter Hardware gemessen ist, hat nichts gezeigt |
| 4.2 | **Ruhezustaende (C-States)** — im Leerlauf wirklich schlafen statt zu drehen; `mwait` statt Warteschleife | in Arbeit (K18) |
| 4.3 | **Turbo** — die hoechste Stufe freigeben oder sperren, mit Blick auf die Temperatur | in Arbeit (K18) |
| 4.4 | **Akku und Netzteil** — Ladestand, Ladezustand, Restzeit, Netzteil an/ab ueber ACPI; Anzeige und Warnung | offen |
| 4.5 | **Temperatur und Luefter** — auslesen, drosseln bevor es heiss wird | offen |
| 4.6 | **Bildschirm** — Helligkeit, Abschalten nach Untaetigkeit | offen |
| 4.7 | **Bereitschaft** — Standby und Ruhezustand (S3/S4). Aufwaendig, weil jeder Treiber mitspielen muss. | offen |
| 4.8 | **Messen statt raten** — Zwischenspeicher fuer Dateisystembloecke, `mmap` statt Kopieren, Auslagern, Zeitgeberaufloesung. Erst sinnvoll, wenn es genug Last gibt, die man messen kann. | offen |

## 5 — Fremde Programme

| | Was | Stand |
|---|---|---|
| 5.1 | **Linux-Binaerkompatibilitaet** — statisch gelinkte Programme ueber Umsetzung der Systemaufrufnummern. Der ELF-Lader steht, und die POSIX-Schicht wurde absichtlich mit den Linux-Nummern gebaut. Das ist Wochen, nicht Jahre. | offen |
| 5.2 | **Dynamisches Linken** — Lader fuer Bibliotheken, damit auch nicht statisch gebundene Programme laufen | offen |
| 5.3 | **Hypervisor** — AMD-V (VT-x offen), verschachtelte Seitentabellen, Gastmaschinen. Damit laufen fremde Systeme unveraendert; der Nutzen geht weit ueber Windows hinaus (Isolation, Linux-Gaeste, Osum in Osum testen). | **fertig fuer AMD-V** (K12: VMCB, NPT, Gaeste, Handles aus Ring 3). VT-x offen — auf der Messmaschine (AMD EPYC ohne /dev/kvm) nicht pruefbar |
| 5.4 | **Windows-Programme** — ueber 5.3 in einem Gastsystem. Eine eigene Nachbildung der Windows-Schnittstellen (der Weg von Wine) ist ausdruecklich NICHT geplant: Wine arbeitet seit 1993 daran und hat allein fuer `user32.dll` 783 von rund 990 Funktionen. | spaeter |

## 6 — Software fuer das System

| | Was | Stand |
|---|---|---|
| 6.1 | **Paketverwaltung** — Format ist entworfen ([PACKAGING.md](PACKAGING.md)): unveraenderliche, inhaltsadressierte Pakete, drei Datentoepfe, Systemgenerationen. Gebaut ist nichts davon. Netz, Dateisystem und Pruefsummen stehen bereits. | offen |
| 6.2 | **Paketquelle** — ein Ort, von dem installiert wird; Signaturen | offen |
| 6.3 | **Browser** — entsteht im Firn-Repo: HTML-Baumbau und DOM (94,89 % der html5lib-Tests), Layout mit Flexbox (31,72 % der Web Platform Tests), Zeichnen in Arbeit. Danach: JS an den DOM binden, HTTP, Oberflaeche. | in Arbeit |
| 6.4 | **Der Firn-Uebersetzer laeuft auf Osum selbst** — der eigentliche Pruefstein. Ein System, auf dem man sein eigenes System bauen kann, ist erwachsen; alles davor ist Kreuzuebersetzen von aussen. | **fertig** (K16). Osum hat auf sich selbst ein Firn-Programm uebersetzt, gebunden und ausgefuehrt, und das Ergebnis ist **Oktett fuer Oktett** dasselbe, was derselbe Uebersetzer auf Linux aus derselben Quelle macht: Assemblertext **14 909 Oktette zeichengleich**, ELF-Datei **8192 Oktette zeichengleich**. Dazu ein eigener Assembler und Binder in Firn, `kernel/user/fas.fi`, 2423 Zeilen — moeglich, weil der Uebersetzer nur einen kleinen Ausschnitt von x86-64 benutzt: ueber 1 533 513 Zeilen Assemblertext gemessen **58** Mnemoniken, **96** Paare aus Mnemonik und Operandenform, 8 Direktiven, 4 Speicherausdruecke, kein Indexregister, kein Massstab, kein Segment. `fas` bindet **54 von 54** Programmen des Userlands; 16 davon gegen `as`+`ld` gehalten, 16-mal gleiche Ausgabe. `.fi` ist doppelklickbar (`kernel/ftype.fi`, `kernel/user/firun.fi`, 242 Z.). Gemessen: `tools/k16/run.sh` **64 Zusagen, 0 Fehler** |
| 6.4a | **…aber `firnc` liegt noch nicht im OrientOS-Produkt.** Das ist die ehrliche Trennung zwischen „Osum kann es" und „das Produkt hat es" | **offen, und leicht zu uebersehen.** `vendor/osum/hole-osum.sh` baut die 69 Programme aus `kernel/user/*.fi`; darunter sind `fas` und `firun`, **nicht** aber `firnc` — der entsteht in K16 durch Querbauen von Firns `bin/firnc1.fi` gegen `kernel/user/user.ld` und braucht dafuer das Firn-Repo. Solange das nicht in `hole-osum.sh` steht, kann man auf einem gebooteten OrientOS ein `.s` assemblieren, aber keine `.fi` uebersetzen |

## 7 — Andere Maschinen

| | Was | Stand |
|---|---|---|
| 7.1 | **Die Architekturgrenze** — x86-Details liegen im Kernel ueberall statt an einer Stelle. Der Rust-Kernel dieses Repos HATTE diese Grenze (Traits plus ein Testschritt, der x86-Begriffe ausserhalb von `arch/` verbot); sie ist **nicht** nach Firn portiert worden, weil das eine Umbauarbeit an jedem Modul waere und keine Portierung. Die Anforderung steht als unuebersetzte Vorlage im Baum: `vorlage/arch_iface.rs`. Der eigentliche Inhalt ist nicht der Trait-Text, sondern die Regel. Siehe [KERNELWECHSEL.md](KERNELWECHSEL.md) § 4.1. | offen, mit Vorlage |
| 7.1b | **aarch64** — Firn uebersetzt bereits nach ARM (296 von 300 Faellen auf beiden Maschinen identisch), der Kernel nicht. Setzt 7.1 voraus. | offen |
| 7.2 | **Die Geraetewirklichkeit auf ARM** — kein PCI-Erkennungsweg wie beim PC, sondern Geraetebaum, und jeder SoC ist anders. Der eigentliche Aufwand. | offen |
| 7.3 | **Der Browser als App auf Android** — der schnellste Weg, das Ergebnis in die Hand zu bekommen. Android ist Linux plus Bionic; eine APK darf native Bibliotheken enthalten. Also: Browserkern nach `aarch64-linux-android` uebersetzen, als `.so` einpacken, duenne Huelle fuer Zeichenflaeche und Beruehrung. Voraussetzung: der Browser ist fertig genug und laesst sich als Bibliothek herausloesen — beim Bauen von Anfang an mitdenken. | offen |

## 8 — Sicherheit und Haerte

| | Was | Stand |
|---|---|---|
| 8.1 | **SMEP/SMAP** — Ring 0 fuehrt keinen Nutzercode aus und fasst Nutzerdaten nur im `stac`-Fenster an. Auf JEDEM Kern (CR4 ist pro Prozessor), zurueckgelesen statt behauptet. Gegenproben: `smapraw` gibt mit dem Bit einen #PF und ohne es das Oktett. | **fertig** (26.08.2026, Osum `kernel/guard.fi`) |
| 8.2 | **Speicherverwuerfelung (KASLR)**, Schutzseiten, nicht ausfuehrbarer Stapel | offen |
| 8.3 | **Signierte Pakete und signierter Start** | offen |
| 8.4 | **Dauerlauf** — Tage statt Minuten, mit Blick auf Fragmentierung und Lecks | offen |

---

## 9 — Was der Kernelwechsel offengelassen hat

Der Wechsel auf den Osum-Kernel ist am 26.08.2026 mit dem Schnitt
abgeschlossen worden ([KERNELWECHSEL.md](KERNELWECHSEL.md) § 7). Zwei
Punkte sind dabei **bewusst** nicht portiert worden, und beide stehen
hier, damit sie nicht in einer Fussnote verschwinden.

| | Was | Stand |
|---|---|---|
| 9.1 | **Die Architekturgrenze** — dieselbe Sache wie 7.1, von der anderen Seite: nicht „wir wollen ARM", sondern „der Rust-Kernel konnte etwas, das der Firn-Kernel nicht kann". Vorlage: `vorlage/arch_iface.rs`, 378 Zeilen, nicht uebersetzt. | offen, mit Vorlage |
| 9.2 | **Kanaele, Ports, Namensraeume, `ProcessSpawn`, Speicherobjekte** der nativen ABI. Die Nummern (ab 2000), die Fehlerwerte und die Bedeutungen sind portiert und stehen in Osums `sys.fi`; die Objekte dahinter fehlen und antworten `NotSupported` (−9). `NotSupported` und nicht `ENOSYS`: der Aufruf EXISTIERT in dieser ABI, dieser Kernel bietet ihn nur nicht an. | offen |
| 9.3 | **Ein markenabhaengiges Userland.** Zwei Marken unterscheiden sich bisher nur im Namen. `userland/PROGRAMME.<marke>` waere der naechste Schritt — der Quelltext bliebe derselbe. | offen |
| 9.4 | **Ein Schreibpfad auf eine echte Platte.** Das Boot-Modul ist eine RAM-Platte; Aenderungen ueberleben den Lauf nicht. | **halb erledigt, und die Haelften gehoeren getrennt.** Der Partitionstafel-Leser ist da (K14, `kernel/part.fi`, MBR und GPT mit beiden CRC32), FAT32 kann schreiben (K14, `kernel/fat.fi`), eine zweite Platte kann der Blockschicht genannt werden. Was **fehlt**, ist der Installationsweg: OrientOS baut ein ISO mit einem Boot-Modul, kein Programm schreibt ein Wurzeldateisystem auf eine Platte, und nichts davon ist gemessen |

---

## 10 — Der festgenagelte Kernel, und was an ihm zu berichtigen war

OrientOS baut nicht gegen den neuesten Osum, sondern gegen **einen**
Commit (`vendor/osum/COMMIT`). Am 26.08.2026 ist der Nagel von `7a53ac3`
auf **`c5fe12f`** gerueckt — vier Runden und 32 525 Zeilen dazwischen
(`git diff --stat 7a53ac3 c5fe12f`: 103 Dateien, 32 525 eingefuegte,
267 geloeschte Zeilen).

**Dieser Commit uebersetzt nicht.** Das ist keine Nebenbemerkung, sondern
der Grund, warum es in diesem Repo seit dem 26.08.2026 einen Patchstapel
gibt. Dem Merge von `k15-ui` sind zwei Stellen abhanden gekommen:

| Datei | Was fehlt | Was firnc sagt |
|---|---|---|
| `kernel/kmain.fi` | EINE schliessende Klammer in `mode_of` | `'fn' is only allowed at top level, not inside a function body` — 13 Funktionen stehen dadurch im Rumpf von `mode_of` |
| `kernel/sys.fi` | die K15-Naht (1800..1806) liegt in `k13_call` statt in `dispatch` | `unknown name 'a4'` — `k13_call` nimmt a0..a3. Und uebersetzt waere der Zweig **tot**: `is_k13` zaehlt 1800..1806 nicht auf |

Beides sind Merge-Verluste und keine Absicht: der Elternteil `78f8e86`
hat den `kmain`-Block vollstaendig, der Elternteil `fcb5c30` hat die Naht
in `dispatch`. Berichtigt wird es **hier** und nicht im Kernelrepo — ein
verschobener Nagel ist ein anderer Stand und entwertet die Messungen von
gestern. Die Dateien liegen in `vendor/osum/patches/`, `hole-osum.sh`
wendet sie beim Auspacken an und **bricht ab**, wenn eine nicht mehr
passt; `tests/step-05-patches.sh` nimmt jede einzeln wieder heraus und
verlangt, dass firnc den Kernel dann ablehnt.

**Unabhaengig bestaetigt.** Dieselben zwei Schaeden sind am selben Tag von
den beiden laufenden Kernelrunden gefunden worden, jede fuer sich:
`k17-usb` (Commit `93ad914`, „zwei Merge-Schaeden aus c5fe12f behoben,
sonst baut nichts") und `k18-power` (Commit `eed1edd`, „main liess sich
nicht uebersetzen — zwei Reste der K15-Verschmelzung"). Drei Befunde,
dieselbe Klammer. Sobald einer dieser Zweige nach `main` geht, passt der
Patch nicht mehr, der Lauf wird rot, und die Datei gehoert geloescht —
nicht angepasst.

**Und eine Zahl, die dabei aufgefallen ist.** `userland/PROGRAMME` stand
auf `bloecke: 6144`. Die Blockkarte von OFS ist EIN Block zu 512
Oktetten, also 4096 Bits, also hoechstens 4096 Bloecke je Abbild — bis
K15 hat `mkfs.py` das nicht geprueft und 6144 klaglos gebaut; die oberen
2048 Bloecke waren nicht adressierbar. Aufgefallen ist es nie, weil das
Produkt sie nie brauchte: 27 Programme, 932 584 Oktette, **1822 von 4062
Datenbloecken**. Seit K15 lehnt `mkfs.py` 6144 ab, und das ist richtig so.

---

## Laufende Regeln

1. **Kein Linux-Code, kein Fork.** Nachbauen aus Spezifikationen ja, uebernehmen nein.
2. **Keine Behauptung ohne Messung.** Jede Zusage hat eine **Gegenprobe** — denselben Kernel mit abgeschalteter Eigenschaft, wo die Messung zusammenbrechen muss. Ein gruener Test ohne Gegenprobe zaehlt nicht.
3. **Nichts wird geloescht, bevor der Ersatz nachweislich laeuft.**
4. **Was fehlt, steht da.** Luecken werden benannt, nicht weggeschrieben.
5. **Was geloescht wird, bleibt in der Historie.** `git rm`, nie `rm`. Ein
   geloeschter Kernel ist nicht vergessen, er ist nur nicht mehr im Weg —
   und `./test.sh` prueft nach, dass die Historie ihn noch kennt.

## Verwandte Entwurfsdokumente

| Datei | Inhalt |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Schichten, Architekturgrenze, Abhaengigkeiten |
| [PACKAGING.md](PACKAGING.md) | unveraenderliche, inhaltsadressierte Pakete, Systemgenerationen |
| [FILESYSTEM.md](FILESYSTEM.md) | VFS, FAT32, ext4 lesend, eigenes Dateisystem |
| [LANGUAGE.md](LANGUAGE.md) | Firn: warum eine eigene Sprache, und was sie dem Kernel abverlangt |
| [KERNELWECHSEL.md](KERNELWECHSEL.md) | Abgleich Modul fuer Modul, was portiert ist und was aussteht |
| [BRANDING.md](BRANDING.md) | ein Quelltext, mehrere Produkte |
