# tests/ — die Abnahme des Gesamtsystems

`test.sh` führt zwei Grundschritte selbst aus (Herkunft des Kernels, „kein
Rust mehr im Baum") und **sourct danach jede Datei `tests/step-*.sh`** in
alphabetischer Reihenfolge. Die Schrittnummern werden **gezählt**, nicht
geschätzt — eine Datei darf mehr als einen Schritt enthalten.

So kann jede Baustelle ihren Nachweis mitbringen, ohne dass zwei Leute
gleichzeitig `test.sh` ändern.

## Was hier gemessen wird — und was nicht

Seit dem Kernelwechsel ([KERNELWECHSEL.md](../KERNELWECHSEL.md)) misst
dieser Lauf **nicht mehr den Kernel**. Der wird in seinem eigenen Repo
abgenommen (Osum, `./test.sh`, 15 Abschnitte, über 1 100 Zusagen). Hier
wird gemessen, was **dieses** Repo tut: Herkunft, Marken, Zusammenstellung
des Produkts, Boot über BIOS und UEFI, ein Userland aus dem Boot-Modul,
und ob die Dokumente dasselbe sagen wie der Baum.

| Datei | Schritte |
|---|---|
| `test.sh` selbst | Herkunft · kein Rust mehr, und die eine Vorlage mit Begründung |
| `step-05-patches.sh` | die Berichtigungen am festgenagelten Kernel: begründet, passend, **je Patch einzeln** noch nötig |
| `step-10-marken.sh` | ein Quellbaum, zwei Produkte |
| `step-20-produkt.sh` | ISO, Userland-Modul, CRC32, Multiboot-Kopf |
| `step-30-boot.sh` | Boot über SeaBIOS **und** OVMF |
| `step-40-userland.sh` | Shell aus dem Modul in Ring 3 · Gegenproben ohne Modul und mit falscher Summe |
| `step-50-schutz.sh` | Capabilities aus Ring 3 · SMEP/SMAP im Produkt |
| `step-60-elf-korpus.sh` | 53 kaputte ELF-Abbilder durch den Lader |
| `step-70-doku.sh` | die Dokumente gegen den Baum |
| `step-80-pakete.sh` | Paketformat, Prüfsumme, Generationen, Quelle **und** ein Paket, das im gebooteten System wirklich läuft |
| `step-90-plan.sh` | the typed PLAN: kernel in the generation, sources with their keys, settings, accounts — and a whole tree rebuilt from **one text file**, byte for byte |
| `step-91-look.sh` | the **appearance** in the system state: colour scheme, wallpaper and taskbar as per-user `pref` lines, images content-addressed — and a rebuilt tree that *looks* the same |

`step-80-pakete.sh` misst die Paketverwaltung (ROADMAP 6.1/6.2, Format in
[PAKETE.md](../PAKETE.md)). Der zweite seiner beiden Schritte startet das
Produkt und lässt `/apps/hallo.osp/start` laufen — alles davor läuft auf
dem Wirt, und ein Paket, dessen Oktette man nur zählen kann, ist
installiert, nicht nachweislich lauffähig.

`step-90-plan.sh` came with round PLAN2 (27.08.2026) and measures the one
thing `step-80` could not: that the system state in `PLAN` is **complete
enough to rebuild a machine from**. Its core assertion runs
`opk.py rebuild` on an empty directory from a 745-octet plan plus a
signed source, and compares the result with `opk.py snapshot` — identical
over 39 entries, 18 files and 3 470 309 octets. Format and rejected
alternatives: [docs/PLAN-FORMAT.md](../docs/PLAN-FORMAT.md); numbers:
[docs/ROUND-PLAN2.md](../docs/ROUND-PLAN2.md).

`step-91-look.sh` came with the addendum of the same day and answers one
question: *does a device rebuilt from its plan **look** the same?* A
system with another colour scheme, another wallpaper, the taskbar on the
left and another timezone is rebuilt on an empty root and compared -- 62
entries, 35 files, 4 396 976 octets identical in the full demo, 45 / 25 /
3 923 350 in the suite. It also measures the level split: with **two**
accounts the `/etc` compatibility view disappears, because there is no
honest answer to whose theme `/etc/theme` would be. Design:
[docs/CONFIG-LEVELS.md](../docs/CONFIG-LEVELS.md).

`step-05-patches.sh` ist am 26.08.2026 dazugekommen und misst etwas, das
es vorher nicht gab: **Patches auf den festgenagelten Kernel.** Osums
Merge-Commit `c5fe12f` übersetzt nicht — zwei Stellen sind im Merge
verlorengegangen. OrientOS berichtigt sie beim Auspacken
(`vendor/osum/patches/`), statt den Nagel zu verschieben. Der Schritt
nimmt jeden Patch **einzeln** wieder heraus und verlangt, dass firnc den
Kernel dann ablehnt: so fällt auf, wenn eine Berichtigung überflüssig
geworden ist, statt dass sie jahrelang mitgeschleppt wird.

`tests/firn-elf/faelle/` ist Datenbestand, kein Schritt: 53 von Hand
gebaute ELF64-Köpfe, jeder mit genau **einem** Fehler, erzeugt von
`faelle.py`. Sie stammen aus der Zeit des Rust-Kernels und sind das
einzige Stück davon, das nicht gelöscht wurde — die Fälle sind besser als
der Code, der sie hervorgebracht hat.

## Vorlage für einen neuen Schritt

```bash
# tests/step-80-etwas.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# Hier steht, WAS gemessen wird und WARUM — nicht, dass etwas gemessen
# wird. Ein Kommentar, der nur den Code wiederholt, ist keiner.
step "Etwas: eine Behauptung, die man widerlegen könnte"
etwas_check() {
    RC=0
    ./run-osum.sh --script 'echo hallo;exit' --log /tmp/x.log >/dev/null 2>&1 \
        && ok "der Lauf endet regulär (21)" \
        || nok "der Lauf ist fehlgeschlagen"
    # GEGENPROBE: ohne den Gegenstand muss die Messung zusammenbrechen.
    ./run-osum.sh --ohne-userland --log /tmp/y.log >/dev/null 2>&1 \
        && grep -qa 'mod: none' /tmp/y.log \
        && ok "ohne Modul sagt der Kernel das, statt zu raten" \
        || nok "die Gegenprobe misst nichts"
    return $RC
}
run etwas_check
```

## Regeln

* **`ok` und `nok`, nicht `echo`.** `test.sh` zählt jede einzelne Zusage.
  Eine Sammelzahl allein wäre zu leicht grün zu bekommen.
* **Jede Zusage hat eine Gegenprobe** — denselben Lauf mit abgeschalteter
  Eigenschaft, wo die Messung zusammenbrechen muss. Ein grüner Test ohne
  Gegenprobe zählt in diesem Projekt nicht.
* **Kein `exit`, kein `set -e`** — die Datei läuft im Prozess von
  `test.sh`. Fehler werden über `RC=1` und den Rückgabewert gemeldet.
* **Wer das Produkt umbaut, baut es zurück.** Ein Schritt, der mit
  `--ohne-userland` oder `--dazu` ein anderes ISO erzeugt, ruft am Ende
  `./build.sh` — sonst misst der nächste Schritt etwas anderes, als er
  glaubt.
* **Die Datei wird erst angelegt, wenn der Nachweis wirklich grün ist.**
  Ein Schritt, der nur „später" grün wird, macht den Gesamtlauf rot.
