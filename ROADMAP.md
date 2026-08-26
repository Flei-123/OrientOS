# Roadmap von OrientOS

Ziel: ein eigenstaendiges Betriebssystem — kein Fork, kein uebernommener
Fremdcode, Kernel und System in [Firn](LANGUAGE.md). Zeithorizont: Jahre.
Sortiert nach Abhaengigkeit, nicht nach Attraktivitaet.

Legende: **fertig** · **teilweise** · **offen**

Was heute laeuft, steht in [README.md](README.md) unter „Was es kann" und
„Was es nicht kann". Diese Datei sagt, was noch zu tun ist.

---

## 1 — Das System benutzbar machen

Ohne diese Punkte ist Osum ein Vorfuehrsystem, kein Arbeitsgeraet.

| | Was | Stand |
|---|---|---|
| 1.1 | **Editor** — bildschirmorientiert, roher Terminalmodus, Suchen/Ersetzen, Rueckgaengig. Ohne ihn laesst sich auf dem System keine Datei aendern. | **fertig** (K11, `/bin/edit`) |
| 1.2 | **Werkzeugkasten** — `find`, `sed`, `diff`, `patch`, `tar`, `gzip`, `xargs`, `du`, `top`, `mount`, `cut`, `tr`, `tee` | **fertig** (K11, 20 Werkzeuge gegen ihre GNU-Gegenstuecke gemessen) |
| 1.3 | **Shell-Skripte** — `if`, `for`, `while`, `case`, Funktionen, Variablenersetzung, `test` | **fertig** (K11) |
| 1.4 | **Benutzer und Rechte** — `uid`/`gid`, `chmod`, `chown`, `setuid`, Anmelden, `passwd`. Heute laeuft alles mit allen Rechten. | offen |
| 1.5 | **`init` und Dienste** — heute startet der Kernel die Shell unmittelbar. Ein System braucht einen ersten Prozess, der Dienste startet, ueberwacht, neu startet und beim Herunterfahren aufraeumt. | offen |
| 1.6 | **VFS** — mehrere Dateisysteme nebeneinander einhaengen. Voraussetzung fuer `/proc`, `/dev` und fuer fremde Platten. | offen |
| 1.7 | **FAT32 und ext4 (lesend)** — damit Osum Platten liest, die ein anderes System beschrieben hat | offen |

## 2 — Oberflaeche

| | Was | Stand |
|---|---|---|
| 2.1 | **Maus** (PS/2, spaeter USB-HID) | **fertig** (K10, PS/2 an IRQ 12) |
| 2.2 | **Fensterserver** — Fenster anlegen, verschieben, stapeln, Fokus, Ereigniszustellung, nur schmutzige Bereiche neu zusammensetzen. Ueber Capability-Handles. | **fertig** (K10) |
| 2.3 | **Echte Schriften** — TrueType lesen und rastern, Kantenglaettung, Unterschneidung. Der 8x16-Zeichensatz reicht nur fuer eine Konsole. | **fertig** (K10, TrueType mit Kantenglaettung, je Zeichen gegen eine zweite Rasterung geprueft) |
| 2.4 | **Widget-Bibliothek** — Knoepfe, Listen, Bildlaufleisten, Textfelder, Menues | in Arbeit (Runde K15) |
| 2.4a | **KEIN grafischer Oberflaechen-Entwerfer.** Am 26.08.2026 ausdruecklich verworfen. Die Widget-Bibliothek wird benutzt wie Windows Forms OHNE Designer: Fenster im Quelltext zusammenbauen, keine Ziehflaeche, kein Eigenschaftenfenster, kein Codegenerator. Steht hier, damit die Frage nicht in einem halben Jahr wiederkommt. | verworfen |
| 2.5 | **Terminalfenster** — die Shell in einem Fenster statt auf der ganzen Flaeche | **fertig** (K10, `/bin/sh` laeuft darin) |
| 2.6 | **Datei-Explorer** — `/bin/explorer`, angezeigt als Datei-Explorer, zweiter Name `/bin/files`. KEIN Eigenname: der Name IST die Beschreibung, wie bei Windows. Dazu ein Programmverzeichnis `/apps/<name>.prog/` (Buendel wie bei Apple, aber NICHT `.app` — zu sehr Apple) und eine sofortige Suche ueber das ganze Dateisystem nach dem Vorbild von Everything: die OFS-Inodetabelle am Stueck lesen und einen Namensindex im Speicher halten, statt den Verzeichnisbaum zu durchlaufen. | in Arbeit (K15) |
| 2.7 | **Einstellungen, Themen, Hintergrundbilder** — Farbschema als Datei, Bild laden, ein Programm dafuer. Fleissarbeit, sobald der Fensterserver steht. | offen |
| 2.8 | **Aufgabenverwaltung** — `top` auf der Konsole, danach grafisch: Prozesse, Speicher, Last, Beenden | `top` fertig (K11), grafisch offen |

## 3 — Geraete

| | Was | Stand |
|---|---|---|
| 3.1 | **USB** — xHCI-Hostcontroller, Geraeteklassen (Tastatur, Maus, Massenspeicher), Anstecken im Betrieb. Der dickste Brocken dieser Gruppe, und Voraussetzung fuer fast jede echte Maschine. | offen |
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
| 4.1 | **Taktverwaltung (P-States)** — Frequenz und Spannung ueber `IA32_PERF_CTL`/`HWP` setzen; darauf die drei Profile. Das ist der Kern der Sache. | offen |
| 4.2 | **Ruhezustaende (C-States)** — im Leerlauf wirklich schlafen statt zu drehen; `mwait` statt Warteschleife | offen |
| 4.3 | **Turbo** — die hoechste Stufe freigeben oder sperren, mit Blick auf die Temperatur | offen |
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
| 6.4 | **Der Firn-Uebersetzer laeuft auf Osum selbst** — der eigentliche Pruefstein. Ein System, auf dem man sein eigenes System bauen kann, ist erwachsen; alles davor ist Kreuzuebersetzen von aussen. Braucht die POSIX-Schicht in vollem Umfang, `mmap` auf Dateien, Auslagern — und den Editor aus 1.1. | offen |

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
| 9.4 | **Ein Schreibpfad auf eine echte Platte.** Das Boot-Modul ist eine RAM-Platte; Aenderungen ueberleben den Lauf nicht. Osum kann ATA und NVMe, aber es fehlen Partitionstabellen-Leser und Installationsweg. | offen |

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
