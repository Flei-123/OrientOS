# Gefundene Fehler im Firn-Übersetzer

Hier liegen **kleinstmögliche** Programme, die einen Fehler des Übersetzers
auslösen. Sie gehören nicht zu osums Testlauf — sie sind das Material für die
Meldung an das Firn-Projekt und der Beweis, dass der Fehler noch da ist.

Regel: ein Eintrag hier nennt **Stand des Übersetzers**, **betroffene
Optimierungsstufe**, den **beobachteten** und den **erwarteten** Wert. Keine
Vermutungen ohne Disassemblat.

---

## F-01 · `mul` überschreibt `rdx` — der Registerallokator rechnet nicht damit

* **Übersetzer:** `4536a191` (Firn `main`, 23.08.2026)
* **Betroffen:** **nur `--opt-level=release-safe`**
  (`dev`, `dev-fast`, `release-fast` sind richtig)
* **Datei:** `rdx-klobber.fi` (20 Zeilen), Treiber `rdx-klobber.c`

### Was passiert

```
$ firnc --opt-level=release-safe -o n.o rdx-klobber.fi
$ cc -O2 -no-pie -o n rdx-klobber.c n.o && ./n
init(bits,4,256) -> a=richtig b=0 c=0   (erwartet b=c=256)
```

Der vierte Parameter (`frames`) ist nach der Schleife **0**. Die anderen
Parameter überleben.

### Warum

`init` rettet `frames` aus `rcx` nach `rdx`:

```
a6:  mov    %rcx,%rdx        # frames lebt ab hier in rdx
```

In der Schleife steht eine **geprüfte** Multiplikation (`w * 8`):

```
df:  mul    %rcx             # <-- schreibt RDX:RAX, zerstört rdx
e2:  jb     ea               # Überlaufzweig
```

`mul` schreibt das Ergebnis **immer** nach `RDX:RAX` — das obere Wort landet in
`rdx`, hier also 0. Damit ist `frames` weg, und die drei Feldschreibungen
danach schreiben Nullen.

### Warum nur `release-safe`

Es braucht **beides** gleichzeitig:

* **geprüfte Arithmetik** — erzeugt erst den `mul`+`jb`-Pfad. `release-fast`
  schaltet die Prüfungen ab; die Multiplikation mit 8 wird dort zu
  `shl $0x3` und fasst `rdx` nie an.
* **Registerallokation** — `dev` und `dev-fast` halten jeden Wert auf dem
  Stapel, dort gibt es nichts zu zerstören.

Nur `release-safe` hat beides. Das ist die unangenehmste Kombination, weil es
genau die Stufe ist, die man für einen Auslieferungsbau wählen würde: optimiert
**und** mit Prüfungen.

### Auswirkung auf osum

Der Bitmap-Rahmenverwalter `kernel/firn/bitmap.fi` fällt auf `release-safe` in
**19 von 23** Prüfstandsfällen um und ist auf allen anderen Stufen fehlerfrei
(`tests/firn-bitmap/lauf.sh`). Gefunden genau dadurch.

osum baut mit der Standardstufe (`dev-fast`) und ist deshalb **nicht**
betroffen. `release-safe` bleibt gesperrt, bis das behoben ist.
