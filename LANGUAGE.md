# Logbuch: der Weg von Rust nach Firn

**Entschieden (21.08.2026): osum wird Firn-only.** Kein Rust, kein übernommener
Fremdcode. Dieses Dokument hieß bis dahin „wo Rust im Kernel im Weg steht" und
sammelte Belege dafür, dass eine eigene Sprache **gerechtfertigt** wäre. Diese
Frage ist beantwortet — die Sprache heißt **Firn**, sie existiert, und der
Umbau hat begonnen. Ab hier ist das Dokument zweierlei: der **Migrationsstand**
und weiterhin das Logbuch der Reibungspunkte, denn jeder davon ist eine
Anforderung an Firn.

> **Der alte Zweck, zur Einordnung:** „Die Sprache wird jetzt nicht gebaut — eine
> Sprache samt Backend, Werkzeugen und Bibliothek zu schreiben, während der
> Kernel noch keinen Userspace hat, ist der klassische Yak-Shave, an dem
> Projekte sterben." Das war zum Zeitpunkt von Runde 3 richtig. Es ist überholt,
> weil Firn inzwischen sich selbst übersetzt und in Runde 62 einen eigenen
> Kernel mit Adressräumen, Scheduler und Dateisystem getragen hat.

---

## M-00 · Migrationsstand

| | |
|---|---|
| **Zielsprache** | Firn, `profile kernel` (freistehend, kein libc, kein Runtime, kein `_start`) |
| **Übersetzer** | **festgenagelt** auf `vendor/firn/COMMIT` — derzeit `4536a191` (23.08.2026), gebaut von `vendor/firn/hole-firnc.sh` |
| **In Firn** | `serial.fi` (117) · `bitmap.fi` (344) · `elf.fi` (415) |
| **Noch in Rust** | rund 17 700 Zeilen in `kernel/`, `libs/`, `userland/` |
| **Aufrufrichtung** | nur **Rust → Firn**. Die Gegenrichtung ist gesperrt, siehe M-02 |

### Warum der Übersetzer festgenagelt ist

Firn wird gerade aktiv weiterentwickelt. Würde osum immer gegen den neuesten
Stand bauen, wäre bei jedem Fehler unklar, ob er aus dem Kernel oder aus dem
Übersetzer kommt. Deshalb: **ein Commit**, eingetragen in `vendor/firn/COMMIT`,
und nachgezogen wird **erst, wenn `./test.sh` grün ist**. Die Binärdatei ist
nicht eingecheckt — nur der Hash und das Bauskript.

## M-01 · Brückenkopf: die serielle Konsole

**Erledigt und gemessen.** `kernel/src/arch/x86_64/serial.rs` enthält keine
Logik mehr; der 16550-UART wird von `kernel/firn/serial.fi` bedient.

* **Warum ausgerechnet dieses Modul:** es ist ein **Blatt**. Es ruft nichts
  außerhalb seiner selbst auf und hält keinen Zustand — als einziges Modul
  trifft es damit keine der beiden Sprachgrenzen aus M-02.
* **Aufrufkonvention:** Firn erzeugt gewöhnliches SysV-AMD64 (Argumente in
  `rdi`/`rsi`, Rückgabe in `rax`, `rbp` und `r12`–`r15` werden gerettet).
  `extern "C"` genügt, es braucht keine Sonderbehandlung.
* **Symbolschema:** `firnc` vergibt `_F0.<name>`. Der Punkt ist in Rust kein
  gültiger Bezeichner, deshalb `#[link_name = "_F0.serial_init"]`.
* **Bauweg:** `build.sh` übersetzt die Firn-Module **vor** cargo und prüft, dass
  das Objekt **null undefinierte Symbole** hat; `kernel/build.rs` reicht es an
  den Linker weiter und bricht mit klarer Meldung ab, wenn es fehlt.
* **Nachweis:** `./test.sh` — **alle 21 Abschnitte bestanden**, 14 QEMU-Boots.
  Jede Zeile Bootausgabe in diesem Lauf ist durch den Firn-Code gegangen; ein
  Fehler dort hätte den gesamten Testlauf blind gemacht.
* **Erster Übersetzerwechsel überstanden (21.08.2026):** von `1bd95ceb` auf
  `c889170a` — 13 Firn-Runden Abstand, darunter Typ-Aliase, Standardtyp für
  Zahlliterale, `+=`, `str` und die Aufteilung von `std`. Das erzeugte Objekt
  ist **bitgleich** zum vorherigen (gleicher SHA-256), und `./test.sh` läuft
  unverändert 21/21 durch. Damit ist die Zusage aus dem Firn-Projekt — additive
  Runden brechen bestehenden Code nicht — für osum einmal **gemessen** statt
  geglaubt. Log: `build/testlauf-firn-c889170a.log`.
* **Reproduzierbar:** derselbe Übersetzer erzeugt aus derselben Quelle
  byteweise dasselbe Objekt (geprüft mit `cmp`). Ein Unterschied im Objekt ist
  damit immer ein Unterschied in Quelle oder Übersetzer, nie Rauschen.

## M-02 · Die zwei Grenzen, die den nächsten Schritt blockieren

Stage 0 von Firn kennt **kein `extern fn`** und **kein `static`**. Beides wird
vom Übersetzer abgewiesen. Daraus folgt hart:

1. **Firn kann nicht nach Rust zurückrufen.** Ein Firn-Modul kann aufgerufen
   werden, aber nichts außerhalb seiner Übersetzungseinheit aufrufen.
2. **Firn kann keinen globalen veränderlichen Zustand halten.** Der Firn-eigene
   Demo-Kernel umgeht das mit einem Zustandsblock, dessen Adresse der
   Startcode übergibt — eine funktionierende Krücke, keine Lösung.

**Solange das so ist, bleibt es bei Blattmodulen.** Jeder Treiber will
irgendwann beides. Die Sprachrunde dafür liegt bei Firn und ist eingeplant;
bis dahin wird hier nichts erzwungen und nichts nachgebaut.

