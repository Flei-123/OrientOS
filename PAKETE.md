# Das Paketformat von OrientOS — gebaut, nicht entworfen

[PACKAGING.md](PACKAGING.md) sagt, **was** OrientOS will und warum:
unveränderliche, inhaltsadressierte Pakete, drei Datentöpfe je App,
Systemgenerationen, keine Registry. Es ist ein Entwurfsdokument und war
bis zum 26.08.2026 mit dem Satz überschrieben, vom Verteilmodell sei
„noch **nichts** gebaut".

Diese Datei beschreibt, was seitdem **da** ist: das Format auf der
Platte, das Werkzeug, und — ebenso wichtig — wo der gebaute Stand vom
Entwurf abweicht und warum.

| | |
|---|---|
| Werkzeug | `pkg/opk.py` |
| Paketbau aus dem festgenagelten Osum-Stand | `pkg/bauen.sh` |
| Übersetzung in ein OFS-Abbild | `pkg/mkfs-spec.py` |
| Was ins Produkt kommt | `userland/PROGRAMME`, Zeilen `paket <name>` |
| Abnahme | `tests/step-80-pakete.sh` |
| Logbuch der Runde | `docs/RUNDE-PAKETE.md` |

> **NACHTRAG VOM 27.08.2026 (Runde PLAN2), auf Englisch — von dieser
> Runde an ist neue Dokumentation englisch.** The `PLAN` described in § 1
> below is no longer the whole story: it now has **typed lines** and
> carries the kernel, the package sources with their public keys, the
> system settings and the accounts, so that a machine can be rebuilt from
> it. Old `name<TAB>hash` files still parse. The format, the rejected
> alternatives and the honest limits are in
> [docs/PLAN-FORMAT.md](docs/PLAN-FORMAT.md); what a backup is, in
> [docs/BACKUP.md](docs/BACKUP.md); the measured numbers in
> [docs/ROUND-PLAN2.md](docs/ROUND-PLAN2.md). Package count went from 3
> to **69**, the kernel being one of them.

---

## 1. Der Store, wie er wirklich aussieht

```
/store/478632825585ffb1b863/     unveraenderlich, Name = Anfang des Hashes
    PAKET                        die Metadaten, Oktett fuer Oktett wie im Paket
    start                        die ausfuehrbare Datei
    INFO                         Anzeigename, Beschreibung, Schluesselwoerter
    symbol                       das Bild (OSYM, aus Osums Runde K15)
    daten/                       alles Weitere

/apps/explorer.prog/             die AKTIVIERTE Sicht der laufenden Generation
    start  INFO  symbol  daten/  -- jede Datei ein ZWEITER NAME auf den Store

/users/justin/config/explorer/   Einstellungen      -> Backup
/users/justin/state/explorer/    Dokumente          -> Backup
/users/justin/cache/explorer/    Vorschau, Indizes  -> NIE Backup

/system/AKTUELL                  die Nummer der laufenden Generation
/system/generations/3/PLAN       name<TAB>hash je Zeile, sortiert
/system/generations/3/GRUND      ein Satz, warum es diese Generation gibt
```

