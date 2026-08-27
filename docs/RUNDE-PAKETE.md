# Runde „Pakete" — Roadmap 6.1 und 6.2

**Stand:** 26.08.2026 · Zweig `pkg` in OrientOS, Kernel festgenagelt auf
Osum `c5fe12f` · Abnahme: `./test.sh` → **ALLE 14 SCHRITTE BESTANDEN,
201 Zusagen**, 0 Fehler (davor 11 Schritte, 151 Zusagen)

> **`.opk` is what you hold, `.osp` is what runs.** Das Paket ist die
> Datei, die man haelt; das Buendel unter `/apps/<name>.osp/` ist das,
> was laeuft. (Die Endung hiess bis zur Runde `rename` `.prog`.)

Die 50 dazugekommenen Zusagen, aufgeschlüsselt: 13 im Patchschritt
(`step-05`), 29 + 6 in den beiden Paketschritten (`step-80`) und 2 in
`step-20`, das jetzt zwischen einem kopierten Programm unter `/bin` und
einem installierten Paket unter `/apps` + `/store` unterscheidet. Keine
einzige bestehende Zusage ist verloren.

**Eine Zahl, die zweimal gemessen werden musste, und warum sie hier
steht.** Der abschließende Lauf aus einem leergeräumten Baum
(`rm -rf build vendor/osum/bin …`) meldete **201 Zusagen mit einem
Fehler**: Schritt 10, `'smapraw' bleibt stehen, aber nicht an einem
#PF`. Das Protokoll dieses Laufs bricht mitten in der Speicherprüfung
ab, lange vor Ring 3 — die QEMU-Maschine ist in ihr Zeitlimit gelaufen.
Grund war die Maschine, nicht das Produkt: auf demselben Rechner lief
gleichzeitig die Abnahme der Kernelrunde K18 (`bash ./test.sh` in
`osum-k18-power`), Lastmittel **13,9**. Derselbe Schritt allein, direkt
danach: **34 Zusagen, 0 Fehler**, `'smapraw' endet in einem #PF
(err=0x1) — SMAP setzt wirklich durch`. Der Stand, der oben zählt, ist
der Lauf ohne diese Fremdlast. Das steht hier, weil eine Zahl, die man
zweimal messen musste, dazugehört.

Diese Runde hat zwei Hälften, und die erste war nicht geplant.

---

## Teil 1 — Nachziehen, und der Stand, der nicht übersetzt

### Was hätte passieren sollen

`vendor/osum/COMMIT` von `7a53ac3` auf `c5fe12f` setzen,
`hole-osum.sh` laufen lassen, `./test.sh` fahren. Vier Runden Kernel
lagen dazwischen — K13 (Benutzer, Rechte, `/sbin/init` als Prozess 1),
K14 (VFS, `/proc`, `/dev`, FAT32, Partitionstafeln), K15 (Widgets,
`/bin/explorer`, `.osp`-Bündel, Namensindex), K16 (der Firn-Übersetzer
läuft auf Osum selbst). `git diff --stat 7a53ac3 c5fe12f`: 103 Dateien,
**32 525 eingefügte** und 267 gelöschte Zeilen.

### Was passiert ist

```
>> Kernel bauen (tools/build-kernel.sh aus dem Osum-Repo)
error: 'fn' is only allowed at top level, not inside a function body
     --> kernel/kmain.fi:2484:1
```

**Der Merge-Commit `c5fe12f` übersetzt nicht.** Zwei Stellen sind beim
Verschmelzen von `k15-ui` verlorengegangen:

1. **`kernel/kmain.fi`, eine schließende Klammer.** In `mode_of` öffnet
   Zeile 2394 ein `if` für `M_NOSTACK`, und der K15-Block ist genau an
   die Stelle der schließenden Klammer geraten. Nachgerechnet: die
   Klammerbilanz der Datei endet bei +1 statt 0, und **13 Funktionen**
   beginnen dadurch auf der falschen Ebene.