Das ist übrigens genau **L-04** aus der Liste unten, nur von der anderen Seite:
Rust hat für „gehört der CPU" keine Kategorie und drängt zu `static mut`
(16 Vorkommen). Firn hat die Kategorie auch noch nicht — aber es ist die
Sprache, in der sie noch entstehen kann.

## M-03 · Was Firn Runde 73 freigibt — und warum es den Fahrplan ändert

Bis Runde 73 verbot `profile kernel` **jedes** `import std.*`. Ein Firn-Modul im
Kernel hatte also die Sprache, aber keine Bibliothek. Das ist aufgehoben:

* **`std.core` ist im Kernelprofil erlaubt.** Darin liegt alles, was weder
  Speicher anfordert noch einen Systemaufruf macht: die Span-Schicht (`find`,
  `trim`, `starts_with`, `compare`, Zerlegen), UTF-8-Lesen, Zahlenumwandlung
  (`text_to_i64`/`u64`/`f64`), Mathematik (`isqrt`, `gcd`, `ilog2`). Der
  Übersetzer **prüft die Behauptung nach** — ein Modul, das doch allokiert,
  bleibt verboten, mit Gegenprobe im Firn-Testlauf.
* **Der Allokator ist ein sichtbarer Parameter** (der Weg, den Zig geht): kein
  globaler Zuteiler, keine versteckte Anforderung. An der Aufrufstelle ist
  ablesbar, ob eine Funktion Speicher kostet. Es gibt eine Arena-Implementierung
  **ohne** Systemaufruf, die ein Kernel benutzen kann.

**Warum das mehr ist als Komfort:** es ist die Antwort auf **L-06**, und zwar
die, die dort schon als Wunsch steht — „Allokator als explizites Argument, ohne
Sonderweg für die Standarddatentypen". Firn hat sie gebaut, bevor osum sie
gebraucht hat.

**Und es dreht M-02 vom Blocker zum Entwurfsprinzip.** „Kein globaler
veränderlicher Zustand" war als Mangel notiert. Wenn Zustand konsequent als
Parameter durchgereicht wird — Allokator wie Gerätezustand —, ist das kein
Umgehen des fehlenden `static`, sondern der bessere Entwurf. Der osum-eigene
`frame_alloc` kann die Schnittstelle erfüllen; damit kann die Speicherverwaltung
nach Firn wandern, **ohne** auf eine Sprachrunde zu warten.

Was dadurch **nicht** gelöst ist: `extern fn`. Solange Rust-Code existiert, den
ein Firn-Modul aufrufen muss, bleibt die Richtung einseitig. Das erledigt sich
mit dem letzten Rust-Modul — oder mit der geplanten Sprachrunde, je nachdem, was
zuerst kommt.

*(Nachtrag 23.08.2026: Runde 75 hat `extern fn` gebracht, in beide Richtungen.
Die Grenze aus M-02 ist damit zur Hälfte gefallen. `static` fehlt weiterhin.)*

## M-04 · Zweiter Baustein: der Bitmap-Rahmenverwalter

**`libs/osum-mem/src/bitmap.rs` (593 Zeilen Rust) ist ausgebaut**, an seiner
Stelle steht `kernel/firn/bitmap.fi` (335 Zeilen Firn), angebunden über
`kernel/src/mm/firn_bitmap.rs`.

### Warum ausgerechnet dieses Modul

* Es hält **keinen globalen Zustand**. Bitspeicher und Verwaltungsblock gehören
  dem Aufrufer und werden als Zeiger hereingereicht — die Rust-Fassung machte
  es mit `&'a mut [u64]` genauso. Damit trifft es die verbliebene Grenze
  (`static`) **nicht**.
* Es ist **reine Logik**: Bitrechnerei, Next-Fit mit Umlauf, Bereichsoperationen.
  Kein Portgefummel, keine Hardware.
* Es hatte **23 Testfälle**, darunter einen Eigenschaftstest gegen ein
  Referenzmodell. Damit war objektiv prüfbar, ob die Neufassung dasselbe tut.
* Es läuft in **jedem Boot**. Ein Fehler fällt sofort auf, nicht irgendwann.

### Was Firn hier beiträgt, was Rust nicht tat

Die Rechnungen sind **geprüft**. In Rusts Release-Bau läuft `used -= 1` still
um; ab da meldet der Verwalter Milliarden freier Rahmen und vergibt Speicher,
den es nicht gibt. Dieselbe Zeile bricht in Firn mit Datei, Zeile, Spalte und
**beiden Zahlen** ab.

Wo ein Überlauf **kein** Fehler ist — Bereichsgrenzen aus der Memory-Map dürfen
über das Ende der Bitmap hinausragen —, steht das ausdrücklich im Quelltext: die
Grenze wird vorher gekappt (`kappen`), und `bm_free` prüft `anzahl > U64MAX -
index`, statt die Summe zu bilden. Der Unterschied zwischen „darf überlaufen"
und „darf nicht" ist damit **lesbar**, statt eine Konvention zu sein.

### Nachweis

* **23 von 23** Fällen bestanden — `tests/firn-bitmap/`, ein C-Prüfstand gegen
  **dasselbe** Objekt, das im Kernelabbild landet (Kernelprofil, freistehend).
  Derselbe Zufallsgenerator wie die Rust-Fassung, also dieselbe Folge von
  Anforderungen.
* `test.sh` Abschnitt 22 prüft zusätzlich, dass der Verwaltungsblock auf beiden
  Seiten denselben Aufbau hat (mit Gegenprobe) und dass die Rust-Fassung
  wirklich weg ist.
* Im Kernel: **8/8** Memory-Map-Zusagen, **28/28** Frame-Zusagen, in jedem Boot.

### Was dabei verloren ging — ehrlich benannt

