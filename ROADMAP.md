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

### Die Deckel, die in keiner Roadmap standen (OFS3 / MEM, 27.08.2026)

Beide Punkte hier haben **gefehlt** — nicht als „offen" eingetragen,
sondern gar nicht. Beide sind am 27.08.2026 am Baum gefunden worden und
nicht in einem Logbuch, und beide waren Groessenordnungen und keine
Feinheiten. Sie stehen deshalb hier, mit den Zahlen aus den Laeufen und
mit dem, was sie NICHT beweisen.

| | Was | Stand |
|---|---|---|
| 1.8 | **Die Platte darf groesser sein als zwei Megaoktett.** OFS hatte EINEN Block Blockkarte: 512 Oktette sind 4096 Bits, also 4096 Bloecke, also **2 MiB je Platte** — seit Runde 62, und der Quelltext hat es selbst gesagt („wer mehr will, braucht eine mehrblockige Karte, das ist eine andere Runde"). Dahinter sass ein zweiter Deckel: `kmain.fi` meldete die Wurzelplatte mit der Konstanten `OSUM_BLOCKS = 4096` an, **ganz gleich was dranhing** — eine Platte von 32 MiB lief nach 2 MiB voll. | **fertig** (Osum, Zweig `ofs3`, Commit `e2ee235`). Fassung 3 des Formats: mehrblockige Karte aus dem Superblock, Inode 128 → **256** Oktette (drei Zeitstempel, dreifach indirekter Zeiger), Verzeichniseintrag 32 → **264** (Name 23 → **255** Zeichen), `T_LINK` als vierte Inodeart, `rename` als echte Verrichtung. `blk.capacity` stellt jetzt ATA IDENTIFY. Gemessen (`tools/ofs3/run.sh`, **75 Zusagen, 0 Fehler**): groesste Platte 2 MiB → **4 GiB** (`df` im Gastsystem: `blocks total=8388608 free=8385271`), groesste Datei 2 134 016 → **136 351 744** Oktette (geschrieben und an drei Stellen zurueckgelesen: 2 396 160), Datei an Block **201252** gelesen UND beschrieben, Zeitstempel nach Neustart Zeichen fuer Zeichen derselbe. Aufwaertskompatibel: Fassungen 1 und 2 booten weiter (`ofsver=1`, `ofsver=2`), ohne `--v3` baut `mkfs.py` Oktett fuer Oktett dieselbe Platte wie vorher. **NICHT bewiesen:** nur 4 GiB gemessen (Format und LBA28-Treiber koennten 128 GiB), die 4-GiB-Platte ist eine Loch-Datei und nie vollgeschrieben, der dreifach indirekte Zeiger ist umgesetzt aber nie wirklich gebraucht worden, `rename` ueberschreibt nicht (POSIX verlangt es) und braucht `vfs` auf der Befehlszeile, harte Verweise gibt es aus Ring 3 nicht. [docs/ROUNDOFS3.md](../osum/docs/ROUNDOFS3.md) |
| 1.9 | **Der Arbeitsspeicher darf groesser sein als 512 Megaoktett.** Die Rahmen-Bitmap lag fest im Kerndatenbereich (`kstate.BITMAP_BYTES = 0x4000`), und die Rahmenzahl war `BITMAP_BYTES * 8` = 131 072 = 512 MiB — gemessen mit `-m 512`, `-m 2G`, `-m 8G` und `-m 64G` stand in ALLEN VIER Laeufen dieselbe Zahl. Und darueber hat der Kernel nicht bloss Speicher verschenkt, **er ist gestorben**: `boot.s` baut EINE Seitentafel zu 512 mal 2 MiB, also genau ein Gibioktett Identitaetsabbildung, und mit `-m 2G` endete der Lauf in `*** EXCEPTION 14 #PF err=0x0 cr2=0x7ffe1adc` — den ACPI-Tafeln knapp unter 2 GiB. | **fertig** (Osum, Zweig `ofs3`, eigener Commit). Drei Schritte, und die Reihenfolge ist die Sache: (1) die kleine Karte im Kerndatenbereich wie bisher, damit es ueberhaupt einen Zuteiler gibt; (2) die Identitaetsabbildung mit Rahmen aus Schritt 1 vergroessern — ein Rahmen je Gibioktett, PDPT ueber `cr3` gefunden statt ueber ein Symbol; (3) die grosse Karte aus Schritt 1 belegen, die 16 KiB von Schritt 1 darueberkopieren und erst dann den Rest eintragen. Gemessen (`tools/mem/run.sh`, **50 Zusagen, 0 Fehler**, ein Kernelabbild, nur `-m` anders): `-m 64G` → **17 039 360 Rahmen verwaltet, 16 775 326 frei, 2 129 920 Oktette Karte, 65 GiB abgebildet**, Lauf endet mit `kernel: done`. Ein Rahmen an **0x103ffff000** (65 GiB) beschrieben und zurueckgelesen. 200 000 Rahmen genommen, jeder beschrieben, jeder geprueft, jeder zurueckgegeben — freie Rahmen vorher = nachher in jeder Groesse. Auf 512 MiB unveraendert (131 072 Rahmen, 16 384 Oktette Karte). Kosten: 32 KiB Karte je GiB, bei 64 GiB **0,0034 %**. **NICHT bewiesen:** alles in QEMU/TCG, nichts auf echter Hardware; zwischen 64 und 512 GiB ist nichts gemessen (`MAX_GIB = 512`, ein PDPT); die Identitaetsabbildung wird als schreibrueckstellend zwischengespeicherte 2-MiB-Seiten gebaut, ohne MTRR/PAT; von den 16 Millionen Rahmen sind 200 000 wirklich angefasst worden und genau EINER am oberen Ende; nichts in diesem System FRAGT je nach so viel Speicher. [docs/ROUNDMEM.md](../osum/docs/ROUNDMEM.md) |

**Und ein dritter Deckel, der dabei aufgefallen ist und noch steht:** die
Kernelhalde ist 256 KiB aus 64 zusammenhaengenden Rahmen (`kmain.fi`),
erster Treffer, keine Groessenklassen. Das ist eine eigene Runde.

| | Was | Stand |
|---|---|---|
| 1.10 | **Kernelhalde** — 256 KiB fest, `kalloc`/`kfree` erster Treffer ohne Groessenklassen. Bei 64 GiB Arbeitsspeicher ist das dieselbe Sorte Missverhaeltnis wie 1.9 vorher. | offen |

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
| 2.6c | **Speicherplatzanalyse** — `/bin/speicher` als Fenster und `/bin/du` auf der Kommandozeile, nach dem Vorbild von TreeSize, aber **ohne Durchlauf beim Start**. Am 27.08.2026 von Justin gewuenscht („so ein Tool wie TreeSize, nur besser und bereits integriert“). Moeglich, weil der Namensindex aus 2.6b schon steht: er traegt seit dieser Runde auch die **Groesse**, aufsummiert bis zur Wurzel, nachgefuehrt vom Aenderungsjournal des Kernels. | **fertig** (Runde SPEICHER, Zweig `speicher`). `kernel/user/speicher.fi` 909 Zeilen (Baum links, groesste Dateien rechts, Treemap nach „slice and dice“ unten, Loeschen aus dem Programm heraus), `kernel/user/du.fi` auf **denselben** Index umgestellt — zwei Wege mit verschiedenen Zahlen waeren ein Fehler. Der Journalring von 56 auf **127** Saetze vergroessert und in zwei eigene kdata-Seiten gezogen, weil seit dieser Runde **jeder `write`, der eine Datei waechst**, ein Satz ist. Zahlen unten in der eigenen Tabelle. Logbuch: Osum `docs/ROUNDSPEICHER.md` |
| 2.7 | **Einstellungen, Themen, Hintergrundbilder** — Farbschema als Datei, Bild laden, ein Programm dafuer. Fleissarbeit, sobald der Fensterserver steht. | offen |
| 2.7b | **Optischer Feinschliff der Oberflaeche** — am 27.08.2026 von Justin beanstandet: die Bildschirmfotos sehen schlecht aus. Belegt und nicht geraten: der Bildschirm laeuft mit **800x600** (Fenster, Titelleisten und Schrift wirken dadurch grob), das Farbschema hat nur **160 bis 178** verschiedene Farben, und ein einziger Blauton (47,95,156) traegt die gesamte Hervorhebung. Zu tun: hoehere Aufloesung ueber GOP/VBE (mindestens 1280x800), groessere Schriftgrade, gleichmaessige Abstaende und Randbreiten, ein durchdachtes Farbschema statt gewachsener Einzelwerte, Fensterrahmen und Titelleiste neu gezeichnet. | offen |
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

### Der Groessenindex gegen den Verzeichnisdurchlauf (SPEICHER)

Dieselbe Anordnung wie oben, aber ein Abbild, in dem die Dateien **Inhalt
haben** (`tools/speicher/baum.py`): 4000 Dateien in siebzehn Ordnern,
davon 308 mit Inhalt, 419 868 Oktette unter `/daten`. Leere Dateien
taugen fuer einen Namensindex, fuer eine Speicherplatzanalyse nicht —
alle Summen waeren null, und ein Fehler in der Aufsummierung fiele nicht
auf, weil 0 + 0 immer 0 ergibt.

| | |
|---|---:|
| Namen aus der Inode-Tabelle | **4030** |
| Aufbau des Index, einmalig | 1 365 386 µs (1,37 s) |
| **Abfrage aus dem Index** (`/daten`) | **13,4 µs** (zehn Abfragen in 134 µs) |
| **Vollstaendiger Durchlauf** (`/daten`) | **220 661 633 µs** (220,66 s) |
| Abfrage gegen Durchlauf | **16 467 286x** |
| **Aufbau** gegen Durchlauf | **161,6x** |
| Verzeichnisse gegengerechnet | **21 von 21**, Oktett fuer Oktett |
| Aufbau der ganzen Anzeige in `/bin/speicher` | 5 882 µs |

**Die letzte Zeile der oberen Haelfte ist die ehrlichere.** Der Index muss
ja auch erst entstehen. Er entsteht ueber `fs.scan` — die Inode-Tabelle
am Stueck, genau der Trick, mit dem WizTree schnell ist —, und schon
dieser Aufbau ist **161-mal** schneller als der Durchlauf. Er passiert
**einmal**, nicht bei jeder Frage.

**Drei Zahlen, nicht zwei.** Index und Durchlauf stehen in derselben
Datei und lesen dieselben Inodes; ein Denkfehler in der Regel, *was*
gezaehlt wird, stuende in beiden gleich falsch drin. Deshalb rechnet
`tools/speicher/baum.py` dieselben Summen ein drittes Mal auf dem **Wirt**,
in Python, ohne Kenntnis des Kernels. Alle 21 Verzeichnisse stimmen in
allen drei Wegen Oktett fuer Oktett ueberein.

**Dass das Journal Wachstum mitbekommt, ist der Kern der Sache** — nicht,
dass es neue Namen mitbekommt. Gemessen: 80 Bloecke zu 512 Oktetten in
eine neue Datei geschrieben ergeben **81 Journalsaetze**, `lost=0`, und
die Wurzelsumme bewegt sich um **genau** 40 960 Oktette (1 101 656 →
1 142 616); das Loeschen bringt genau den alten Stand zurueck; der Index
wurde im ganzen Lauf **einmal** gebaut. Die Gegenprobe ohne Nachziehen
steht daneben (`erwartet=1 142 616`, `nachher=1 101 656`, `ok=0`) — also
zog vorher wirklich das Journal.

**Was das NICHT ist: ein Vergleich mit TreeSize.** Gemessen wurde Osum
gegen Osum. Dass TreeSize durchlaufen *muss*, ist eine Aussage ueber
seinen Aufbau, keine Messung an seinem Programm. Und die Zeiten gelten
fuer QEMU ohne KVM — auf echter Hardware waeren beide kleiner. Die
uebrigen Einschraenkungen (harte Verweise ungeprueft, Index fasst 4200
Namen, nur 308 Dateien mit Inhalt) stehen in `docs/ROUNDSPEICHER.md`,
Abschnitt „Was NICHT bewiesen ist“.

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
| 3.1 | **USB** — xHCI-Hostcontroller, Geraeteklassen (Tastatur, Maus, Massenspeicher), Anstecken im Betrieb. Der dickste Brocken dieser Gruppe, und Voraussetzung fuer fast jede echte Maschine. | **PLATZHALTER** |
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
| 4.9 | **Firn soll SCHNELLER werden als Rust** — nicht gleichauf, sondern davor. Ausdrueckliches Ziel von Justin, 27.08.2026. Bis dahin galt `<= 2x` als Messlatte; die ist jetzt ueberholt. **Stand heute: Median 2,08x LANGSAMER** als Rust `-O` (`bench/RESULTS.md` im Firn-Repo, Median aus 9 Laeufen, beide Seiten mit identischer Arbeit und `black_box`): fib 1,52x · bytecount 1,43x · statemachine 1,99x · bubblesort 2,17x · matmul 2,90x · **sieve 4,16x** — drei von sechs liegen schon unter 2x, `sieve` traegt den Median. Der Optimierer bringt gegenueber `--no-opt` bereits **13,8x**. Was dafuer noetig ist, ist benannt und nicht geraten: Schrankenpruefungen ueber Schleifen hinweg fehlen (in `decode` **225 Befehle je Oktett**, 28 % der Gesamtarbeit), Vektorbefehle werden nicht erzeugt, und die Registerzuteilung ist linear statt faerbend. Der Vorteil, den Firn hat: der Uebersetzer gehoert uns ganz — es gibt keine fremde Zwischensprache, an der die Optimierung endet. | offen, neues Ziel |
| 4.10 | **Der Kernel ist NICHT gegen Linux gemessen** — und das gehoert hierher, damit es niemand fuer erledigt haelt. Es gibt keine Zahl zu Systemaufruf-Durchsatz, Kontextwechselzeit oder Ein-/Ausgaberate im Vergleich zu Linux. Auf der Messmaschine (QEMU/TCG ohne `/dev/kvm`) waere eine solche Zahl auch wertlos: TCG uebersetzt Befehle, es taktet nichts. Braucht echte Hardware. | offen |
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
| 6.1 | **Paketverwaltung** — unveraenderliche, inhaltsadressierte Pakete, drei Datentoepfe, Systemgenerationen | **gebaut** (26.08.2026, Zweig `pkg`). Format `.opk` und die Abweichungen vom Entwurf: [PAKETE.md](PAKETE.md). Werkzeug `pkg/opk.py` mit `bauen zeigen installieren entfernen liste aktualisieren generationen zurueck aufraeumen pruefen baum verweise`. Gemessen (`tests/step-80-pakete.sh`, 35 Zusagen, 0 Fehler): zweimal gebaut ergibt Oktett fuer Oktett dasselbe Paket (237 075 Oktette); ein Paket mit beschaedigten Daten, beschaedigten Metadaten **oder** veraendertem Hash im Kopf wird abgelehnt, das unversehrte angenommen; nach Installieren und Entfernen ist der Baum Oktett fuer Oktett der von vorher (23 Eintraege verglichen, dazwischen 16 veraendert); eine Generation zurueck stellt `apps/` Eintrag fuer Eintrag wieder her (13 Eintraege). **Offen bleibt:** `opk` laeuft auf dem Wirt, nicht auf Osum — das gebootete System liest den Store und startet Pakete, installiert aber nicht |
| 6.1a | **The self-bearing PLAN** — the system state must describe a whole machine, not just its applications | **built** (27.08.2026, branch `plan2`). Typed PLAN lines `app` / `kernel` / `source` / `setting` / `account`, old `name<TAB>hash` files still readable; the kernel is a package with `kind=kernel` and rolls back with `zurueck`; nine settings render the files Osum really reads (`/etc/netz.conf`, `/etc/zeit.conf`, `/etc/schirm.conf`); accounts carry the **SHA-256 of the credential**, never the credential. Measured (`tests/step-90-plan.sh`, 33 assertions, 0 failures): a 745-octet plan plus a signed source rebuilt a tree from nothing that is **identical over 39 entries, 18 files and 3 470 309 octets**; without the credentials exactly one file differs (`/etc/shadow`, accounts locked); a source signed by a foreign key is refused. Packages 3 → **69** (`pkg/recipes.py`, 62 recipes generated by machine, 7 skipped with a reason). **Open:** nothing boots from `system/kernel` yet — that is round INSTALL; no `https://` sources; the product ISO carries no settings. [docs/PLAN-FORMAT.md](docs/PLAN-FORMAT.md) · [docs/BACKUP.md](docs/BACKUP.md) · [docs/ROUND-PLAN2.md](docs/ROUND-PLAN2.md) |
| 6.1b | **The appearance is part of the system state, and it is a USER setting** — the answer to "if I set up a new device, does it look the same?" | **built** (27.08.2026, addendum to `plan2`). A sixth PLAN type `pref <user> <key> <value>` for colour scheme, wallpaper and taskbar edge/height/autohide, rendered to `/users/<who>/config/desktop/`; system-wide things (resolution, network, timezone) stay `setting` -> `/etc/`. Images are content-addressed: the plan carries the SHA-256, the octets are an **asset package** (`kind=asset`) in the same store, same signed INDEX, same collector -- and are generated from four lines of text (`pkg/osym.py` -> OSYM). Measured (`tests/step-91-look.sh`, 27 assertions, 0 failures): a system with another scheme, another wallpaper, the taskbar on the left and another timezone was rebuilt on an empty root **identical over 62 entries, 35 files, 4 396 976 octets**; the wallpaper has **three names and one inode**; with TWO accounts the `/etc` compatibility view **disappears**, because there is no honest answer to whose theme `/etc/theme` would be. Packages 69 -> **73**. **Open:** the product ISO carries no appearance (`/etc` collision with `userland/PROGRAMME`); nothing switches the view on login; `einstellungen.fi` still writes `/etc` directly, so a change made in the running system is not a generation. [docs/CONFIG-LEVELS.md](docs/CONFIG-LEVELS.md) |
| 6.1c | **Two machines** — `.opk` and `/apps/<name>.prog/` on x86-64 **and** AArch64 | **built** (27.08.2026, second addendum to `plan2`). A seventh PLAN type `arch <machine>` says what the MACHINE is; `arch=` in the package metadata says what the PACKAGE is for — two different facts, each written once, and the second is inside the hash, so a package cannot change what it claims without becoming a different package. **The architecture is measured, not declared:** `bauen` reads `e_machine` out of the ELF header of every file it packs, so the two recipes for two builds are identical except for one path, and a recipe that lies loses against the octets. What has no machine code in it gets `arch=any` — the same octets on both machines, so the same hash, so **one store entry serves both**. Separate packages per machine, **not** a fat one: a fat package's hash would name two things and the choice of half would be made outside the plan. Measured (`tests/step-92-arch.sh`, 35 assertions, 0 failures, real toolchain via `firnc --target=`): the same source runs on both machines printing the same output, two different hashes; a wallpaper packed on two hosts is **identical, 173 018 octets**; an aarch64 plan rebuilds on an empty root into ARM binaries and the two plans differ in **exactly 2 app lines plus 2 arch lines**; six refusals each with its counter-check (wrong machine, unlabelled package, ambiguous source, fat package, dependency across the boundary, a plan that names the other machine's build — which refuses **without half-building**); one store holds both machines while `/apps` names one. On the real 73-package source, `arch=any` saves **346 977 octets (2.9 %)** of a two-machine source — small today because only 4 of 73 packages are machine-independent. **Open:** Osum does not run on AArch64 yet (round OSUM-ARM), so the ARM binaries measured here are Firn's under `qemu-aarch64` and not Osum programs; all 69 code packages are `x86_64`; the product ISO has no architecture. [docs/ARCHITECTURES.md](docs/ARCHITECTURES.md) |
| 6.1d | **Programs with no source** — "what about applications that are not in the app store, when I set up a new device?" | **built** (27.08.2026, third addendum to `plan2`). A plan may name **any number** of sources, so the recommended answer is a **source of your own** (a directory with an INDEX and an Ed25519 signature, on a NAS or a stick) — measured: one `source-add` takes a tree from `2 of 3 covered, 1 ORPHANED` to `3 of 3, 0 ORPHANED`. For everything else, **orphanhood is decided by computation, not remembered**: `opk.py orphans` asks every recorded source whether it has each hash and names what no source can give back (`--strict` exits 1, so it can be a nightly check). Exactly those packages, and only those, are carried in the backup — `opk.py backup-set` emits the typed list round TRESOR needs (`plan`/`secret`/`tree`/`package`, plus `# unreachable` when a source could not be read), and `opk.py vault-export` writes the octets out as `.opk` files, refusing if repacking the store entry does not give back the very same hash. `rebuild --vault` fetches by hash and needs **no signature**, because a question that names a hash checks its own answer — unlike a source, which is asked for a *name*. Measured (`tests/step-93-orphans.sh`, 29 assertions, 0 failures): a hand-installed package is reported orphaned, exported as **1 package / 4 228 octets** that is the original file octet for octet, restored on an empty root so that the whole tree matches **entry for entry over 29 entries**, and without the vault the rebuild **refuses by name** (`MISSING mytool 5eff0f81b5cf2844`) having built **nothing**. `--allow-missing` builds the rest and leaves `system/INCOMPLETE` in the tree, which `verify` then **fails** on. An orphan for another architecture gets its own message — the octets are the only ones in the world and cannot run there, so the way out is the source code, not the backup. **Open:** no `https://`, so the recommended private source cannot be reached over a network; a backup restores the current state, not the history. [docs/BACKUP.md](docs/BACKUP.md) |
| 6.2 | **Paketquelle** — ein Ort, von dem installiert wird; Signaturen | **gebaut** (26.08.2026). Ein Verzeichnis mit den `.opk`-Dateien, einem `INDEX` und einer Ed25519-Signatur darueber. Ed25519 steht ZWEIMAL da — eigene Umsetzung nach RFC 8032 und die Bibliothek des Wirts —, beide rechnen bei jedem Pruefen, und bei Uneinigkeit bricht das Werkzeug ab. Gemessen: veraenderter Index abgelehnt, unveraenderter angenommen, ein Paket das nicht zum signierten Index passt abgelehnt. **Offen:** der oeffentliche Schluessel entsteht bei jedem Bau neu und ist damit noch keine Herkunft, sondern nur ein Verfahren |
| 6.3 | **Certus** (der Browser; Befehl `/bin/browser`, Anzeigename und Fenstertitel "Certus", auf Android App-Name "Certus", Store-Titel "Certus Browser", Paket-ID `com.orientos.certus`) — entsteht im Firn-Repo; Runde B6 ist im Hauptbaum (`main`, Commit `ce28104bf`, Zweig `b6-layout`). **Stand 27.08.2026 (Runde B6), alles gemessen:** HTML-Baumbau und DOM **1837 / 1936 der html5lib-Tests (94,89 %)**; Layout gegen die offiziellen Web Platform Tests **97 / 186 (52,15 %)** — vorher 59 / 186 (31,72 %), Chromium 141 erreicht auf demselben Korpus 138 / 186; Zeichnen gegen die offiziellen Referenzbilder **202 / 541 (37,34 %)**, die 32 Paare, bei denen beide Seiten leer sind, getrennt ausgewiesen und NICHT mitgezählt; DOM aus JavaScript gegen die WPT **390 / 1714 Untertests (22,75 %)**, 169 Dateien, deren Prüfstand nicht fertig wurde, getrennt gezählt und nie als bestanden; HTTP/1.1 **28 / 28 Regeln** gegen Pythons `http.server`; **TLS 1.3 fertig und im Hauptbaum** — 647 / 647 Bausteinfälle gegen Pythons `cryptography` (49 Gegenproben), 26 / 26 Zertifikatsfälle (14 Ablehnungen mit dem RICHTIGEN Grund), 18 / 18 Handschläge gegen `openssl s_server` und das echte Netz (7 Ablehnungen, darunter ein Mann in der Mitte, der ein Bit kippt); JPEG gegen libjpeg (größte Kanalabweichung 3), `<img>`, und ein X11-Fenster mit Adresszeile, von der Serverseite mit `xwd` fotografiert (10 / 10). **Fingerabdruck-Abwehr (Kapitel Z) begonnen:** Leinwand-Rückleseweg und `navigator`-Felder verrauscht je Ursprung und je Sitzung, standardmäßig an, kein Schalter — 500 Ursprünge ergeben 500 verschiedene Leinwände, dieselbe Bahn ohne Rauschen genau 1, größte Farbabweichung 1. **Offen:** Grid (3 / 22), Schreibrichtungen (0 / 171), Grundlinienausrichtung, WebAudio und damit Z1/Z2, Filterlisten (Z5). Logbuch: `firn/docs/ROUNDB6.md` | in Arbeit |
| 6.4 | **Der Firn-Uebersetzer laeuft auf Osum selbst** — der eigentliche Pruefstein. Ein System, auf dem man sein eigenes System bauen kann, ist erwachsen; alles davor ist Kreuzuebersetzen von aussen. | **fertig** (K16). Osum hat auf sich selbst ein Firn-Programm uebersetzt, gebunden und ausgefuehrt, und das Ergebnis ist **Oktett fuer Oktett** dasselbe, was derselbe Uebersetzer auf Linux aus derselben Quelle macht: Assemblertext **14 909 Oktette zeichengleich**, ELF-Datei **8192 Oktette zeichengleich**. Dazu ein eigener Assembler und Binder in Firn, `kernel/user/fas.fi`, 2423 Zeilen — moeglich, weil der Uebersetzer nur einen kleinen Ausschnitt von x86-64 benutzt: ueber 1 533 513 Zeilen Assemblertext gemessen **58** Mnemoniken, **96** Paare aus Mnemonik und Operandenform, 8 Direktiven, 4 Speicherausdruecke, kein Indexregister, kein Massstab, kein Segment. `fas` bindet **54 von 54** Programmen des Userlands; 16 davon gegen `as`+`ld` gehalten, 16-mal gleiche Ausgabe. `.fi` ist doppelklickbar (`kernel/ftype.fi`, `kernel/user/firun.fi`, 242 Z.). Gemessen: `tools/k16/run.sh` **64 Zusagen, 0 Fehler** |
| 6.4a | **…aber `firnc` liegt noch nicht im OrientOS-Produkt.** Das ist die ehrliche Trennung zwischen „Osum kann es" und „das Produkt hat es" | **offen, und leicht zu uebersehen.** `vendor/osum/hole-osum.sh` baut die 69 Programme aus `kernel/user/*.fi`; darunter sind `fas` und `firun`, **nicht** aber `firnc` — der entsteht in K16 durch Querbauen von Firns `bin/firnc1.fi` gegen `kernel/user/user.ld` und braucht dafuer das Firn-Repo. Solange das nicht in `hole-osum.sh` steht, kann man auf einem gebooteten OrientOS ein `.s` assemblieren, aber keine `.fi` uebersetzen |