2. **`kernel/sys.fi`, die K15-Naht in der falschen Funktion.** Die
   Aufrufe 1800..1806 landeten in `k13_call` statt in `dispatch`.
   `k13_call` nimmt `a0..a3` entgegen, die Naht braucht fünf Argumente
   → `unknown name 'a4'`. Und selbst wenn es übersetzt hätte, wäre der
   Zweig **tot** gewesen: er stünde hinter `if is_k13(number)`, und
   `is_k13` zählt die Linux-Nummern 21, 90..95, 102..108, 113..120, 169,
   260..269 und 1600 auf. 1800..1806 sind nicht darunter. **Kein
   Widget-Aufruf hätte diese Stelle je erreicht.**

Dass es Merge-Verluste sind und keine Absicht, steht in den Eltern:
`git show 78f8e86:kernel/kmain.fi` (Zeilen 2387..2389) hat den Block
vollständig, `git show fcb5c30:kernel/sys.fi` (Zeilen 785..792) hat die
Naht in `dispatch`.

### Und ein dritter Fund, der später kam

Als die Paketrunde ein Abbild mit mehr als 128 Inodes brauchte, fiel
`tools/osum/mkfs.py` auf:

```python
inodes = struct.unpack_from("<Q", raw, SB["INODES"])[0] or INODE_COUNT
fs = Fs(max(...), inodes)          # Signatur ist (blocks, version, inodes)
```

Die Inodezahl aus dem Superblock landet im Feld `version`, und `inodes`
fällt auf die Vorgabe 128 zurück. Folge: `mkfs.py list` meldet
`inodes=85/128` statt `85/512`, und `self.data_start` steht auf **34
statt 130** — also mitten auf der Inodetabelle. Lesen geht trotzdem, weil
ein Inode absolute Blocknummern trägt; **Schreiben würde die Tabelle
überschreiben.** Dass aus `version = 512` kein weiterer Schaden entsteht,
ist Zufall: geprüft wird `version >= OFS_V2`, und 512 ist größer.

### Wie es berichtigt ist — und warum nicht im Kernelrepo

Ein verschobener Nagel ist ein anderer Stand und entwertet jede Messung,
die gegen den alten gemacht wurde. Der Nagel bleibt deshalb `c5fe12f`,
und die drei Stellen werden **beim Auspacken** berichtigt:

```
vendor/osum/patches/0001-kmain-merge-klammer.patch
vendor/osum/patches/0002-sys-wig-naht-in-dispatch.patch
vendor/osum/patches/0003-mkfs-load-inodezahl.patch
```

`hole-osum.sh` wendet sie an und **bricht ab**, wenn eine nicht mehr
passt. Jeder Patch trägt im Kopf, *was* er berichtigt, *warum* es ein
Fehler ist und *woran* man sieht, dass er überflüssig geworden ist.

Ein Patchstapel ist der Ort, an dem still etwas verrottet — deshalb steht
er unter Messung (`tests/step-05-patches.sh`, 13 Zusagen). Der Kern
davon: **jeder Patch wird einzeln wieder herausgenommen**, und dann muss
die Probe umfallen. Für Kernelquelltext ist die Probe ein
`firnc`-Lauf über `kernel/kmain.fi` (rund zwei Sekunden); ein Patch an
einem Werkzeug schreibt sich seine eigene in eine Zeile `Probe:` im
Kopf.

```
[ ok ] mit allen 3 Berichtigung(en) uebersetzt der Kernel (1988416 Oktette)
[ ok ] GEGENPROBE: ohne 0001 bricht firnc ab (error: 'fn' is only allowed at top level)
[ ok ] GEGENPROBE: ohne 0002 bricht firnc ab (error: unknown name 'a4')
[ ok ] GEGENPROBE: ohne 0003 faellt seine eigene Probe durch
```

### Unabhängig bestätigt, dreimal derselbe Befund