Vier Host-Tests in `libs/osum-mem/src/lib.rs` prüften den Übergang
Memory-Map → Bitmap. Zwei davon (unsortierte überlappende Karte, nicht
ausgerichtete Ränder) sind nach `mm::frame::map_selftest()` gewandert und laufen
jetzt in **jedem Boot** gegen das echte Objekt — das ist eine Verbesserung.

**Ersatzlos entfallen** ist ein Eigenschaftstest über **200 zufällig zerrissene
Memory-Maps**. Er brauchte `Vec` und einen Host. Die Zufallsbreite fehlt seitdem.

### Der Preis: Tempo

Gemessen unter identischer Last (40 000 Anforderungen/Rückgaben, 65 536 Rahmen,
dieselbe Folge, bestes von 7 Läufen):

| Fassung | Zeit | Verhältnis |
|---|---|---|
| Rust-Verfahren, ungeprüft | 0,081 s | 1,00× |
| **Firn `dev-fast`** (was osum baut) | **0,415 s** | **5,11×** |
| Firn `release-fast` (Prüfungen aus) | 0,098 s | 1,22× |

**Das ist kein Rundungsfehler und wird hier nicht schöngeredet.** Die Aufteilung
sagt aber, woher es kommt: mit abgeschalteten Prüfungen liegt Firn bei 1,22× —
der Rest sind die Prüfungen selbst plus ein Codegen, der noch jede lokale
Variable über den Stapel führt statt über Register.

**Warum es trotzdem vertretbar ist:** der Rahmenverwalter wird im Boot einige
tausend Mal aufgerufen, nicht einige Millionen. 5× auf einer Operation im
Mikrosekundenbereich ist im Boot-Log nicht messbar. Sollte sich das ändern —
etwa wenn der Verwalter in einem heißen Pfad landet —, ist die Zahl hier
festgehalten und muss dann neu bewertet werden.

### Ein Übersetzerfehler, dabei gefunden

`--opt-level=release-safe` erzeugt falschen Code: eine geprüfte Multiplikation
benutzt `mul`, das implizit `rdx` überschreibt, und der Registerallokator
rechnet nicht damit. Ein Wert, der in `rdx` liegt und die Rechnung überleben
soll, ist danach zerstört. Die Bitmap fällt auf dieser Stufe in **19 von 23**
Fällen um und ist auf allen anderen fehlerfrei.

Minimalfall (20 Zeilen), Disassemblat und Analyse: `tests/firn-fehler/`.
osum baut mit `dev-fast` und ist nicht betroffen; `release-safe` bleibt gesperrt.

## M-05 · Dritter Baustein: der ELF64-Prüfteil

**Der Prüfteil von `kernel/src/kcore/elf.rs` (268 Zeilen Rust) ist ausgebaut**,
an seiner Stelle steht `kernel/firn/elf.fi` (415 Zeilen Firn), angebunden über
`kernel/src/kcore/firn_elf.rs`.

### Warum ausgerechnet dieser Teil

Ein ELF kommt **von außen**. Jede Zahl darin ist die Behauptung eines Fremden:
„meine Tabelle beginnt bei Offset X", „mein Segment ist Y lang". Wer damit
ungeprüft rechnet, baut sich genau die Lücke, die Angreifer suchen — ein
`off + filesz`, das umläuft, ergibt eine kleine Zahl, die Bereichsprüfung geht
durch, und danach wird an einer Adresse gelesen, die nie geprüft wurde.

Der **Ladeteil** bleibt in Rust. Er hängt an Seitentabellen, Rahmenverwalter und
Direct-Map-Fenster — ein Dutzend Rückrufe. Er fasst aber auch keine fremden
Bytes an: er arbeitet mit den bereits geprüften Werten. Das ist keine
Bequemlichkeit, sondern der richtige Schnitt.

### Erst der Maßstab, dann die Portierung

`elf.rs` hatte **null `#[test]`**. Es gab einen Selbsttest im Kernel mit
16 Fällen — besser als nichts, aber kein Maßstab, an dem sich eine Neufassung
messen lässt: die Fälle standen als Rust-Code **in** der Datei, die ersetzt
werden sollte.

Deshalb wurde der Maßstab **zuerst** gebaut und gegen die **alte** Fassung
belegt, bevor eine Zeile Firn entstand:

* **53 Falldateien**, erzeugt von `tests/firn-elf/faelle.py` — 8 gültige,
  45 abzuweisende
* Abgedeckt: abgeschnittene Dateien an jeder interessanten Stelle, jedes
  Kopffeld einzeln verfälscht, lügende Offsets und Größen, **Werte dicht an
  `u64::MAX`, die erst beim Addieren überlaufen**, überlappende Segmente,
  Segmente in derselben Seite, Grenzfälle der Ausrichtung, genau 16 gegen 17
  Segmente
* **Die alte Rust-Fassung besteht 53 von 53** — belegt, nicht behauptet

Die alte Fassung liegt als **Referenzmaßstab** in
`tests/firn-elf/alter-pruefteil.rs.txt`. Sie wird nicht mehr gebaut, aber in
jedem Testlauf **gefahren**: solange beide Fassungen zu jedem Fall dasselbe
sagen, ist die Portierung nachweislich **verhaltensgleich** — nicht bloß
„besteht auch Tests".

### Nachweis

* **53 von 53** gegen die Firn-Fassung (`tests/firn-elf/lauf.sh`)
* **53 von 53** gegen die Rust-Fassung (`tests/firn-elf/lauf-rust.sh`)
* `test.sh` Abschnitt 23: beide Läufe, `MAX_SEGMENTS`-Abgleich,
  Strukturaufbau mit Gegenprobe, Symbole im Abbild, „der Rust-Prüfteil ist weg"
* Im Kernel: **16/16** ELF-Negativfälle, **5/5** Ladefälle, jeder Boot — das
  echte `hello` aus dem Startdateisystem wird über den Firn-Parser geladen