**Der ganze Zustand des Systems sind die `PLAN`-Dateien.** Sie sind Text,
sie sind sortiert, und man kann sie mit `cat` lesen. Das ist Regel 4 aus
PACKAGING.md („keine globale Registry, niemals") nicht als Absicht,
sondern als Bauweise: es gibt keinen zweiten Ort, an dem etwas stehen
könnte.

### `/apps/<name>.prog` und nicht `/apps/<name>`

PACKAGING.md zeichnet `/apps/editor -> /store/8f3a…c1`. Gebaut ist
`/apps/explorer.prog/` als **Verzeichnis mit zweiten Namen** darin, und
dafür gibt es zwei Gründe, die beide von außen kommen:

1. **OFS kennt keine symbolischen Verweise.** Der Inodetyp ist
   `T_FREE`, `T_FILE` oder `T_DIR` und sonst nichts (`kernel/fs.fi` im
   Osum-Repo, `mkfs.py` in `vendor/osum/`). Ein Pfeil auf ein
   Verzeichnis lässt sich in diesem Dateisystem nicht schreiben. Was es
   gibt, sind **harte Verweise** — zwei Verzeichniseinträge mit
   derselben Inode-Nummer —, und die gelten nur für Dateien.
2. **`.prog` ist das Bündelformat, das Osums Runde K15 schon benutzt.**
   Der Starter und der Datei-Explorer suchen `/apps/*.prog/` und starten
   `<bündel>/start`. Ein eigenes, zweites Layout danebenzustellen wäre
   eine Beschreibung an zwei Orten — einer zu viel.

Es kostet nichts: ein Bündel ist ein Verzeichnis mit vier
Verzeichniseinträgen, und die Oktette liegen **einmal** da. Gemessen im
Produkt-Abbild: 4 Dateien mit je zwei Namen, **236 703 Oktette nicht
doppelt abgelegt**.

### Der Name im Store ist ein Präfix, kein voller Hash

Ein Verzeichniseintrag in OFS ist 32 Oktette: acht für die Inode-Nummer,
**vierundzwanzig für den Namen**, davon einer für die Null. Ein SHA-256
in Hexziffern ist 64 Zeichen lang und passt nicht hinein. Der erste
Versuch, eine Paketwurzel in ein Abbild zu schreiben, endete wörtlich
mit

```
mkfs: the name '478632825585ffb1b8639f68da1bd3b0d132ea42e2cab51de820a320552d8f95' is too long
```

Der Ausweg ist **nicht**, den Hash zu ersetzen. Die Identität eines
Pakets bleibt die volle SHA-256: sie steht in jedem `PLAN`, sie wird bei
jeder Installation über den ganzen Inhalt nachgerechnet, und
`opk pruefen` rechnet sie für jeden Store-Eintrag noch einmal nach.
Verkürzt ist nur der **Verzeichnisname**, auf zwanzig Hexziffern = 80
Bit. Derselbe Handel, den Nix mit seinen 32 Base-32-Zeichen macht, an
eine kleinere Zahl angepasst.

Und die Kollision wird nicht weggehofft: liegt der kurze Name schon da,
liest `opk` das `PAKET` des vorhandenen Eintrags, rechnet dessen vollen
Hash nach und **bricht ab**, wenn er ein anderer ist.

---

## 2. Die Datei `<name>.opk`

```
Kopf, 64 Oktette
   0   8   Kennung  "OPKG0001"
   8   8   Laenge des Metadatenteils   (u64, klein zuerst)
  16   8   Laenge des Datenteils       (u64, klein zuerst)
  24  32   SHA-256 ueber (Metadaten || Daten)    <- der INHALTSHASH
  56   8   frei, 0
  64       Metadaten
           Daten
```

**Metadaten** sind Text, `schlüssel=wert`, eine Zeile je Feld, in fester
Reihenfolge:

```
name=explorer
fassung=1.0.0
titel=Datei-Explorer
info=Dateien und Ordner ansehen
keys=datei,dateien,explorer,ordner,verzeichnis,manager,file,files,folder
braucht=<name>            0..n, sortiert
handle=cache              0..n, sortiert
handle=config
handle=konsole
handle=state
```

`handle=` ist die Liste der Objekte, die die Anwendung beim Start
bekommt (PACKAGING.md § 7). Sie steht **im Paket** und nicht im
Quelltext der Anwendung — was eine App darf, entscheidet der, der sie
ausliefert, nicht sie selbst.

**Der Datenteil ist ein eigenes Archiv**, kein tar:

```
je Eintrag
   1   'd' Verzeichnis  |  'f' Datei
   2   Modus            (u16, klein zuerst)
   2   Laenge des Namens (u16)
   n   Name, UTF-8, ohne fuehrenden Schraegstrich
   8   Laenge des Inhalts (u64, 0 bei 'd')
   m   Inhalt
```

Sortiert über die **Oktette** des Namens. Kein Zeitstempel, kein
Eigentümer, kein Auffüllen, und der Modus kommt **nicht** vom Wirt: `755`
für `start`, `644` für alles andere.

**Warum kein tar.** tar trägt Zeit, uid, gid und Fülloktette mit sich.
Zwei Läufe über denselben Baum ergeben verschiedene Oktette, also
verschiedene Hashes — und dann ist „gleicher Hash = bitgleiche Software"
nur noch die Hälfte wahr: derselbe Inhalt hätte mehrere Namen, und der
Store liefe mit Einträgen voll, die dasselbe sind. Gemessen: zweimal
gebaut, **237 075 Oktette, Oktett für Oktett dasselbe**.

---

## 3. Was die Befehle tun

```
opk.py bauen <rezept> -o <datei.opk>
opk.py zeigen <datei.opk>
opk.py installieren  --wurzel W <datei.opk | name> [--quelle Q]
opk.py entfernen     --wurzel W <name> [--trotzdem] [--behalte-daten]
opk.py liste         --wurzel W
opk.py aktualisieren --wurzel W [<name>] --quelle Q
opk.py generationen  --wurzel W
opk.py zurueck       --wurzel W <n>
opk.py aufraeumen    --wurzel W [--behalte N]
opk.py pruefen       --wurzel W
opk.py baum          --wurzel W [--ohne <verzeichnis>]
opk.py verweise      --wurzel W
opk.py schluessel <verzeichnis>
opk.py quelle     <verzeichnis> [--schluessel <datei>]
```

**Installieren** ist: Hash nachrechnen → Abhängigkeiten prüfen →
Store-Eintrag in ein `.teil`-Verzeichnis schreiben und mit **einem**
`rename` sichtbar machen → neue Generation → `apps/` vollständig neu
bauen → die drei Töpfe anlegen. Es gibt keinen Zustand, in dem
`store/<hash>` halb da ist.

**Aktiviert wird immer vollständig neu**, nie fortgeschrieben. Ein
Zustand, den man nachzieht, hängt von seiner Geschichte ab; dann wäre
„Generation 7" nicht dasselbe wie „Generation 7, auf dem anderen Weg
erreicht". Es kostet Verzeichniseinträge und keinen einzigen Block.

**Entfernen** legt eine Generation ohne das Paket an, baut `apps/` neu
und löscht die drei Töpfe. Der **Store-Eintrag bleibt**, solange
irgendeine Generation ihn nennt — das ist keine Nachlässigkeit, sondern
die Bedingung dafür, dass man zurückrollen kann. `aufraeumen` wirft
Generationen weg und danach jeden Store-Eintrag, den keine übrige mehr
nennt (Erreichbarkeit, nicht Referenzzählung — PACKAGING.md § 8 nennt
sie robuster, und bei ein paar Textdateien ist sie auch die billigere).

---

## 4. Die Quelle und ihre Signatur (Roadmap 6.2)

Eine Quelle ist ein Verzeichnis mit den `.opk`-Dateien, einem `INDEX`
(`name TAB fassung TAB hash TAB groesse TAB datei`) und `INDEX.sig`,
einer Ed25519-Signatur über die Oktette des `INDEX`.

Die Schichtung ist wichtig und wird hier ausgeschrieben, weil sie sonst
verwechselt wird:

| Was | Wogegen es schützt | Woran es hängt |
|---|---|---|
| Der Hash **im Paket** | Übertragungsfehler, kaputte Platte | an nichts — er ist nachrechenbar |
| Der Hash **im INDEX** | ein ausgetauschtes Paket | an der Echtheit des INDEX |
| Die **Signatur** über den INDEX | ein ausgetauschter INDEX | am öffentlichen Schlüssel |

Ein Paket allein beweist nur, dass es unversehrt ist — nicht, woher es
kommt. Das steht schon in PACKAGING.md § 8 („Der Hash beweist
Unverändertheit, nicht Herkunft") und ist genau so gebaut.

**Ed25519 steht zweimal da.** Einmal als eigene Umsetzung nach RFC 8032
in `pkg/opk.py`, einmal als Aufruf der Bibliothek des Wirts
(`cryptography`). Beide rechnen bei jedem Signieren und jedem Prüfen,
und wenn sie sich uneins sind, bricht das Werkzeug mit dieser Meldung
ab statt sich für eine zu entscheiden. Derselbe Gedanke, aus dem
`mkfs.py` die zweite Umsetzung des Dateisystems ist: eine Prüfung, die
immer „ja" sagt, fällt bei keinem gültigen Paket auf.

---

## 5. Was NICHT gebaut ist, und was anders kam als im Entwurf

Das gehört hierher und nicht in eine Fußnote.

* **`opk` läuft auf dem Wirt, nicht auf Osum.** Es ist ein
  Python-Programm; das gebootete System hat kein Python. Ein `/bin/opk`
  in Firn wäre die ehrliche Fortsetzung — Osum hat seit Runde K13
  SHA-256 in Firn (`kernel/user/pw.fi`) und seit K14 ein VFS, es fehlt
  also nicht das Werkzeug, sondern die Runde. **Was das gebootete System
  heute kann:** den Store lesen, die Bündel benutzen, ein Paket starten.
  Was es nicht kann: installieren, entfernen, zurückrollen.
* **Die Generation ist keine Bootoption.** PACKAGING.md § 5 verspricht
  „Bootmenü-Eintrag wählen". Gebaut ist ein Verweis in einer Datei
  (`system/AKTUELL`), umgelegt vom Wirt. Ein Bootmenü mit Generationen
  setzt einen Schreibpfad auf eine echte Platte voraus (ROADMAP 9.4),
  und den gibt es nicht.
* **Kein Copy-on-Write.** PACKAGING.md § 5 nennt CoW-Schnappschüsse
  Voraussetzung für Generationen. Gebraucht werden sie hier nicht, weil
  eine Generation nur eine Textdatei ist und die aktivierte Sicht aus
  harten Verweisen besteht. Was ohne CoW **fehlt**, ist das Zurückrollen
  von Nutzerdaten — siehe den nächsten Punkt.
* **Nutzerdaten rollen nicht mit.** Eine Generation beschreibt
  installierte **Pakete**, nicht die Dokumente des Nutzers: ein
  Rücksprung auf gestern darf nicht die Arbeit von heute wegnehmen.
  Die Kehrseite steht ausdrücklich da: `entfernen` wirft die drei Töpfe
  wirklich weg (PACKAGING.md § 1.3), und ein späteres `zurueck` bringt
  sie **nicht** wieder. Wer das nicht will, nimmt `--behalte-daten`.
* **Kein Nachladen von Abhängigkeiten.** Was fehlt, wird benannt und
  nicht besorgt. Ein Auflöser für einen Abhängigkeitsgraphen ist eine
  eigene Runde, und dieses Userland hat noch keine echte Abhängigkeit:
  jedes Programm aus Osum ist statisch gebunden. Gemessen wird die
  Prüfung deshalb an zwei eigens gebauten Paketen — was sie zeigt, ist
  die Prüfung, nicht ein echter Fall.
* **Kein Aktualisieren zur Laufzeit.** Eine neue Fassung bekommt einen
  neuen Store-Eintrag und eine neue Generation; ein laufender Prozess
  hält weiter seinen alten. PACKAGING.md § 8 hält das für die
  ehrlichere Antwort, und so ist es gebaut.
* **`bin/` bleibt unberührt.** Ein Paket legt dort nichts ab.
  PACKAGING.md § 6 nennt Kompatibilitätsverweise nach `/usr/bin`
  ausdrücklich das, was man von GoboLinux **nicht** übernimmt — „die
  Hintertür, die alles wieder aufweicht". Was unter `/bin` liegt, kommt
  aus `userland/PROGRAMME` und ist das alte, ungepackte Userland.
* **Drei Pakete im Produkt, nicht fünf.** Ein Abbild hat 4096 Blöcke zu
  512 Oktetten. Der Datei-Explorer allein ist 234 448 Oktette.

---

## 6. Was gemessen ist

`tests/step-80-pakete.sh`, zwei Schritte, **35 Zusagen** (29 auf dem
Wirt, 6 im gebooteten System). Jede Zusage hat ihre Gegenprobe, und die
Gegenproben zu den Gegenproben stehen daneben: dass ein **unversehrtes** Paket angenommen wird, dass sich der
Baum durch eine Installation **wirklich ändert**, dass die
Abhängigkeitsprüfung nicht grundsätzlich ablehnt. Ohne sie wäre ein
Werkzeug, das gar nichts tut, hier grün.

Die Zahlen stehen in [docs/RUNDE-PAKETE.md](docs/RUNDE-PAKETE.md).