## 7 — Andere Maschinen

| | Was | Stand |
|---|---|---|
| 7.1 | **Die Architekturgrenze** — x86-Details liegen im Kernel ueberall statt an einer Stelle. Der Rust-Kernel dieses Repos HATTE diese Grenze (Traits plus ein Testschritt, der x86-Begriffe ausserhalb von `arch/` verbot); sie ist **nicht** nach Firn portiert worden, weil das eine Umbauarbeit an jedem Modul waere und keine Portierung. Die Anforderung steht als unuebersetzte Vorlage im Baum: `vorlage/arch_iface.rs`. Der eigentliche Inhalt ist nicht der Trait-Text, sondern die Regel. Siehe [KERNELWECHSEL.md](KERNELWECHSEL.md) § 4.1. | offen, mit Vorlage |
| 7.1b | **aarch64** — Firn uebersetzt bereits nach ARM (296 von 300 Faellen auf beiden Maschinen identisch), der Kernel nicht. Setzt 7.1 voraus. | offen |
| 7.2 | **Die Geraetewirklichkeit auf ARM** — kein PCI-Erkennungsweg wie beim PC, sondern Geraetebaum, und jeder SoC ist anders. Der eigentliche Aufwand. | offen |
| 7.3 | **Certus als App auf Android** (App-Name "Certus", Store-Titel "Certus Browser", Paket-ID `com.orientos.certus`) — der schnellste Weg, das Ergebnis in die Hand zu bekommen. Android ist Linux plus Bionic; eine APK darf native Bibliotheken enthalten. Also: Browserkern nach `aarch64-linux-android` uebersetzen, als `.so` einpacken, duenne Huelle fuer Zeichenflaeche und Beruehrung. Voraussetzung: der Browser ist fertig genug und laesst sich als Bibliothek herausloesen — beim Bauen von Anfang an mitdenken. | offen |

