# tests/ — zusaetzliche Schritte fuer `./test.sh`

`test.sh` fuehrt die 15 Grundschritte selbst aus und **sourct danach jede
Datei `tests/step-*.sh`** in alphabetischer Reihenfolge. Die Schrittnummern
zaehlt `test.sh` automatisch hoch (`N/TOTAL`).

So kann jede Baustelle ihren Nachweis mitbringen, ohne dass zwei Leute
gleichzeitig `test.sh` aendern.

Vorlage:

```bash
# tests/step-16-preempt.sh — wird von test.sh gesourct, nicht direkt gestartet.
step "Verdraengung: Wechsel ohne freiwilliges yield"
run ./run-qemu.sh --test-preempt
```

Regeln:

* Eine Datei = ein Schritt = genau ein `step` und ein `run`.
* Kein `exit`, kein `set -e` — die Datei laeuft im Prozess von `test.sh`.
* Die Datei wird erst angelegt, wenn der Nachweis **wirklich gruen** ist.
  Ein Schritt, der nur "spaeter" gruen wird, macht den Gesamtlauf rot.