Dieselben zwei Merge-Schäden sind am selben Tag von den beiden laufenden
Kernelrunden gefunden worden, jede für sich:

| Zweig | Commit | Text |
|---|---|---|
| `k17-usb` | `93ad914` | „K17 Vorarbeit: zwei Merge-Schaeden aus c5fe12f behoben, sonst baut nichts." |
| `k18-power` | `eed1edd` | „K18, Vorarbeit: main liess sich nicht uebersetzen — zwei Reste der K15-Verschmelzung." |

Drei Befunde, dieselbe Klammer. K17s Fassung ist Zeichen für Zeichen
dieselbe wie die hier; bei `sys.fi` liegt der Block bei K17 ein paar
Zeilen tiefer (unmittelbar vor `if is_k13`), hier unmittelbar hinter der
K12-Naht — dazwischen steht nur ein Kommentar, das Verhalten ist
gleich. Sobald einer dieser Zweige nach `main` geht, passt der Patch
nicht mehr, der Lauf wird rot, und die Datei gehört gelöscht, nicht
angepasst.

### Eine Zahl, die zwanzig Läufe lang falsch war

`userland/PROGRAMME` stand auf `bloecke: 6144`. Die Blockkarte von OFS
ist **ein** Block zu 512 Oktetten, also 4096 Bits, also höchstens 4096
Blöcke je Abbild. Bis K15 hat `mkfs.py` das nicht geprüft und 6144
klaglos gebaut — die oberen 2048 Blöcke (1 MiB) waren dabei nicht
adressierbar, weil kein Bit sie beschreibt. Aufgefallen ist es nie, weil
das Produkt sie nie brauchte: 27 Programme, 932 584 Oktette, **1822 von
4062 Datenblöcken**. Seit K15 lehnt `mkfs.py` 6144 ab, und das ist
richtig so.

### Der Stand nach dem Nachziehen

```
############ ALLE 12 SCHRITTE BESTANDEN, 160 Zusagen ############
```

Vorher: 11 Schritte, 151 Zusagen. Die neun dazugekommenen sind der
Patchschritt. Keine einzige bestehende Zusage ist verloren. Gebaut:
`osum.mb` 1 639 256 Oktette, **69 Programme** (vorher 51),
`build/orientos.iso` 7424 KiB.

Danach ist `ROADMAP.md` berichtigt worden — sieben Punkte auf *fertig*,
jeder am Baum von `c5fe12f` nachgeprüft und die Zeilenzahlen mit `wc -l`
nachgezählt statt aus den Logbüchern übernommen. Dabei kamen drei
Abweichungen heraus: `wlib.fi` hat 2843 Zeilen und nicht die 2688 aus
`ROUNDK15.md`, `wig.fi` 499 statt 484, `explorer.fi` 1013 statt 939 —
die Nachträge der Runde sind in den Tabellen des Logbuchs nicht
nachgeführt worden.

Und eine Zahl aus dem Auftrag stimmte nicht: der Namensindex ist gegen
den Baumdurchlauf um **Faktor 282** schneller (Suchwort `kupfer`), nicht
332. Für `07` sind es 191, für `quaste` 261. Eingetragen ist, was
gemessen wurde.

---

## Teil 2 — Die Paketverwaltung

### Die drei Entscheidungen, aus denen der Rest folgt

**Keine Zeitstempel, nirgends.** Weder im Paket noch in einer
Generation. Ein Zeitstempel macht zwei gleiche Vorgänge unterscheidbar,
und dann lässt sich „nach Installieren und Entfernen ist alles wie
vorher" nicht mehr messen, sondern nur behaupten. Deshalb auch kein
`tar`: tar trägt Zeit, uid, gid und Fülloktette mit sich, und zwei Läufe
über denselben Baum ergeben verschiedene Oktette. Der Datenteil eines
Pakets ist ein eigenes, sortiertes Archiv von fünfzehn Zeilen Code.

