# Software-Verteilung in OrientOS

Entwurfsentscheidung, festgelegt. Dieses Dokument beschreibt das **Ziel** und
begründet es. Was der Kernel als Unterbau mitbringt, steht in § 7 und ist seit
Phase 3 mehr als eine Absichtserklärung.

> **NACHTRAG VOM 26.08.2026.** Bis zu diesem Tag stand hier, vom
> Verteilmodell sei „noch **nichts** gebaut — dafür fehlt das
> Dateisystem". Das Dateisystem steht seit Osums Runde K14, und das
> Modell ist seitdem gebaut: `pkg/opk.py`, Format und Abweichungen vom
> Entwurf in [PAKETE.md](PAKETE.md), Zahlen in
> [docs/RUNDE-PAKETE.md](docs/RUNDE-PAKETE.md). Dieses Dokument bleibt
> der **Entwurf** und wird nicht nachträglich passend gemacht: wo der
> gebaute Stand abweicht — `/apps/<name>.prog/` statt eines Verweises,
> ein verkürzter Store-Name, kein Bootmenü mit Generationen —, steht das
> in PAKETE.md § 1 und § 5 mit dem Grund daneben.

---

## 1. Die vier Regeln

1. **Eine App ist ein einziges, unveränderliches Verzeichnis, benannt nach dem
   Hash ihres Inhalts.** Nach der Installation wird daran nie wieder etwas
   geändert. Gleicher Hash = bitgleiche Software, überall.
2. **Nutzerdaten liegen strikt getrennt in drei Töpfen je App:**
   `config` (Backup), `state` (Backup), `cache` (**nie** Backup, jederzeit
   löschbar).
3. **Deinstallieren heißt: zwei Verzeichnisse löschen.** Reste sind per
   Konstruktion unmöglich, weil es gar keinen Ort gibt, an dem sie liegen
   könnten.
4. **Es gibt keine globale Registry. Niemals.** Kein zentraler Zustand, den
   eine Installation beschädigen kann.

---

## 2. Wie das aussieht

```
/store/
  8f3a…c1/                      unveränderlich, Name = Hash des Inhalts
    app.toml                    Bezeichnung, Version, benötigte Handles
    bin/editor
    lib/…                       nur was diese App wirklich braucht
  2b71…9d/                      andere App — evtl. dieselbe lib, dann derselbe
  …                             Hash-Eintrag, also nur einmal auf der Platte

/apps/
  editor  ->  /store/8f3a…c1    lesbarer Name, zeigt auf eine Version

/users/justin/
  config/editor/                Einstellungen        → Backup
  state/editor/                 Dokumente, Sitzung   → Backup
  cache/editor/                 Vorschaubilder, Index → NIE Backup
```

Deinstallieren:

```
rm -r /users/justin/{config,state,cache}/editor   # Nutzerdaten
rm /apps/editor                                   # Verweis
# /store/8f3a…c1 verschwindet, sobald niemand mehr darauf zeigt (GC)
```

Es gibt keinen fünften Ort. Keine `/etc`-Schnipsel, keine Registry-Schlüssel,
keine Dienste, die sich irgendwo eingetragen haben, keine `~/.irgendwas`-Streu.

---

## 3. Warum drei Töpfe und nicht einer

Der Unterschied ist keine Ordnungsliebe, sondern eine **Backup- und
Wiederherstellungsentscheidung**:

| Topf | Inhalt | Backup | Verlust bedeutet |
|---|---|---|---|
| `config` | Einstellungen, Tastenbelegung, Konten | ja, klein, versionierbar | Ärger |
| `state` | Dokumente, Datenbanken, Verlauf | ja, das ist das Wertvolle | Datenverlust |
| `cache` | Thumbnails, Indizes, kompilierte Shader | **nein** | nichts, wird neu erzeugt |

Wo diese Trennung fehlt, sichern Backups jahrelang Browser-Caches mit, und
niemand weiß, ob ein Verzeichnis gelöscht werden darf. Die App muss die
Trennung nicht selbst können — sie bekommt beim Start **drei Handles** und kann
gar nicht anders, als sie zu benutzen (siehe § 7).