## 8 — Sicherheit und Haerte

| | Was | Stand |
|---|---|---|
| 8.1 | **SMEP/SMAP** — Ring 0 fuehrt keinen Nutzercode aus und fasst Nutzerdaten nur im `stac`-Fenster an. Auf JEDEM Kern (CR4 ist pro Prozessor), zurueckgelesen statt behauptet. Gegenproben: `smapraw` gibt mit dem Bit einen #PF und ohne es das Oktett. | **fertig** (26.08.2026, Osum `kernel/guard.fi`) |
| 8.2 | **Speicherverwuerfelung (KASLR)**, Schutzseiten, nicht ausfuehrbarer Stapel | offen |
| 8.3 | **Signierte Pakete und signierter Start** | **halb** — signierte Paketquelle steht (6.2, Ed25519 ueber den Index, zwei Umsetzungen). Signierter Start ist offen, und er ist die schwierigere Haelfte: er braucht eine Vertrauenskette, die vor dem Kernel anfaengt |
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
| `tools/osum/mkfs.py` | `load()` ruft `Fs(blocks, inodes)` — die Signatur ist `(blocks, version, inodes)` | kein Abbruch, sondern etwas Schlimmeres: jedes Abbild mit mehr als 128 Inodes wird mit falscher Geometrie gelesen. `data_start` steht dann auf 34 statt 130, also **auf der Inodetabelle**. Lesen geht (ein Inode traegt absolute Blocknummern), Schreiben wuerde die Tabelle ueberschreiben. Aufgefallen ist es erst, als die Paketrunde ein Abbild mit 512 Inodes brauchte |

Die ersten beiden sind Merge-Verluste und keine Absicht: der Elternteil `78f8e86`
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