**Harte Verweise statt Kopien.** `apps/explorer.osp/start` ist ein
zweiter Name auf dieselbe Datei im Store — dasselbe Verfahren, mit dem
Osums K15 `/bin/explorer` und `/bin/files` verbindet, und dasselbe, das
`mkfs.py` mit `<neu>@<vorhanden>` in ein OFS-Abbild schreibt. Auf dem
Wirt ist es `os.link`, und dass es wirklich derselbe Inode ist, wird über
`st_ino` gemessen und nicht angenommen.

**Aktiviert wird vollständig neu, nie fortgeschrieben.** Ein Zustand,
den man nachzieht, hängt von seiner Geschichte ab; dann wäre
„Generation 7" nicht dasselbe wie „Generation 7, auf dem anderen Weg
erreicht". Es kostet Verzeichniseinträge und keinen einzigen Block.

### Was der Entwurf nicht vorhergesehen hatte

**OFS kennt keine symbolischen Verweise.** PACKAGING.md zeichnet
`/apps/editor -> /store/8f3a…c1`. Den Pfeil gibt es in diesem
Dateisystem nicht: der Inodetyp ist `T_FREE`, `T_FILE` oder `T_DIR`.
Gebaut ist deshalb `/apps/<name>.osp/` als Verzeichnis mit zweiten
Namen darin — was auch der Form entspricht, die Osums Starter und
Datei-Explorer ohnehin suchen.

**Und ein SHA-256 passt nicht in einen Verzeichniseintrag.** Der erste
Versuch, eine Paketwurzel in ein Abbild zu schreiben, endete wörtlich
mit

```
mkfs: the name '478632825585ffb1b8639f68da1bd3b0d132ea42e2cab51de820a320552d8f95' is too long
```

Ein Eintrag ist 32 Oktette: acht für die Inode-Nummer, vierundzwanzig
für den Namen. Der Ausweg ist nicht, den Hash zu ersetzen — die
Identität bleibt die volle SHA-256, sie steht in jedem `PLAN` und wird
bei jeder Installation nachgerechnet. Verkürzt ist nur der
Verzeichnisname, auf zwanzig Hexziffern = 80 Bit. Und die Kollision wird
nicht weggehofft: liegt der kurze Name schon da, rechnet `opk` den
vollen Hash des vorhandenen Eintrags nach und **bricht ab**, wenn er ein
anderer ist.

### Die Gegenproben, die verlangt waren

**1. Installieren, entfernen, und der Baum ist wieder wie vorher.**
Verglichen wird nicht „hat keinen Fehler gemeldet", sondern eine
kanonische Beschreibung des Baums: Art, Pfad, Modus, Größe und die
SHA-256 jedes Inhalts, sortiert.

| | |
|---|---:|
| Einträge im Baum vorher | **23** |
| davon durch die Installation verändert | **16** |
| Einträge im Baum nachher | **23**, Zeile für Zeile dieselben |
| Generationen, die übrigbleiben | 1 (Nr. 3) |

Die mittlere Zeile ist die Gegenprobe **zum Vergleich selbst**: wäre der
Baum durch eine Installation unverändert, vergliche der Test nichts. Und
die erste ist die Gegenprobe gegen die Warnung aus den früheren Runden —
ein Vergleich zweier leerer Bäume geht immer durch, deshalb steht die
Zeilenzahl in der Zusage und eine Zahl unter 15 lässt sie fallen.

Was **nicht** verschwindet, steht daneben statt weggeschrieben zu
werden: die Generationsnummern zählen weiter. `system/generations/3/`
statt `.../1/`, mit demselben `PLAN`-Inhalt. Das ist ein Protokoll und
kein Rest.

**2. Ein Paket mit falscher Prüfsumme wird abgelehnt.** An drei Stellen
gemessen, weil eine allein nicht reicht:

