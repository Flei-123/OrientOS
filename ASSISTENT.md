# ASSISTENT.md — der eingebaute Assistent und die Wahrnehmungsschicht

Entwurfsentscheidung. Vom Assistenten selbst ist **nichts** gebaut — dafür
fehlen Dateisystem, Grafik und Netz. Was hier steht, ist die Schnittstelle, an
der er später andockt, und der Grund, warum sie jetzt schon festgelegt wird:
sie kostet heute fast nichts und lässt sich später nicht mehr nachrüsten.

Die Idee stammt aus einem früheren Projekt (Orient OS, ~2024): ein
Betriebssystem, in dem ein KI-Assistent nicht ein Programm *auf* dem System
ist, sondern Teil davon — er sieht, was der Mensch sieht, kann suchen, kann
bedienen, kann das System erweitern.

---

## 1. Die eine Regel, die alles andere bestimmt

> **Das System bleibt ohne Assistent vollständig.**

Der Assistent ist ein **Paket**, kein Bestandteil. Bei der Installation fragt
das System, ob es ihn geben soll; in den Einstellungen kann er jederzeit
nachinstalliert oder wieder entfernt werden. Danach ist nichts von ihm übrig
und nichts hängt von ihm ab.

Das ist keine Höflichkeitsfloskel, sondern eine Bauvorschrift mit drei
Konsequenzen:

1. **Kein Kernelcode gehört dem Assistenten.** Was er braucht, brauchen auch
   andere: Barrierefreiheit-Werkzeuge, Fernwartung, Testautomatisierung,
   Skripte. Es gibt keinen Syscall, keinen Dienst und keine Datenstruktur, die
   es nur wegen ihm gibt.
2. **Kein Programm ruft den Assistenten auf.** Der Fluss geht immer in die
   andere Richtung. Ein Editor, der bei fehlendem Assistenten nicht startet,
   ist ein Fehler im Editor.
3. **Entfernen heißt entfernen.** `PACKAGING.md` liefert das schon: App ist ein
   Verzeichnis im Store, deinstallieren sind zwei `rm`. Kein Registryeintrag,
   der zurückbleibt, kein Dienst, der weiterläuft.

---

## 2. Warum keine Bildschirmfotos

Der naheliegende Weg — alle 500 ms ein Bild vom Bildschirm an ein Modell — ist
der schlechteste. Nicht nur wegen der Datenmenge:

| | Bildschirmfoto | Szenenbaum |
|---|---|---|
| Datenmenge | 1920×1080 als PNG: ~1–2 MB, als Modelleingabe grob 1.100–1.600 Bild-Token | ein Fenster als Text: wenige KB, einige hundert Token |
| Häufigkeit | Abfrage im Takt, meist ins Leere | nur bei echter Änderung |
| Latenz | Bild erzeugen, kodieren, übertragen, dekodieren | eine Nachricht über einen Channel |
| **Aussagekraft** | **das Modell muss raten, was ein Knopf ist** | **es steht da: Art, Name, Zustand, Ort** |

Der letzte Punkt ist der eigentliche. Ein Bildschirmfoto ist ein Notbehelf für
den Umstand, dass gängige Betriebssysteme ihre eigene Oberfläche nicht kennen.
Wer Pixel schickt, wirft Information weg, die im System bereits vorliegt —
und lässt sie anschließend vom Modell wieder erraten.

**Wer Text zeichnet, kennt ihn als Text.** Es gibt in diesem System keinen
Grund, jemals Zeichen aus Pixeln zurückzugewinnen.

---

## 3. Die Wahrnehmungsschicht

### 3.1 Der Szenenbaum ist ein Nebenprodukt

Der Fensterserver muss ohnehin wissen, welches Element wo liegt, welchen Text
es trägt und ob es gerade den Fokus hat — sonst könnte er es nicht zeichnen und
keine Eingabe zustellen. Die Wahrnehmungsschicht ist nichts weiter als **dieser
Zustand, nach außen gegeben** statt nur nach unten in den Bildspeicher.

Ein Knoten trägt: Art (Knopf, Textfeld, Liste, Beschriftung, Bild …), Name,
Wert, Zustand (fokussiert, gesperrt, gewählt), Rechteck, Kindknoten.

