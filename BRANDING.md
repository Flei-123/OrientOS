# BRANDING.md — zwei Marken, ein Quellbaum

Es gibt **zwei** Wege, dieses System anders zu nennen. Sie loesen verschiedene
Probleme; wer den falschen nimmt, macht sich unnoetig Arbeit oder einen Fork.

| | `./build.sh --brand <name>` | `./rename.sh <kernel> <os>` |
|---|---|---|
| aendert | nur das **Produkt** beim Bauen | den **ganzen Baum**: Verzeichnisse, Doku, Cargo-Namen |
| Baum danach | **unveraendert** | umbenannt, ein Commit |
| Ergebnis | `build/<slug>.iso` neben dem anderen | ein Projekt mit neuem Namen |
| wofuer | Zweitmarke (XoffiOS), Testballon, Kundenausgabe | endgueltige Umbenennung, einmalig |
| Anleitung | dieses Dokument | [RENAME.md](RENAME.md) |

Der Normalfall ist der erste. Ein Fork ist **nie** der richtige Weg — zwei
Baeume laufen auseinander, und ab dem Tag pflegt man alles doppelt.

---

## 1. Eine Marke bauen

```sh
./build.sh                    # Standardmarke -> build/orientos.iso
./build.sh --brand xoffi      # zweite Marke  -> build/xoffi.iso
```

Beide Abbilder liegen nebeneinander, aus demselben Quelltext, ohne dass eine
Datei im Baum angefasst wurde. Zum Starten dieselbe Marke angeben:

```sh
BRAND=xoffi ./run-qemu.sh --check
```

Gegenprobe, dass es wirklich wirkt — die Zeile kommt aus `kcore::branding`:

```
[osum] boot       osum v0.1.0 — Kernel von XoffiOS
```

Der Kernel heisst in beiden Marken `osum`. Das ist Absicht: eine Marke aendert
das Produkt, nicht den Kernel — so wie NT unter jeder Windows-Ausgabe NT heisst
und XNU unter macOS wie unter iOS.

---

## 2. Eine Marke anlegen

Eine Datei in `brands/`, fertig:

```toml
# brands/xoffi.toml
os-name   = "XoffiOS"
slug      = "xoffi"
publisher = "FleiTec"
web       = "https://xoffi.fleitec.com"
feed      = "https://xoffi.fleitec.com/pakete"
```

| Feld | Bedeutung | Pflicht |
|---|---|---|
| `os-name` | Name fuer Menschen: Banner, Oberflaeche, Doku | ja |
| `slug` | Kurzname fuer Maschinen: `<slug>.iso`, Verzeichnisse | ja |
| `publisher` | Herausgeber | nein |
| `web` | oeffentliche Adresse | nein |
| `feed` | **Paketquelle dieser Marke** | nein |
| `kernel-name` | Kernelname ueberschreiben | nein, normalerweise weglassen |

`feed` ist getrennt pro Marke, und das ist kein Detail: ein XoffiOS darf sich
niemals zu einem OrientOS „aktualisieren". Dieselbe Lehre steckt in
FreeViewer (`src/brand.rs`, `FV_BRAND_FEED`).

Fehlt ein Feld, greift der Wert aus `[package.metadata.branding]` in
`kernel/Cargo.toml`.

---

## 3. Woher die Werte kommen

`kernel/build.rs` fragt in dieser Reihenfolge, die erste Antwort gewinnt:

1. **Einzelne Umgebungsvariablen** — `OS_NAME_OVERRIDE`, `OS_SLUG_OVERRIDE`,
   `OS_PUBLISHER_OVERRIDE`, `OS_WEB_OVERRIDE`, `OS_FEED_OVERRIDE`,
   `KERNEL_NAME_OVERRIDE`. Fuer einen schnellen Versuch ohne Datei:
   ```sh
   OS_NAME_OVERRIDE="Testsystem" ./build.sh
   ```
2. **`brands/$BRAND.toml`**, wenn `BRAND` gesetzt ist (das macht `--brand`).
3. **`[package.metadata.branding]`** in `kernel/Cargo.toml`.
4. **Ableitung** aus dem Cargo-Paketnamen.

Ein **unbekannter Markenname bricht ab** und faellt nicht still auf die
Standardmarke zurueck — sonst baut man in Ruhe das falsche Produkt.

Die Bauskripte loesen dieselbe Reihenfolge in `brand.sh` auf (`OS_NAME`,
`SLUG`, `KERNEL_PKG`), damit Skript und Kernel nie auseinanderlaufen koennen.

---

## 4. Die Regel dahinter

> **Im Quelltext steht kein Produktname.**

Alles kommt aus `kernel/src/kcore/branding.rs`:

```rust
KERNEL_NAME  OS_NAME  SLUG  PUBLISHER  WEB  FEED  VERSION  LOG_TAG  NATIVE_ABI
banner()
```

Statt `"osum laeuft"` schreibt man `"{} laeuft", branding::KERNEL_NAME`.
`./test.sh` laesst den Build durchfallen, wenn ein Produktname als Literal
irgendwo sonst in `kernel/src` auftaucht — die Regel ist geprueft, nicht nur
aufgeschrieben.

Dasselbe gilt fuer die Testskripte: `run-qemu.sh` prueft den Boot-Banner gegen
`$OS_NAME` aus `brand.sh`, nicht gegen einen festen Namen. Sonst wuerde jede
Zweitmarke den Testlauf rot faerben, obwohl alles richtig ist.

---

## 5. Was NICHT in eine Markendatei gehoert

Unterschiede zwischen Marken gehoeren in **Daten**, nie in Code:

* welche Pakete im Abbild liegen,
* Erscheinungsbild und Voreinstellungen,
* Paketquelle.

Sobald irgendwo `if marke == "xoffi"` steht, ist die Trennung kaputt und man
hat sich einen Fork gebaut, der nur so tut, als waere er keiner.