| beschädigt in | Ergebnis |
|---|---|
| den **Daten** (Oktett 70 000 gekippt) | abgelehnt: „im Kopf steht 478632825585…, gerechnet 40c85ebf…" |
| den **Metadaten** (Oktett 70) | abgelehnt: „gerechnet 57a308e2…" |
| dem **genannten Hash** selbst (Oktett 24) | abgelehnt: „im Kopf steht 468632825585…, gerechnet 478632825585…" |
| **gar nicht** (dieselbe Datei, unversehrt) | **angenommen** |

Die dritte Zeile ist die interessante: dort wird nicht der Inhalt
verändert, sondern der Hash. Ein Werkzeug, das die Zahl nur
weiterreicht, statt sie nachzurechnen, fällt genau hier durch. Die
vierte ist die Gegenprobe zur Gegenprobe — ohne sie wäre ein Werkzeug,
das grundsätzlich nichts annimmt, hier grün.

**3. Eine Generation zurückrollen.**

```
    0   0 Paket(e)  leer
    1   1 Paket(e)  installiert terminal 1.0.0
    2   2 Paket(e)  installiert explorer 1.0.0
    3   3 Paket(e)  installiert widgets 1.0.0
*   4   2 Paket(e)  entfernt explorer
```

`zurueck 2` → `apps/` ist Eintrag für Eintrag (**13 Einträge**) wieder
der Stand von Generation 2. Dass die Zusage etwas misst, steht daneben:
Generation 4 unterscheidet sich nachweislich von 2, sonst verglichen
beide Seiten dasselbe. Der Store hält weiter alle **3** Einträge, auch
den gerade unbenutzten — ohne das wäre ein Rückrollen nicht möglich.

**Was NICHT mitrollt, und das ist die unangenehme Stelle dieses
Entwurfs.** Nutzerdaten gehören keiner Generation: ein Rücksprung auf
gestern darf die Dokumente von heute nicht wegnehmen. Die Kehrseite ist,
dass `entfernen` die drei Töpfe wirklich wegwirft (PACKAGING.md § 1.3
verlangt es so) und ein späteres `zurueck` sie **nicht** wiederbringt.
Das steht jetzt in der Ausgabe von `zurueck`, damit es niemand erst
merkt, wenn es weh tut, und es gibt `--behalte-daten` für den, der es
anders will.

### Die vierte Gegenprobe, die niemand verlangt hatte

Der Store soll unveränderlich sein. Als root halten Modusbits niemanden
auf — eine Zusage „ein Schreibversuch in den Store scheitert" wäre in
dieser Umgebung genau die Art von Test, vor der die Aufgabe warnt. Was
wirklich schützt, ist Nachrechnen:

```
[ ok ] pruefen: 1 Eintrag/Eintraege, 0 kaputt, 0 verwaist, 0 fehlend
[ ok ] GEGENPROBE: ein Oktett mehr im Store -> KAPUTT store/478632825585ffb1b863
```

### Signaturen (6.2)

Eine Quelle ist ein Verzeichnis mit den Paketen, einem `INDEX` und einer
Ed25519-Signatur über dessen Oktette. **Ed25519 steht zweimal da**:
einmal als eigene Umsetzung nach RFC 8032 (rund 90 Zeilen in
`pkg/opk.py`), einmal als Aufruf der Bibliothek des Wirts. Beide
rechnen bei jedem Signieren und jedem Prüfen; sind sie sich uneins,
bricht das Werkzeug mit dieser Meldung ab, statt sich für eine zu
entscheiden. Derselbe Gedanke, aus dem `mkfs.py` die zweite Umsetzung
des Dateisystems ist.

| geprüft | Ergebnis |
|---|---|
| Signatur des Index, eigene Umsetzung | anerkannt |
| dieselbe Signatur, `cryptography` des Wirts | anerkannt |
| Index um eine Zeile ergänzt | **abgelehnt** — „die Signatur des Index passt nicht" |
| Index unverändert | angenommen |
| Paket im Verzeichnis um ein Oktett verändert | **abgelehnt** — passt nicht zum Hash im signierten Index |