Erwartbarer Einwand: *Apps halten sich nicht daran.* Genau deswegen ist es
keine Konvention, sondern die einzige verfügbare Schnittstelle. Eine App ohne
Handle auf `state` hat keinen Weg, dort etwas abzulegen.

---

## 4. Warum content-addressed

* **Reproduzierbar.** Hash gleich = Inhalt gleich. „Bei mir läuft's" wird
  überprüfbar statt zur Ausrede.
* **Dedup umsonst.** Zwei Apps mit derselben Bibliotheksversion teilen sich
  einen Store-Eintrag. Der Speicherverbrauch von „jede App bringt alles mit"
  entfällt, ohne dass eine App der anderen die Bibliothek unter den Füßen
  wegtauschen kann.
* **Kein DLL-/Dependency-Hell.** Zwei Versionen derselben Bibliothek können
  gleichzeitig existieren. Es gibt keinen gemeinsamen Namensraum, in dem sie
  sich streiten könnten.
* **Atomar.** Installation ist: neuen Store-Eintrag schreiben, dann **einen**
  Verweis umlegen. Es gibt keinen halb installierten Zustand.
* **Prüfbar.** Ein Scan über den Store erkennt jede Veränderung an installierter
  Software — ohne Antivirensignaturen, allein durch Rechnen.

---

## 5. Systemgenerationen

Jede Systemänderung (Update, Installation, Konfigurationsänderung) erzeugt eine
**neue Generation**. Die alte bleibt vollständig und bootbar.

```
/system/generations/
  41/   ← aktuell
  40/   ← vorherige, bootbar
  39/
```

* **Rollback in Sekunden**: Bootmenü-Eintrag wählen oder einen Verweis umlegen.
  Kein Wiedereinspielen, kein Zurückrechnen von Änderungen.
* **Updates verlieren ihren Schrecken.** Das größte Risiko eines Updates ist
  nicht der Fehler, sondern dass man ihn nicht mehr los wird.
* **Aufräumen ist ausdrücklich**: alte Generationen bleiben, bis jemand sie
  löscht; danach räumt der Store-GC unerreichbare Einträge weg.

Voraussetzung dafür ist ein Dateisystem mit **Copy-on-Write-Snapshots** — der
Grund, warum das eigene Dateisystem in [FILESYSTEM.md](FILESYSTEM.md) CoW als
Pflichtmerkmal führt und nicht als Zusatz.

---

## 6. Vorbilder — und was wir daraus nehmen

| System | Übernommen | Nicht übernommen |
|---|---|---|
| **Nix / Guix** | Content-Addressing, Generationen, atomares Update, GC | die Sprache und die Komplexität der Ableitungen; OrientOS braucht kein eigenes Build-System, um eine App zu installieren |
| **macOS `.app`** | eine App = ein Verzeichnis, Löschen = Wegwerfen | die Ausnahmen (`~/Library`-Streu, `/Library/LaunchDaemons`, Installer-Pakete) |
| **Android** | strikte Datentrennung je App, App bekommt nur, was sie darf | die Play-Services-Abhängigkeit und das Rechte-Nachfragen zur Laufzeit |
| **GoboLinux** | lesbare Verzeichnisstruktur statt historisch gewachsener Pfade | Kompatibilitätssymlinks nach `/usr/bin` — das wäre die Hintertür, die alles wieder aufweicht |

---

## 7. Was das mit dem Kernel zu tun hat

Der entscheidende Punkt, und der Grund, warum das hier und nicht in einem
Userspace-Dokument steht: **Dieses Modell ist nur durchsetzbar, wenn der Kern
keine Umgebungsautorität kennt.** Auf einem POSIX-Kern wäre es Kosmetik — jedes
Programm könnte `/etc` öffnen und die Trennung unterlaufen.

Drei Kerneleigenschaften sind Voraussetzung, alle drei sind so gebaut — und
seit Phase 3 nicht nur als Typen, sondern als laufender Code mit Negativtests
im Boot-Log (Handle-Tabelle je Prozess mit Slot + Generation + prozesseigenem
Würfelwert, `kernel/src/abi/native.rs`, `libs/osum-abi-native/src/table.rs`):