### 3.2 Pflicht statt Nachrüstung — der eigentliche Vorteil

Windows (UIA), Linux (AT-SPI) und macOS (AX) haben so einen Baum. Er ist überall
lückenhaft, weil er **nachträglich** kam: Programme müssen mitmachen, viele tun
es nicht, und in Oberflächen, die alles selbst zeichnen, ist er leer.

Hier ist die Reihenfolge umgekehrt. Der Fensterserver wird gerade erst gebaut,
also gilt:

> **Kein Fenster bekommt eine Zeichenfläche, wenn es seinen Baum nicht
> ausliefert.**

Kein bestehendes System kann das noch durchsetzen, ohne die Hälfte seiner
Programme zu zerstören. Dieses hat noch keine Programme — der billigste
Zeitpunkt, den es je geben wird.

Der Baum ist damit auch nicht „das Interface für die KI", sondern die
**Bedienoberfläche in Datenform**: dieselbe Quelle bedient Assistent,
Bildschirmleser, Fernwartung, Tastatursteuerung und den Test der Oberfläche.
Fünf Verbraucher, ein Mechanismus — das ist die Rechtfertigung dafür, dass es
im Kern liegt.

### 3.3 Ereignisse, nicht Abfragen

Der Beobachter fragt nicht „wie sieht es aus", er abonniert Änderungen:

```
fenster 7 · feld "suche" · wert -> "firn"
fenster 7 · fokus -> knopf "öffnen"
fenster 3 · geschlossen
```

Ein vollständiger Baum wird genau einmal geholt, danach nur noch Unterschiede.
Nichts passiert, solange nichts passiert.

### 3.4 Drei Stufen, damit es klein bleibt

| Stufe | Umfang |
|---|---|
| `fokus` | nur das Fenster mit dem Fokus |
| `sichtbar` | alles, was der Mensch gerade sieht |
| `alles` | auch verdeckte Fenster |

Der Beobachter wählt. Voreinstellung ist `fokus`, weil das in fast allen Fällen
reicht und am wenigsten preisgibt.

### 3.5 Bildschirmfoto als Rückfallebene

Für echte Pixel — Video, Spiel, Foto, freie Zeichenfläche — bleibt ein
Einzelbild möglich, aber:

* nur auf ausdrückliche Anforderung, nie im Takt,
* nur der angeforderte Ausschnitt, nicht der ganze Schirm,
* als eigenes Recht, das getrennt vergeben wird.

Ein Knoten, dessen Inhalt nicht als Struktur beschreibbar ist, markiert sich
selbst als „hier hilft nur ein Bild". Dann weiß der Beobachter, wann sich der
teure Weg lohnt — und nur dann.

---

## 4. Wie sich das in die bestehende ABI einfügt

**Es braucht keinen einzigen neuen Syscall.**

`libs/osum-abi-native/src/syscall.rs` hat 23 Nummern (0–22), lückenlos, stabil,
„werden nur angehängt". Der Fensterserver ist ein gewöhnlicher Prozess, und die
vorhandenen Bausteine reichen vollständig:

| Vorhanden | Aufgabe hier |
|---|---|
| `NamespaceOpen` (6) | den Fensterserver finden — kein globaler Pfad, kein prozessglobaler Root |
| `ChannelCreate` / `Send` / `Recv` (11–13) | Baum anfordern, Bedienbefehle schicken |
| `PortCreate` / `PortWait` / `PortBind` (14–16) | den Änderungsstrom abwarten, ohne zu pollen |
| `HandleDuplicate` mit Rechtemaske (4) | einem Helfer weniger Rechte weiterreichen, nie mehr |
| `MemoryCreate` / `MemoryMap` (8–9) | ein Bildschirmfoto übergeben, ohne es zu kopieren |

Dass eine Fähigkeit dieser Größenordnung ohne Erweiterung der Aufrufliste
hineinpasst, ist der beste verfügbare Beleg dafür, dass die ABI richtig
geschnitten ist. Sollte sich das ändern, gilt weiter: nur anhängen, nie eine
Nummer neu belegen.

---

## 5. Rechte — hier entscheidet sich alles