Die Schichtung ist absichtlich sichtbar: der Hash im Paket schützt gegen
Übertragungsfehler, der Hash im Index gegen ein ausgetauschtes Paket,
die Signatur gegen einen ausgetauschten Index. PACKAGING.md § 8 sagt es
vorweg — „der Hash beweist Unverändertheit, nicht Herkunft".

### Und dann läuft es wirklich

Alles bisherige läuft auf dem Wirt. Die Frage, um die es geht, ist, ob
aus einem installierten Paket auf der gebauten Platte ein **Prozess**
wird. Aus dem seriellen Anschluss der Maschine, wörtlich:

```
osum$ ls /apps
./ ../ explorer.osp/ hallo.osp/ widgets.osp/
osum$ ls /store
./ ../ 478632825585ffb1b863/ 73d3f6f3c4e8256de0b4/ e9cc64391932e1ed26f1/
osum$ cat /system/AKTUELL
3
osum$ /apps/hallo.osp/start
hello: argc1
hello: arg 0 /apps/hallo.osp/start
hello: bss 0
hello: data 3735928559
hello: ro 424242
osum$ wc -c /apps/explorer.osp/start
234448 /apps/explorer.osp/start
```

`hallo` ist im Produkt, weil die beiden großen Kandidaten aus K15 — der
Datei-Explorer und die Widget-Anwendung — einen Bildschirm brauchen.
Dass sie **installiert** sind, lässt sich an ihren Oktetten messen; dass
ein installiertes Paket **läuft**, nicht. Ein Programm, das eine Zeile
schreibt und sich beendet, kann beides.

Und die Zahl in der letzten Zeile ist der Punkt: **234 448** Oktette,
vom Gastsystem gezählt, dieselbe Zahl wie `stat -c%s
vendor/osum/bin/explorer` auf dem Wirt. Der Weg — Paket bauen, Hash
rechnen, in einen Store installieren, harte Verweise legen, in ein
OFS-Abbild übersetzen, als Multiboot-Modul booten — hat kein Oktett
verändert.

Dass die Datei dabei nur **einmal** auf der Platte liegt, ist ebenfalls
gemessen: 1139 Blöcke frei, und ein zweites Exemplar des Explorers
kostete 458.

### Die Zahlen des Produkts

| | vorher | nachher |
|---|---:|---:|
| Blöcke im Abbild | 4096 | 4096 |
| davon frei | 2191 | **1139** |
| Inodes benutzt / Tabelle | 32 / 128 | **85 / 512** |
| Bündel unter `/apps` | 0 | **3** |
| Einträge unter `/store` | 0 | **3** |
| ISO | 7424 KiB | 7424 KiB |

---

## Was nicht geklappt hat

*Der Reihe nach, wie es aufgetreten ist.*

1. **Der Kernel übersetzte nicht.** Erwarteter Aufwand für Teil 1: ein
   `COMMIT` ändern und ein Skript laufen lassen. Tatsächlich: drei
   Patches, ein neuer Testschritt und ein halber Nachmittag. Die
   Vorrichtung dafür (`vendor/osum/patches/`) ist der bleibende Teil
   davon; die drei Dateien darin sollen wieder verschwinden.
2. **`mkfs.py` lehnte 6144 Blöcke ab** — und hatte recht. Zwanzig
   Läufe lang war die Zahl im Produkt falsch, ohne dass es je auffiel.
3. **Der SHA-256 passte nicht in einen Verzeichniseintrag.** Der ganze
   Store musste auf verkürzte Namen umgestellt werden, nachdem er
   fertig war. Hätte man `kernel/fs.fi` vorher gelesen statt
   PACKAGING.md, wäre es vor dem Bauen aufgefallen.