1. **Kein globaler Pfad-Namensraum im Core.**
   `osum-native` hat kein `open("/pfad")`. Es gibt nur
   `NamespaceOpen(namespace_handle, name, rights)` — aufgelöst **relativ zu
   einem Handle**, das der Prozess besitzt. Ein Prozess ohne Namespace-Handle
   auf `/store` kann `/store` nicht einmal benennen.
   → `libs/osum-abi-native/src/syscall.rs`, `Syscall::NamespaceOpen`
2. **Keine Umgebungsautorität (ambient authority).**
   Jeder Zugriff braucht ein `Handle` mit `Rights`. Rechte lassen sich beim
   Weitergeben nur **verkleinern** (`Rights::restrict`), nie vergrößern.
   → `libs/osum-abi-native/src/rights.rs` (host-getestet:
   `rights_can_only_shrink`)
3. **Kein `fork`.**
   `fork` vererbt den kompletten Adressraum und alle Deskriptoren — das exakte
   Gegenteil von expliziter Rechteweitergabe. OrientOS hat nur
   `ProcessSpawn(image, namespace, handles[])`: Der Elternprozess **listet auf**,
   was das Kind bekommt. In der POSIX-Schicht ist `fork` dauerhaft `-ENOSYS`.
   → `libs/osum-abi-posix/src/lib.rs`, `sys_fork_unsupported()`
   Kernelseitig gebaut ist das als `spawn`/`spawn_with` in
   `kernel/src/abi/native.rs`: eine frische Handle-Tabelle ist **leer**, das
   Kind bekommt nur namentlich übergebene Handles, und Rechte können dabei nur
   kleiner werden. Im Boot-Log nachweisbar (`Prozessprobe: spawn ohne fork,
   Kind "dienst" mit 1 explizit uebergebenen Handle(n)`), samt Gegenprobe
   `Schreiben ohne Recht -> RightsDenied`. Als Syscall aus Ring 3 ist
   `ProcessSpawn` noch `NotSupported` — das kommt mit den Adressräumen je
   Prozess.

Der Start einer App sieht damit so aus:

```
ProcessSpawn(
    image     = <Handle auf /store/8f3a…c1/bin/editor, nur EXEC>,
    namespace = <Handle auf /store/8f3a…c1, nur READ>,
    handles   = [ config:  READ|WRITE,
                  state:   READ|WRITE,
                  cache:   READ|WRITE|CREATE,
                  konsole: WRITE ]
)
```

Die App hat danach **kein Mittel**, an das Dateisystem des Nutzers, an andere
Apps oder an Systemverzeichnisse zu kommen. Nicht weil eine Richtlinie es
verbietet, sondern weil sie die Objekte nicht benennen kann. Dateiauswahl
geschieht wie bei Android/macOS über einen vertrauenswürdigen Dienst, der ein
**Handle auf genau eine Datei** zurückgibt — nicht über einen Pfad, den die App
selbst öffnet.

---

## 8. Offene Fragen (ehrlich)

* **Store-GC**: Referenzzählung oder Erreichbarkeitsanalyse? Erreichbarkeit ist
  robuster, braucht aber einen Durchlauf über alle Generationen.
* **Update ohne Neustart**: Eine laufende App hält Handles auf ihren
  Store-Eintrag. Weiterlaufen lassen bis zum Beenden — oder Migration anbieten?
  Vermutlich Ersteres, es ist ehrlicher.
* **Dienste im Hintergrund**: Wer startet sie, mit welchen Handles, und wie
  verhindert man, dass daraus doch wieder ein globales `/etc` wird?
* **Signaturen**: Der Hash beweist Unverändertheit, nicht Herkunft. Signatur
  über den Hash, geprüft von wem, mit welchen Schlüsseln?
* **Größe**: Dedup über Hashes ist grob (ganze Dateien). Blockweises Dedup im
  Dateisystem wäre feiner — dann muss das aber das Dateisystem können.

Diese Fragen werden beantwortet, wenn Phase 4 (VFS, Dateisysteme) steht — nicht
vorher. Erst der Kern, dann das Modell darauf.