Ein Assistent, der den Bildschirm sieht und Eingaben erzeugt, ist anderswo ein
Sicherheitsalbtraum, weil er dieselbe Machtfülle hat wie der angemeldete
Mensch. Hier nicht: er bekommt **Handles mit `Rights`**, und Rechte lassen sich
beim Weitergeben nur verkleinern (`Rights::restrict`).

Vorgesehen, in der Sprache der bestehenden Rechte:

| Recht | erlaubt | ohne dieses Recht |
|---|---|---|
| `OBSERVE` | den Szenenbaum lesen | der Beobachter sieht nichts |
| `CAPTURE` | Einzelbild eines Bereichs | keine Pixel, auch nicht über Umwege |
| `INJECT` | Eingaben erzeugen | er kann berichten, aber nicht handeln |

Drei Eigenschaften, die daraus folgen und die kein nachgerüstetes System bieten
kann:

* **Getrennt vergeben.** Sehen ohne Bedienen ist der Normalfall. `INJECT` wird
  einzeln und sichtbar erteilt.
* **Ein Fenster darf sich entziehen.** Ein Passwortspeicher, der `OBSERVE`
  verweigert, ist für den Assistenten nicht etwa gesperrt — er **existiert für
  ihn nicht**. Das ist keine Richtlinie, die man umgehen könnte; das Handle
  fehlt schlicht.
* **Kein Erben.** Eine frische Handle-Tabelle ist leer. Ein vom Assistenten
  gestarteter Helfer hat von sich aus **keinen** Zugriff auf irgendetwas.

Ob der Assistent gerade zusieht, gehört sichtbar in die Oberfläche —
durchgesetzt vom Fensterserver, nicht vom Assistenten. Wer beobachtet, darf das
nicht selbst verschweigen können.

---

## 6. Wo das Modell läuft

Zwei Betriebsarten, dieselbe Schnittstelle:

**Entfernt.** Der Assistent redet über das Netz mit einem Dienst. Machbar,
sobald der Netzstack steht. Braucht das Recht `NET`, und die Kombination
`OBSERVE` + `NET` gehört ausdrücklich bestätigt — das ist der Punkt, an dem
Bildschirminhalt das Gerät verlässt.

**Örtlich.** Das Modell läuft auf dem Rechner. Ehrlich: das ist weit weg. Es
braucht Grafiktreiber, brauchbaren Durchsatz bei Matrixrechnung, Verwaltung
sehr großer Speicherbereiche — und es gibt noch kein Dateisystem. Kein Grund,
jetzt daran zu bauen.

Der Grund, die Schnittstelle trotzdem heute festzulegen: Sie ist in beiden
Fällen dieselbe. Wer die Wahrnehmungsschicht sauber schneidet, kann das Modell
später austauschen, ohne die Oberfläche anzufassen.

---

## 7. Reihenfolge

Nichts davon ist der nächste Schritt. Die Abhängigkeiten sind hart:

1. **Dateisystem** — ohne das kein Store, kein Paket, kein Nachinstallieren.
2. **Grafik und Fensterserver** — hier entsteht der Szenenbaum. **Der Zeitpunkt,
   an dem § 3.2 entschieden wird.**
3. **Eingabe** — Tastatur und Zeiger, und damit `INJECT`.
4. **Netz** — der entfernte Betrieb.
5. **Dateimanager und die übrige Oberfläche** — die ersten ernsthaften
   Verbraucher des Baums.
6. **Assistent als Paket.**

Was **jetzt** zu tun ist, ist genau eine Sache und sie kostet keine
Implementierung: Wenn der Fensterserver gebaut wird, wird der Szenenbaum
**gleich mitgebaut und zur Pflicht gemacht**. Nachträglich wird daraus die
gleiche Lückenlandschaft wie überall sonst.

---

## 8. Was hier bewusst offen bleibt

* Das genaue Format des Baums — entsteht mit dem Fensterserver, nicht davor.
* Ob `INJECT` sich als synthetisches Eingabeereignis oder als Aufruf am
  Baumknoten darstellt. Zweiteres ist verlässlicher, aber die Programme müssen
  mitspielen.
* Wie „das System erweitern" aussieht. Ein Assistent, der Pakete installiert,
  braucht Rechte im Store — und das ist eine eigene Entscheidung, die erst nach
  `PACKAGING.md` in gebauter Form getroffen werden kann.