### Was Firn hier beiträgt

Die Rust-Fassung hat die Überläufe mit `checked_add` von Hand abgefangen, an
jeder einzelnen Stelle, und der Modulkopf sagte ausdrücklich dazu: *„alle
Additionen sind überlaufsicher"*. Das war **Disziplin**, und sie hat gehalten.

In Firn ist es die **Sprache**: jede Rechnung, die überläuft, bricht ab — auch
die, an die niemand gedacht hat. Wo ein Überlauf ein *erwarteter Fehlerfall*
ist und kein Programmfehler, steht das ausdrücklich da: `passt_dazu(a, b)`
bildet die Summe gar nicht erst, sondern prüft `b <= U64MAX - a`. Der
Unterschied zwischen „darf nicht überlaufen" und „muss einen Fehlerwert
liefern" ist damit **lesbar**.

### Der Preis: Tempo

1 060 000 Aufrufe über alle 53 Fälle, bestes von 5 Läufen:

| Fassung | je Aufruf | Verhältnis |
|---|---|---|
| Rust-Verfahren (`checked_*`, `cc -O2`) | 79 ns | 1,00× |
| **Firn `dev-fast`** (was osum baut) | **651 ns** | **8,25×** |
| Firn `release-fast` (Prüfungen aus) | 136 ns | 1,97× |

**Das ist schlechter als bei der Bitmap** (dort 5,11× / 1,22×), und der Grund
ist klar: ein Parser rechnet in jeder Zeile.

**Warum es trotzdem tragbar ist:** der Prüfteil läuft im Boot **fünfmal** —
einmal je Programm plus die Selbsttests. 651 statt 79 Nanosekunden sind
zusammen unter drei Mikrosekunden. Sollte osum je Programme im Sekundentakt
starten, ist die Zahl hier festgehalten und muss neu bewertet werden.

**Die Ursache ist gefunden und aufgeschrieben**, nicht hingenommen:
`tests/firn-fehler/TEMPO.md` benennt drei Codegen-Befunde mit Minimalbeispiel
und Disassemblat gegen `cc -O2`. Der größte — **die geprüfte Addition legt
beide Operanden auf den Stapel, auf dem Erfolgspfad** — erklärt rund drei
Viertel des Aufschlags und lässt sich beheben, ohne die Semantik anzufassen.

### Was verloren ging — ehrlich benannt

**Nichts an Testabdeckung.** Das ist hier ausnahmsweise die ganze Wahrheit: der
Kernel-Selbsttest mit seinen 16 Fällen läuft unverändert weiter (jetzt gegen die
Firn-Fassung), und dazu kamen 53 Falldateien, die es vorher nicht gab.

Was **nicht** portiert ist und offen bleibt: der Ladeteil (Seiten anlegen,
Inhalte kopieren, Zurückrollen), rund 440 Zeilen. Er braucht Rückrufe nach
`mm` und `arch` — machbar seit Runde 75, aber ein eigener Brocken.

---

## Die Reibungspunkte mit Rust

Weiterhin gültig als **Anforderungsliste an Firn**: jeder Eintrag beschreibt
eine Stelle, die in der neuen Sprache besser gelöst werden muss als in der
alten. Stand der Messwerte (21.08.2026): **438** `unsafe`-Vorkommen in
`kernel/src`, **16** `static mut`.

