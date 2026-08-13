# Logbuch: wo Rust im Kernel im Weg steht

**Zweck.** Langfristig soll Karstos eine eigene Systemsprache bekommen. Sie wird
**jetzt nicht gebaut** — eine Sprache samt Backend, Werkzeugen und Bibliothek zu
schreiben, während der Kernel noch keinen Userspace hat, ist der klassische
Yak-Shave, an dem Projekte sterben.

Stattdessen: **Beweise sammeln.** Ab sofort wird jede konkrete Stelle
protokolliert, an der Rust im Kernel-Kontext behindert — mit Datei, Zeile,
Problem, Workaround und der Frage, was eine eigene Sprache anders machen müsste.
Wenn die Liste lang und die Muster klar genug sind, ist die Sprache begründet
statt gewünscht. Wenn nicht, war es die richtige Entscheidung, sie nicht zu
bauen.

**Regel:** Dieses Dokument wächst mit jeder Runde. Wer einen Workaround schreibt,
trägt ihn hier ein. Kein Eintrag ohne Datei und Zeile.

**Stand:** Runde 2, Phase 1–2 (bootfähiger Kern + kooperativer Scheduler).
Messwerte: 308 `unsafe`-Vorkommen in `kernel/src`, 13 `static mut`,
25 `addr_of!`/`&raw`, 2 nightly-Features, 1 `transmute`.

---

## L-01 · `abi_x86_interrupt` — nightly für etwas Fundamentales

* **Datei:** `kernel/src/main.rs:23`, benutzt in `kernel/src/arch/x86_64/interrupts.rs`
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

* **Datei:** `kernel/src/main.rs:27`
* **Problem:** Wir wollen bei erschöpftem Heap eine **lesbare** Meldung mit
  Heap-Statistik statt eines nackten Panics. Das erfordert ein weiteres
  instabiles Feature.
* **Workaround:** `#![feature(alloc_error_handler)]`.
* **Eigene Sprache:** Speicherbeschaffung sollte gar keine „Sprachhaken"
  brauchen. Allokation ist eine gewöhnliche Operation mit einem gewöhnlichen
  Ergebnis (`Result`), kein globaler Zustand mit einem globalen Fehlerhaken.
  Siehe auch L-06.

## L-03 · `naked` + `naked_asm` für den Kontextwechsel

* **Datei:** `kernel/src/arch/x86_64/context.rs:111`
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
  `arch/x86_64/idt.rs:50` (IDT), `arch/x86_64/gdt.rs:36` (GDT, TSS, IST-Stapel),
  `boot/limine.rs:64` (Memory-Map-Puffer), `kcore/sched.rs:421` (aktiver Planer),
  `kcore/print.rs:31` (Konsolen-Senke)
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

* **Datei:** `kernel/src/arch/x86_64/selftest.rs:71`
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

* **Dateien:** `build.sh`, `.cargo/config.toml`, `x86_64-karst-none.json`
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

---

## Muster, die sich abzeichnen

Nach Phase 1–2 wiederholen sich drei Themen:

1. **Adressräume und Zeigerarten** (L-05, L-08) — Rust kennt genau eine Art
   Zeiger. Ein Kernel braucht mindestens drei (physisch, virtuell-eigener
   Raum, virtuell-fremder Raum) und will sie nicht verwechseln dürfen.
2. **Hardware-Eigentum und Einzelinstanzen** (L-04) — Objekte, deren Eigentümer
   die CPU ist, passen in kein Eigentumsmodell, das nur Programmteile kennt.
3. **Ausführungskontext als Effekt** (L-03, L-07) — „läuft im Interrupt",
   „wechselt den Kontext", „hält Sperre L" sind prüfbare Eigenschaften, die
   heute nur in Kommentaren stehen.

Das sind noch keine zehn erdrückenden Argumente, aber es sind drei klare Muster
nach zwei Phasen. Die Liste wird fortgeschrieben.

## Was gegen eine eigene Sprache spricht (auch das gehört hierher)

* Rust hat 308 `unsafe`-Vorkommen **nicht verhindert**, aber sie **sichtbar
  gemacht**. Jede dieser Stellen ist markiert, begründet und auffindbar. Das ist
  mehr, als C je geboten hat.
* Der Borrow-Checker hat in `libs/karst-mem` (Bitmap, Heap, Regionen) real
  Fehler verhindert — dort liegt die Logik, dort ist er wertvoll, dort laufen
  129 Host-Tests.
* Eine eigene Sprache heißt: Compiler, Optimierer, Debugger-Formate, Formatierer,
  Editorunterstützung, Bibliothek. Jede Stunde daran ist eine Stunde nicht am
  Betriebssystem.
* Solange die Liste oben zehn Einträge hat und nicht hundert, ist die ehrliche
  Antwort: **Rust reicht.**
