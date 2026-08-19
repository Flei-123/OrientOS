# Umbenennen von Kernel und Betriebssystem

Der Name ist eine **Konfiguration, keine Eigenschaft des Codes**. Wer sich den
endgültigen Namen offenhalten will, soll das ohne Refactoring tun können.
Deshalb gilt in diesem Projekt:

> **Im Quelltext steht kein Produktname.** Er kommt ausschließlich aus
> `kernel/src/kcore/branding.rs`, gespeist aus Cargo-Metadaten.
> `./test.sh` Schritt 14 lässt den Build durchfallen, wenn jemand das umgeht.

---

## Der kurze Weg (unter 15 Minuten, meist unter 2)

```sh
./rename.sh <neuer-kernel-name> <neuer-os-name>
# Beispiel:
./rename.sh nova Novaos
```

Das Skript macht alles und **prüft sich selbst**: am Ende baut es den Kernel
und bootet ihn in QEMU. Wenn `ERGEBNIS: alle Pruefungen bestanden.` erscheint,
ist die Umbenennung vollständig.

Regeln für die Namen:

| | Format | Beispiel |
|---|---|---|
| Kernelname | `^[a-z][a-z0-9-]*$` (gültiger Cargo-Paketname) | `nova` |
| OS-Name | `^[A-Za-z][A-Za-z0-9-]*$` | `Novaos` |

**Verifiziert:** `./rename.sh nova Novaos` wurde in einer Kopie unter `/tmp`
ausgeführt. Ergebnis: `x86_64-nova-none.json`, `libs/nova-mem`,
`libs/nova-abi-native`, `libs/nova-abi-posix`, Boot-Log
`[nova] boot nova v0.1.0 — Kernel von Novaos`, alle Prüfungen bestanden.

---

## Was das Skript im Einzelnen tut

1. **Alte Namen auslesen** — nicht raten:
   * Kernelname aus `kernel/Cargo.toml`, `name = "…"`
   * OS-Name aus `kernel/Cargo.toml`, `[package.metadata.branding] os-name = "…"`
2. **Verzeichnisse und Dateien umbenennen**
   * `libs/<alt>-mem`, `libs/<alt>-abi-native`, `libs/<alt>-abi-posix`
   * `x86_64-<alt>-none.json` (und `.VERIFIED`)
3. **Textersetzung über alle Textdateien** (ohne `.git/`, `target/`, `vendor/`,
   `build/`, ISO- und Logdateien). Die Reihenfolge ist wichtig, weil der
   OS-Name den Kernelnamen als Teilzeichenkette enthält (`karstos` ⊃ `osum`):
   1. OS-Name (`Karstos`) und seine Kleinschreibung (`karstos`)
   2. Zusammensetzungen (`osumfs`)
   3. Rust-Modulpfade (`osum_mem` → `nova_mem`)
   4. Bindestrichformen (`osum-abi-native`)
   5. der Name allein, mit Wortgrenzen (`\bkarst\b`)
4. **`Cargo.lock` löschen** (wird beim nächsten Build neu erzeugt, sonst
   verweist er auf Pakete, die es nicht mehr gibt)
5. **Gegenprobe**: `./build.sh` und `./run-qemu.sh --check`

---

## Von Hand — falls das Skript einmal nicht passt

```sh
ALT_K=osum; ALT_OS=Karstos
NEU_K=nova;  NEU_OS=Novaos

# 1. Verzeichnisse
for s in mem abi-native abi-posix; do mv libs/$ALT_K-$s libs/$NEU_K-$s; done
mv x86_64-$ALT_K-none.json          x86_64-$NEU_K-none.json
mv x86_64-$ALT_K-none.json.VERIFIED x86_64-$NEU_K-none.json.VERIFIED

# 2. Text (Reihenfolge beachten!)
find . -type f -not -path './.git/*' -not -path './target/*' \
       -not -path './vendor/*' -not -path './build/*' \
  -exec perl -pi -e "
      s/\Q$ALT_OS\E/$NEU_OS/g;
      s/\Qkarstos\E/\L$NEU_OS/g;
      s/\b\Q$ALT_K\Efs\b/${NEU_K}fs/g;
      s/\b\Q$ALT_K\E_/${NEU_K}_/g;
      s/\b\Q$ALT_K\E-/${NEU_K}-/g;
      s/\b\Q$ALT_K\E\b/$NEU_K/g;
  " {} +

# 3. Sperrdatei weg, neu bauen, prüfen
rm -f Cargo.lock && ./build.sh && ./run-qemu.sh --check
```

---

## Nur den OS-Namen ändern (Kernelname bleibt)

Eine Zeile in `kernel/Cargo.toml`:

```toml
[package.metadata.branding]
os-name = "Novaos"
```

Oder ohne Dateiänderung, nur für einen Lauf:

```sh
OS_NAME_OVERRIDE=Novaos ./build.sh && ./run-qemu.sh --check
```

---

## Woher der Name im Binary kommt

```
kernel/Cargo.toml
   name = "osum"                    ─┐
   [package.metadata.branding]        │
   os-name = "Karstos"               ─┤
                                      │  liest
OS_NAME_OVERRIDE (Umgebung, optional)─┤
                                      ▼
kernel/build.rs   ──  cargo:rustc-env=BRANDING_KERNEL_NAME / BRANDING_OS_NAME
                                      │
                                      ▼
kernel/src/kcore/branding.rs
   KERNEL_NAME · OS_NAME · VERSION · LOG_TAG · NATIVE_ABI · banner()
                                      │
                                      ▼
   klog!(), Panic-Handler, Boot-Banner, ABI-Beschreibung — alles im Baum
```

`build.rs` kommt **ohne TOML-Crate** aus: es sucht drei Zeilen per Textvergleich.
Eine Bauabhängigkeit dafür wäre genau der Ballast, den dieses Projekt vermeidet.

---

## Was das Skript **nicht** anfasst — und warum

| | Grund |
|---|---|
| `vendor/limine/` | Fremdcode, gehört uns nicht |
| `target/`, `build/` | Wegwerfartefakte, werden neu erzeugt |
| `.git/` | Historie bleibt Historie |
| ISO- und Logdateien | Binär bzw. Momentaufnahmen alter Läufe |

Nach der Umbenennung liegt in `build/` noch das alte ISO. `./build.sh`
überschreibt es beim nächsten Lauf.

---

## Grenzfälle

* **Der neue Name ist Präfix des alten** (`osum` → `kar`): funktioniert, weil
  alle Regeln Wortgrenzen benutzen.
* **Der neue OS-Name enthält den neuen Kernelnamen** (`nova` → `Novaos`):
  gewollt und getestet — die OS-Regel läuft vor der Kernel-Regel.
* **Zweimal hintereinander umbenennen**: funktioniert, weil die alten Namen
  jedes Mal frisch aus `kernel/Cargo.toml` gelesen werden.
* **Umbenennen mit unsauberem Arbeitsverzeichnis**: erst committen. Das Skript
  ändert sehr viele Dateien; ohne sauberen Stand ist der Diff unlesbar.