**Regel:** Wer einen Workaround schreibt, trägt ihn hier ein. Kein Eintrag ohne
Datei und Zeile. Jeder Verweis hat die Form `` `datei.rs:Zeile` (`Anker`) `` und
wird von `test.sh` (Schritt „Doku-Verweise") maschinell geprüft.

## L-01 · `abi_x86_interrupt` — nightly für etwas Fundamentales

* **Datei:** `kernel/src/main.rs:26` (`abi_x86_interrupt`), benutzt in `kernel/src/arch/x86_64/interrupts.rs`
* **Problem:** Ausnahmehandler brauchen eine eigene Aufrufkonvention (die CPU
  legt einen anderen Rahmen ab, es endet mit `iretq`, bei manchen Vektoren liegt
  ein Fehlercode auf dem Stack). Rust kann das nur über ein **instabiles**
  Feature. Ein Kernel ohne Interrupt-Handler ist kein Kernel — das heißt: für
  eine Kernaufgabe ist die stabile Sprache nicht ausreichend.
* **Workaround:** `#![feature(abi_x86_interrupt)]`, dokumentiert in
  ARCHITECTURE.md § 9. Die Alternative wären handgeschriebene Assembler-Stubs
  für 256 Vektoren.
* **Eigene Sprache:** Aufrufkonventionen müssen **deklarierbar** sein, nicht
  eingebaut. Etwas wie „diese Funktion erhält ihren Rahmen von der Hardware,
  kehrt mit Instruktion X zurück, Register-Rettung nach Liste Y" — als
  Sprachmittel, nicht als Compiler-Sonderfall pro Architektur.

## L-02 · `alloc_error_handler` — nightly für eine Fehlermeldung

* **Datei:** `kernel/src/main.rs:30` (`alloc_error_handler`)
* **Problem:** Wir wollen bei erschöpftem Heap eine **lesbare** Meldung mit
  Heap-Statistik statt eines nackten Panics. Das erfordert ein weiteres
  instabiles Feature.
* **Workaround:** `#![feature(alloc_error_handler)]`.
* **Eigene Sprache:** Speicherbeschaffung sollte gar keine „Sprachhaken"
  brauchen. Allokation ist eine gewöhnliche Operation mit einem gewöhnlichen
  Ergebnis (`Result`), kein globaler Zustand mit einem globalen Fehlerhaken.
  Siehe auch L-06.

## L-03 · `naked` + `naked_asm` für den Kontextwechsel

* **Datei:** `kernel/src/arch/x86_64/context.rs:100` (`fn switch`)
* **Problem:** Ein Kontextwechsel darf **keinen** vom Compiler erzeugten Prolog
  und Epilog haben — er tauscht den Stack unter sich selbst aus. Rust braucht
  dafür `#[unsafe(naked)]` plus `naked_asm!` und verbietet in solchen Funktionen
  praktisch alles andere.
* **Workaround:** funktioniert, ist aber eine Insel: der Compiler weiß nichts
  über das, was dort passiert, und kann nichts prüfen.
* **Eigene Sprache:** Der Kontextwechsel ist eine der zehn wichtigsten
  Operationen eines Kernels. Er sollte ein **verstandenes Sprachkonstrukt** sein
  („diese Funktion wechselt den Ausführungskontext von A nach B"), damit der
  Compiler die Register-Rettung selbst korrekt erzeugt und prüfen kann, statt
  dass ein Mensch sie in Assembler aufschreibt und hofft.

## L-04 · `static mut` überall dort, wo Hardware die Adresse vorgibt

* **Dateien:** 13 Stellen, u. a.
  `kernel/src/arch/x86_64/idt.rs:50` (`static mut IDT`),
  `kernel/src/arch/x86_64/gdt.rs:36` (`static mut DF_STACK`),
  `kernel/src/boot/limine.rs:68` (`static mut REGIONS`),
  `kernel/src/kcore/sched.rs:734` (`static mut ACTIVE`),
  `kernel/src/kcore/print.rs:31` (`static mut SINK_OBJ`)
* **Problem:** Diese Objekte haben **genau eine** Instanz, ihre Adresse wird der
  Hardware übergeben (`lidt`, `lgdt`, `ltr`), und sie werden aus
  Interrupt-Kontext gelesen. Rusts Eigentumsmodell hat dafür keine Kategorie:
  es gibt kein „gehört der CPU". Man landet bei `static mut` + `addr_of_mut!`
  (25 Stellen) oder baut Zeigergymnastik.
* **Workaround:** `static mut` mit `addr_of_mut!`, um keine Referenzen zu
  erzeugen; jeder Zugriff ist `unsafe`.
* **Eigene Sprache:** Ein Eigentümerbegriff für **Hardware-Eigentum**. Etwa
  `hardware static IDT: [Gate; 256] @ align(16)` — der Compiler weiß dann: eine
  Instanz, Adresse stabil, Zugriff nur über eine definierte Schnittstelle, keine
  Referenzen nach draußen. Das ist kein Sicherheitsloch, sondern eine fehlende
  Kategorie.

## L-05 · Seitentabellen: 34 `unsafe`-Vorkommen für eine Baumstruktur

* **Datei:** `kernel/src/arch/x86_64/paging.rs` (34), `mm/mod.rs` (9)
* **Problem:** Eine Seitentabelle ist ein Baum aus 512er-Feldern, die über
  **physische** Adressen verkettet sind und nur über ein
  Direct-Map-Fenster (`phys + HHDM`) erreichbar sind. Rusts Referenzen können
  physische Adressen nicht ausdrücken; jeder Abstieg im Baum ist eine
  Rohzeigerrechnung. Der Borrow-Checker hilft hier nirgends.
* **Workaround:** `PhysAddr`/`VirtAddr` als eigene Typen, `phys_to_virt()`,
  Rohzeiger, alles `unsafe`.
* **Eigene Sprache:** **Adressräume als Typen erster Klasse.** Ein Zeiger sollte
  wissen, in welchem Adressraum er gilt (`phys`, `virt`, `virt@prozess-7`), und
  der Compiler sollte die Umwandlung erzwingen statt sie zu erlauben. Dann wäre
  „physische Adresse dereferenziert ohne HHDM" ein Typfehler statt eines Absturzes.

## L-06 · Der globale Allokator ist ein globaler Zustand

* **Datei:** `kernel/src/mm/heap.rs` (37 `unsafe`-Vorkommen)
* **Problem:** `#[global_allocator]` ist genau ein Allokator für den ganzen
  Prozess. Ein Kernel will mehrere: früher Boot-Allokator, Kernel-Heap,
  DMA-tauglicher Speicher (physisch zusammenhängend, unterhalb einer Grenze),
  Per-CPU-Bereiche, später Per-Prozess. `Vec::new_in(alloc)` existiert nur
  hinter `allocator_api` (wieder nightly), und `Box`/`String`/`format!` benutzen
  weiter den globalen.
* **Workaround:** ein einziger globaler Heap; alles andere geht am `alloc`-Crate
  vorbei und arbeitet direkt mit Frames.
* **Eigene Sprache:** Allokator als **explizites Argument** oder als Kontext im
  Typ, ohne Sonderweg für die Standarddatentypen. „Woher kommt dieser Speicher"
  ist im Kernel eine Kernfrage, keine Randnotiz.

## L-07 · Interrupt-Kontext ist im Typsystem unsichtbar

* **Dateien:** `arch/x86_64/interrupts.rs`, `arch/x86_64/serial.rs` (bewusst
  ungesperrt), `kcore/print.rs`
* **Problem:** Manche Funktionen dürfen aus einem Interrupt-Handler nicht
  aufgerufen werden (alles, was eine Sperre nimmt, die der unterbrochene Code
  schon hält). Rust kann das nicht ausdrücken — es bleibt bei Kommentaren.
  Konkrete Folge in diesem Baum: die serielle Ausgabe läuft **ohne Sperre**,
  damit der Panic-Handler nicht verklemmt (`serial.rs`, Modulkommentar). Das ist
  eine bewusste Entscheidung, die niemand prüft.
* **Workaround:** Disziplin und Kommentare.
* **Eigene Sprache:** **Effekte im Typsystem.** Eine Funktion trägt „darf im
  Interrupt-Kontext laufen" oder „nimmt Sperre L"; der Compiler weist
  Verletzungen ab. Verklemmungen sind eine statisch entscheidbare Klasse von
  Fehlern, sobald die Sperrhierarchie deklariert ist.

## L-08 · `transmute` für einen Funktionszeiger auf Daten

* **Datei:** `kernel/src/arch/x86_64/selftest.rs:71` (`transmute`)
* **Problem:** Der NX-Selbsttest muss absichtlich in einen Datenpuffer springen.
  Rust kennt keinen Weg, aus einer Adresse einen aufrufbaren Zeiger zu machen,
  außer `transmute` — dem stumpfsten Werkzeug der Sprache, das alles erlaubt.
* **Workaround:** `core::mem::transmute`, eng gekapselt.
* **Eigene Sprache:** Ein **gezielter** Operator „Adresse als Code
  interpretieren", der genau das erlaubt und sonst nichts. Ein Werkzeug, das
  alles kann, verhindert nichts.

## L-09 · Verlorene Typinformation an der Fassade `arch`

* **Dateien:** `kernel/src/arch/mod.rs`, `kcore/arch_iface.rs`
* **Problem:** Die arch-Grenze ist als Trait mit assoziierten Typen gebaut
  (`ArchOps::AddressSpace`, `::Timer`, `::IntCtl`). Damit der Core sie ohne
  Generics benutzen kann, gibt es in `arch/mod.rs` freie Funktionen, die
  weiterreichen. Das ist Verdopplung von Hand: jede neue Traitmethode muss an
  drei Stellen eingetragen werden (Trait, Fassade, Implementierung).
  Sichtbar geworden bei `fault_stack_top()` in dieser Runde.
* **Workaround:** Disziplin; `test.sh` Schritt 15 fängt wenigstens Lecks.
* **Eigene Sprache:** Etwas wie „Modul erfüllt Schnittstelle, wird beim Bauen
  ausgewählt" (Signaturen wie in ML/OCaml, nur mit statischer Auswahl) — dann
  entfällt die Fassade ersatzlos.

## L-10 · Kein `core` ohne Kunstgriffe: `build-std` und Target-JSON

* **Dateien:** `build.sh`, `.cargo/config.toml`, `x86_64-osum-none.json`
* **Problem:** Für ein eigenes Ziel muss `core`/`alloc` aus den Quellen neu
  gebaut werden (`-Z build-std`, instabil). Zusätzlich musste `build-std` aus
  `.cargo/config.toml` **heraus**, weil `[unstable]` für **jedes** Ziel gilt und
  die Host-Tests dann an einem zweiten `core` scheitern
  (`duplicate lang item: sized`) — ein Werkzeugproblem, das eine halbe Stunde
  Suche gekostet hat.
* **Workaround:** `build-std` als Flag in `build.sh`, dokumentiert in
  ARCHITECTURE.md § 9.
* **Eigene Sprache/Werkzeugkette:** Ein Ziel ist eine **Beschreibung**, keine
  privilegierte Liste im Compiler. Die Standardbibliothek wird für jedes Ziel
  gleich behandelt — es gibt keinen „eingebauten" und keinen „selbstgebauten"
  Fall.

## L-11 · Der volle Registersatz ist Handarbeit — und niemand prüft ihn

* **Dateien:** `kernel/src/arch/x86_64/preempt.rs:244` (`fn irq0_entry`,
  `naked_asm!`), Struktur `kernel/src/arch/x86_64/preempt.rs:90`
  (`struct FullFrame`) derselben Datei
* **Problem:** Für Präemption muss der Einsprung **alle 15** GPR retten (der
  kooperative Wechsel kommt mit den callee-saved aus). Der Assemblerteil pusht
  sie in einer Reihenfolge; die Rust-Struktur `FullFrame`, mit der der Verteiler
  sie liest, muss **exakt die umgekehrte** Reihenfolge auflisten. Beide stehen
   zwar untereinander in einer Datei, sind aber durch nichts verbunden: vertauscht
  jemand zwei Zeilen im Assembler und nicht in der Struktur, liest der Kernel
  `r11` als `rax` — und zwar erst dann falsch, wenn ein Tick genau dort landet.
  Der Compiler sieht davon nichts.
* **Workaround:** Kommentar über der Struktur („umgekehrte Push-Reihenfolge"),
  ein Selbsttest, der nach 60 Ticks prüft, dass alle drei Threads unversehrt
  weiterrechnen.
* **Eigene Sprache:** Registerrettung gehört **abgeleitet**, nicht abgeschrieben.
  Aus einer Deklaration „dieser Einsprung erhält den Hardware-Rahmen X und
  rettet Registersatz Y" sollte der Compiler Prolog, Epilog *und* den Typ, mit
  dem man den Rahmen liest, gemeinsam erzeugen. Zwei Beschreibungen desselben
  Sachverhalts, die von Hand synchron gehalten werden, sind ein Sprachmangel.

## L-12 · Systemaufruf-Einsprung: eine Konvention, die Rust nicht kennt

* **Dateien:** `kernel/src/arch/x86_64/user.rs:773` (`fn syscall_entry`),
  `kernel/src/arch/x86_64/user.rs:656` (`fn enter_ring3`),
  `kernel/src/arch/x86_64/user.rs:702` (`fn resume_kernel`),
  `kernel/src/arch/x86_64/user.rs:724` (`fn resume_kernel_after_fault`)
  — vier weitere `naked_asm!`-Rümpfe
* **Problem:** Bei `syscall` liefert die CPU keinen Stapelrahmen, sondern legt
  den Rücksprung in `rcx`, die Flags in `r11` und **behält den Stapel des
  Aufrufers** — der Kernel muss selbst umschalten, vorher das Basisregister
  tauschen und danach exakt in umgekehrter Reihenfolge zurück. Rust hat dafür
  keine Aufrufkonvention; `extern "C"` wäre schlicht falsch. Ergebnis: fünf
  nackte Funktionen in einer Datei, in denen der Compiler nichts prüfen kann,
  und ein Per-CPU-Block als `static mut`
  (`kernel/src/arch/x86_64/user.rs:117` (`static mut PERCPU`)), weil „gehört diesem
  Prozessorkern" keine Kategorie im Eigentumsmodell ist.
* **Workaround:** alles in eine Datei, ausführlicher Ablaufkommentar im
  Modulkopf, Messwerte (CS, CPL, Stapel) aus einem **echten** Ausnahmerahmen
  statt aus Behauptungen.
* **Eigene Sprache:** siehe L-01 — Aufrufkonventionen als deklarierbares
  Sprachmittel. Zusätzlich: **Privilegstufe als Effekt.** „Diese Funktion läuft
  auf Kernelseite eines Übergangs" ist prüfbar, sobald die Sprache den Begriff
  kennt.

## L-13 · Ein Zeiger aus Ring 3 sieht aus wie jeder andere Zeiger

* **Dateien:** `kernel/src/kcore/user.rs:66` (`fn is_user_addr`),
  `kernel/src/kcore/user.rs:254` (`aus Ring 3 abgewiesen`, Meldung im Log),
  Pufferprüfung im Verteiler `kernel/src/abi/native.rs`
* **Problem:** Ein Systemaufruf bekommt Adresse und Länge als zwei `u64`. Im
  Typsystem ist nichts daran anders als an einer Kerneladresse — die Prüfung
  „liegt im eigenen Bereich, Länge läuft nicht über" ist reine Disziplin an
  jeder einzelnen Aufrufstelle. Vergisst man sie an einer Stelle, schreibt ein
  unprivilegiertes Programm in den Kernel, und **kein Werkzeug meldet das**.
* **Workaround:** Jeder Puffer aus Ring 3 läuft durch dieselbe Prüfung; der
  Negativtest steht als Zusage im Boot-Log („Zeiger … aus Ring 3 abgewiesen").
* **Eigene Sprache:** Fortsetzung von L-05: Zeiger sollten ihren **Adressraum
  und ihre Herkunft** im Typ tragen (`virt@user(pid)` vs. `virt@kernel`). Eine
  Umwandlung ist dann eine Prüfung mit Ergebnis — nicht eine Konvention, an die
  man sich erinnern muss.

## L-14 · Bytes in Strukturen lesen: entweder `unsafe` oder von Hand

* **Datei:** `kernel/src/kcore/initramfs.rs:154` (`fn rd32`), daneben `rd64`
  * *Stand 23.08.2026:* Die ELF-Seite dieses Eintrags ist weg — die Byteleser
    des ELF-Kopfs liegen jetzt in `kernel/firn/elf.fi`. **Gelöst ist der Punkt
    damit nicht:** dort stehen sie genauso von Hand, nur mit geprüfter
    Arithmetik. Die Formatbeschreibung als Sprachmittel fehlt Firn ebenso.
    Der Archivleser ist noch in Rust und hält den Eintrag offen.
* **Problem:** Ein ELF-Kopf und ein Archiveintrag sind Bytefolgen fester Lage.
  Rust bietet dafür genau zwei Wege: `transmute`/Zeigerspiel (unsicher, und bei
  falscher Ausrichtung schlicht undefiniert) oder Feld für Feld
  `u64::from_le_bytes([b[o], b[o+1], …])`. Wir haben den zweiten gewählt — das
  ist sicher, aber es sind acht Indexzugriffe für **eine** Zahl, und die
  Feldversätze stehen als Konstanten daneben, statt aus einer
  Formatbeschreibung zu folgen. Eine Crate wie `zerocopy` wäre eine
  Fremdabhängigkeit für etwas, das die Sprache können sollte.
* **Workaround:** eigene kleine Leser mit vorheriger Längenprüfung; alle
  Additionen `checked_*`; 11 + 7 Negativfälle im Boot-Log als Beweis.
* **Eigene Sprache:** **Formatbeschreibungen als Sprachmittel.** „Diese
  Struktur liegt little-endian ab Versatz N in diesem Puffer" sollte deklariert
  werden, woraus der Compiler Prüfung und Zugriff erzeugt — sicher, ohne
  Ausrichtungsannahme und ohne Fremdcrate.

## L-15 · Im Interrupt darf nicht allokiert werden — sagt nur der Kommentar

* **Dateien:** `kernel/src/kcore/preempt.rs`, `kernel/src/kcore/sched.rs:734`
  (`static mut ACTIVE`)
* **Problem:** Der Zeitgebereinsprung entscheidet über einen Threadwechsel. Er
  darf dabei weder den Heap anfassen (der Sperren nimmt) noch eine Sperre
  nehmen, die der verdrängte Code gerade hält. Der Planer ist deshalb über
  einen rohen Zeiger erreichbar statt über eine Sperre — funktioniert, ist aber
  wieder nur Disziplin. Rust kann „diese Funktion allokiert nicht" nicht
  ausdrücken; `#[no_alloc]` gibt es nicht.
* **Workaround:** Der verdrängende Pfad allokiert nachweislich nicht (alle
  Thread-Stapel entstehen beim `spawn`, nicht im Tick), Kommentar im Modulkopf.
* **Eigene Sprache:** Effektsystem wie in L-07, aber mit der Kategorie
  **Allokation**. „Darf nicht allokieren" ist die häufigste ungeschriebene
  Regel im Kernel und gehört in die Signatur.

---

## Muster, die sich abzeichnen

Nach Phase 1–3 wiederholen sich dieselben drei Themen — jetzt mit mehr Belegen:

1. **Adressräume und Zeigerarten** (L-05, L-08, **L-13**) — Rust kennt genau
   eine Art Zeiger. Ein Kernel braucht mindestens vier (physisch,
   virtuell-eigener Raum, virtuell-fremder Raum, **aus Ring 3 hereingereicht**)
   und will sie nicht verwechseln dürfen. Mit Ring 3 ist aus einem
   Bequemlichkeitsproblem ein Sicherheitsproblem geworden.
2. **Hardware-Eigentum und Einzelinstanzen** (L-04, **L-12**) — Objekte, deren
   Eigentümer die CPU ist, passen in kein Eigentumsmodell, das nur Programmteile
   kennt. Der Per-CPU-Block ist der dritte Fall derselben Art.
3. **Ausführungskontext als Effekt** (L-03, L-07, **L-11**, **L-12**, **L-15**)
   — „läuft im Interrupt", „wechselt den Kontext", „hält Sperre L", „allokiert
   nicht", „läuft auf der Kernelseite eines Privilegübergangs" sind prüfbare
   Eigenschaften, die heute alle nur in Kommentaren stehen. Dieses Muster ist in
   dieser Runde am deutlichsten gewachsen.

Neu hinzugekommen und (noch) kein Muster, aber notiert:
**Formatbeschreibungen** (L-14) — ELF-Kopf, Archiveintrag, Seitentabelleneintrag
sind alle „Bytes an festen Versätzen"; dreimal von Hand ausgelesen.

Fünf neue Einträge in einer Runde, alle aus demselben Anlass (Präemption,
Ring 3). Fünfzehn Einträge sind noch kein Zwang zur eigenen Sprache — aber die
Richtung ist inzwischen eindeutig.

## Was gegen eine eigene Sprache spricht (auch das gehört hierher)

* Rust hat 413 `unsafe`-Vorkommen **nicht verhindert**, aber sie **sichtbar
  gemacht**. Jede dieser Stellen ist markiert, begründet und auffindbar. Das ist
  mehr, als C je geboten hat.
* Der Borrow-Checker hat in `libs/osum-mem` (Bitmap, Heap, Regionen) und in
  `libs/osum-abi-native` (Handle-Tabelle) real Fehler verhindert — dort liegt
  die Logik, dort ist er wertvoll, dort laufen 151 Host-Tests. Die
  Capability-Regeln dieser Runde waren auf dem Host fertig getestet, **bevor**
  der Kernel sie zum ersten Mal ausgeführt hat.
* Eine eigene Sprache heißt: Compiler, Optimierer, Debugger-Formate, Formatierer,
  Editorunterstützung, Bibliothek. Jede Stunde daran ist eine Stunde nicht am
  Betriebssystem.
* Solange die Liste oben zehn Einträge hat und nicht hundert, ist die ehrliche
  Antwort: **Rust reicht.**

## L-16 · Ein Ausnahmerahmen gehört zu einem Stapel — die Sprache weiß es nicht

* **Dateien:** `kernel/src/arch/x86_64/user.rs:162` (`const TRAP_GAP`),
  `kernel/src/arch/x86_64/preempt.rs:60` (`static USER_PREEMPTIONS`),
  `kernel/src/kcore/user.rs:510` (`fn run_preemptible`)
* **Problem:** Damit ein Programm in Ring 3 verdrängt und später **fortgesetzt**
  werden kann, muss der Rahmen, den die CPU beim Privilegwechsel ablegt, auf dem
  Kernelstapel **genau des Threads** liegen, der das Programm trägt. Liegt er auf
  einem gemeinsamen Stapel, überschreibt ihn der nächste Wechsel — und das
  Programm kehrt in einen Zustand zurück, den es nie hatte. Diese Bindung
  „Rahmen ↔ Stapel ↔ Thread" ist im Typsystem nirgends sichtbar: `TSS.rsp0` ist
  eine nackte Zahl, und ob sie gerade auf den richtigen Stapel zeigt, weiß nur
  der Mensch. Ein Fehler hier äußert sich nicht als Absturz an der Fehlerstelle,
  sondern als sporadisch falsches Register Millisekunden später.
* **Workaround:** Der Stapel wird beim Eintritt in Ring 3 aus dem eigenen
  Stapelzeiger abgeleitet (`TRAP_GAP` Abstand) und danach zurückgesetzt; ein
  Schalter (`set_preemptible`) trennt die beiden Betriebsarten hart, und der
  Nachweis läuft in jedem Boot mit (12 Unterbrechungen, Programm endet trotzdem
  mit Code 0).
* **Eigene Sprache:** Stapel sollten **Typen mit Kapazität und Besitzer** sein,
  keine `u64`. „Diese Ausnahme legt ihren Rahmen auf Stapel S ab, S gehört
  Thread T, T ist gerade eingelastet" ist eine Aussage, die eine Systemsprache
  prüfen können sollte. Solange sie es nicht kann, ist Präemption in Ring 3
  Handarbeit mit sehr scharfen Kanten.

## L-17 · `swapgs`: globaler Zustand, den kein Typ beschreibt

* **Dateien:** `kernel/src/arch/x86_64/user.rs:688` (`swapgs` im Einsprung),
  `kernel/src/arch/x86_64/preempt.rs:244` (`fn irq0_entry`, bewusst **ohne**
  `swapgs`)
* **Problem:** Ob `gs:` gerade auf den Kernel- oder auf den Programmblock zeigt,
  hängt davon ab, wie oft `swapgs` auf dem Weg hierher ausgeführt wurde. Das ist
  versteckter globaler Zustand: dieselbe Rust-Funktion ist je nach Aufrufweg
  korrekt oder nicht. Rust hat dafür keinen Begriff, und `unsafe` sagt nur
  „hier passiert etwas Gefährliches", nicht „hier gilt Zustand X".
* **Workaround:** Genau eine Datei fasst `swapgs` an, der Modulkopf beschreibt
  jeden Pfad einzeln, und der Verdrängungspfad kommt bewusst ohne aus (er
  benutzt `gs:` nicht).
* **Eigene Sprache:** So etwas gehört als **Effekt/Zustandsindex** in die
  Signatur: `fn dispatch(...) requires gs = kernel`. Der Compiler weist dann den
  Aufruf aus einem Pfad ab, der den Zustand nicht hergestellt hat — statt dass
  ein Mensch drei Aufrufwege im Kopf behält.