4. **Die Inodetabelle war zu klein**, und dabei fiel der dritte Defekt
   im festgenagelten Stand auf. Ohne die Paketrunde hätte ihn niemand
   gesucht: ein Abbild mit 128 Inodes trifft ihn nicht.
5. **Die Gegenprobe zum dritten Patch ging fälschlich grün durch.** Der
   Testschritt holte beim Zurücknehmen eines Patches nur Dateien unter
   `kernel/` zurück — `tools/osum/mkfs.py` blieb berichtigt, und die
   Probe bestand natürlich. Genau die Art von Test, vor der die Aufgabe
   warnt: er misst nicht, was er zu messen behauptet. Jetzt wird von
   **jeder** betroffenen Datei eine unberührte Fassung beiseitegelegt,
   bevor der erste Patch angewandt wird.
6. **Die Rezepte der Abhängigkeitsprobe fanden ihre Dateien nicht.**
   Ein `datei=`-Eintrag ist relativ zum Rezept; die Probe-Rezepte liegen
   in einem Wegwerfordner, `../../vendor/osum/bin/true` zeigte dort ins
   Leere. Der Fehler war nicht schlimm, aber die Zusage darüber war es:
   „ohne die Abhängigkeit abgelehnt" ging grün durch, **weil das Paket
   gar nicht existierte**. Zwei leere Seiten, wieder. Jetzt wird
   zusätzlich zugesagt, dass sich das Hilfspaket überhaupt bauen lässt.

---

## Was offen bleibt

* **`opk` läuft auf dem Wirt, nicht auf Osum.** Das gebootete System
  liest den Store, benutzt die Bündel und startet ein Paket; es kann
  nicht installieren, entfernen oder zurückrollen. Ein `/bin/opk` in
  Firn wäre die ehrliche Fortsetzung — Osum hat seit K13 SHA-256 in
  Firn (`kernel/user/pw.fi`, 838 Zeilen) und seit K14 ein VFS, es fehlt
  also nicht der Unterbau, sondern die Runde.
* **Die Generation ist keine Bootoption.** PACKAGING.md § 5 verspricht
  „Bootmenü-Eintrag wählen". Gebaut ist ein Verweis in einer Datei,
  umgelegt vom Wirt. Ein Bootmenü mit Generationen setzt einen
  Schreibpfad auf eine echte Platte voraus (ROADMAP 9.4).
* **Keine echte Abhängigkeit im Userland.** Jedes Programm aus Osum ist
  statisch gebunden. Die Prüfung ist an zwei eigens gebauten Paketen
  gemessen — sie zeigt die Prüfung, nicht einen echten Fall.
* **Der Hypervisor aus K12 ist kein Paketkandidat**, anders als die
  Aufgabe annahm: es gibt in `kernel/user/*.fi` kein Ring-3-Programm
  dafür. Der Gastteil liegt in `kernel/uprog.fi` und damit **im
  Kernel**; `vendor/osum/bin/` enthält kein `vm`, `hv` oder ähnliches.
  Paketiert sind deshalb die Programme aus K15 (Explorer, Widgets) und
  `hallo`.
* **Der öffentliche Schlüssel entsteht bei jedem Bau neu.** Für eine
  Quelle, die wirklich verteilt, wäre das falsch — dann muss der
  öffentliche Teil fest sein, sonst hat die Signatur keinen Wert.
  Solange die Quelle im Bauverzeichnis liegt, ist ein Schlüssel je Lauf
  ehrlicher als einer, der im Repo liegt und damit niemandem gehört.
* **Kein Store-GC über mehrere Wurzeln, kein Deduplizieren unterhalb
  ganzer Dateien.** PACKAGING.md § 8 nennt beides; hier ist nichts
  davon gebaut.
* **Zwei der fünf gebauten Pakete sind nicht im Produkt** (`editor`,
  `suchen`). Sie ließen sich einbauen; es fehlt der Platz, nicht die
  Technik.
