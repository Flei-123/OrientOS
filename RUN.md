# RUN — OrientOS bauen, starten, prüfen

Kernel `osum`, Betriebssystem `OrientOS`, Rust `no_std`, Ziel `x86_64-osum-none`.
Alle Befehle laufen im Projektwurzelverzeichnis, alle Pfade sind relativ.

## Voraussetzungen

```sh
rustup toolchain install nightly && rustup default nightly
rustup component add rust-src llvm-tools
sudo apt install qemu-system-x86 xorriso mtools nasm ovmf python3
```

`nasm` + `ld` + `python3` werden für das unprivilegierte Programm und das
Startdateisystem gebraucht. Fehlen sie, baut `build.sh` trotzdem durch und der
Kernel meldet ehrlich, dass kein Archiv da ist — die ELF-Tests fallen dann aus.

Netzzugang zu crates.io ist nicht nötig; Limine liegt vorgebaut in `vendor/limine/`.
`build-std` steht bewusst als Flag in `build.sh` und **nicht** in
`.cargo/config.toml` — sonst scheitern die Host-Tests an `duplicate lang item`.

## Der kurze Weg

```sh
./build.sh          # Kernel + Userland + Initramfs + bootfähiges build/orientos.iso
./run-qemu.sh       # interaktiver Start, serielle Ausgabe im Terminal (Strg-C beendet)
./test.sh           # der vollständige Nachweis: 19 Schritte, Exitcode 0 = alles grün
```

Ein Durchlauf von `./test.sh` dauert auf diesem Rechner rund **2,5 Minuten**
(154 s gemessen, 14 echte QEMU-Boots). Nach `cargo clean` sind es rund
**6,5 Minuten** (392 s), weil `core`/`alloc` über `build-std` neu übersetzt
werden.

## Einzelne Schalter

| Befehl | Wirkung |
|---|---|
| `./build.sh` | Release-Build + Userland + `build/initramfs.img` + ISO + Symbolkarte `build/osum.map` |
| `./build.sh --debug` | Debug-Build |
| `./build.sh --no-posix` | Gegenprobe ohne POSIX-Schicht (`--no-default-features`) |
| `./run-qemu.sh --check` | bootet und prüft 38 Merkmale im Log, Exitcode 0/1 |
| `./run-qemu.sh --check --no-posix` | muss „posix-Schicht NICHT einkompiliert“ melden |
| `./run-qemu.sh --check --uefi` | Boot über OVMF statt SeaBIOS |
| `./run-qemu.sh --test-pagefault` | absichtlicher `#PF`, CR2 und Ursache im Klartext |
| `./run-qemu.sh --test-doublefault` | **echter** `#DF` auf eigenem Notfallstapel (kein `int 8`) |
| `./run-qemu.sh --test-panic` | Panic-Handler mit Backtrace |
| `./run-qemu.sh --test-rodata` | Schreibversuch auf `.rodata` muss `#PF` auslösen |
| `./run-qemu.sh --test-nx` | Ausführungsversuch auf Daten muss `#PF` auslösen |
| `./run-qemu.sh --test-gp` | nicht kanonische Adresse muss `#GP` auslösen |
| `./run-qemu.sh --test-ud` | ungültige Instruktion (`ud2`) |
| `./run-qemu.sh --test-preempt` | Verdrängung: Zählschleifen ohne `yield`, Tickverteilung nach Priorität |
| `./run-qemu.sh --test-ring3` | unprivilegiertes Programm in Ring 3 + Negativtest auf eine Kerneladresse |
| `./run-qemu.sh --test-elf` | ELF64 aus dem Startdateisystem laden, Müll definiert abweisen |
| `./run-qemu.sh --test-handles` | Handle-Negativtests (Index, Generation, fehlendes Recht) |
| `./rename.sh <kernel> <os>` | benennt Kernel und OS im ganzen Baum um (siehe RENAME.md) |

### POSIX abwählen — die genaue Befehlszeile

`./build.sh --no-posix` ist die empfohlene Form. Wer `cargo` direkt aufruft,
**muss die `build-std`-Flags mitgeben** — sie stehen bewusst in `build.sh` und
nicht in `.cargo/config.toml` (sonst scheitern die Host-Tests an
`duplicate lang item: sized`). Ein nacktes `cargo build --no-default-features`
scheitert deshalb mit `can't find crate for core`; das ist erwartet und kein
Defekt der POSIX-Abtrennung. Die vollständige Form:

```sh
cargo build -Z build-std=core,compiler_builtins,alloc \
            -Z build-std-features=compiler-builtins-mem \
            --release --no-default-features
```

Der Boot ohne POSIX meldet dann im Log:

```
[osum] boot       ABI         : osum-native (POSIX nicht einkompiliert)
[osum] abi        posix-Schicht NICHT einkompiliert — Core laeuft ohne sie
```

Host-Tests der hardwarefreien Logik allein (146 Tests, laufen in Sekunden):

```sh
cargo test --target x86_64-unknown-linux-gnu \
    -p osum-mem -p osum-abi-native -p osum-abi-posix
```

## Was `./test.sh` abdeckt

Die Grundschritte 1–15 stehen in `test.sh`, die Nachweise der einzelnen
Baustellen als eigene Dateien in `tests/step-*.sh` (werden automatisch
angehängt und mitgezählt).

1. Host-Tests (146) · 2. Build mit POSIX · 3. Build ohne POSIX ·
4. Boot BIOS · 5. Boot ohne POSIX · 6. `#PF` · 7. echter `#DF` · 8. Panic ·
9. `.rodata` schreibgeschützt · 10. NX · 11. `#GP` · 12. `#UD` ·
13. Boot über UEFI · 14. Produktname nur aus `branding.rs` ·
15. Architekturgrenze und Codehygiene · 16. Verdrängung ·
17. Ring 3 · 18. ELF-Lader · 19. Capabilities

Schritt 15 prüft unter anderem, dass **keine** x86-Details außerhalb
`kernel/src/arch/` im Code stehen, dass es kein `todo!()` gibt, dass der letzte
Build **null** Compilerwarnungen erzeugt hat und dass jedes `test-*`-Feature aus
`kernel/Cargo.toml` auch einen Schalter in `run-qemu.sh` hat.

## Belegter Zustand (letzter vollständiger Lauf, 13.08.2026)

```
$ ./test.sh ; echo "EXIT=$?"
 1/19 Host-Tests der architekturunabhaengigen Crates                        => bestanden
 2/19 Kernel baut (Release, mit POSIX)                                      => bestanden
 3/19 Kernel baut OHNE POSIX-Schicht                                        => bestanden
 4/19 Boot in QEMU (BIOS)                                                   => bestanden
 5/19 Boot in QEMU ohne POSIX-Schicht                                       => bestanden
 6/19 Selbsttest Page Fault                                                 => bestanden
 7/19 Selbsttest Double Fault (echter #DF auf IST-Stapel)                   => bestanden
 8/19 Selbsttest Panic-Handler                                              => bestanden
 9/19 Selbsttest .rodata schreibgeschuetzt (#PF erwartet)                   => bestanden
10/19 Selbsttest NX: Daten ausfuehren (#PF mit Instruktionsabruf erwartet)  => bestanden
11/19 Selbsttest #GP: nicht kanonische Adresse (kein #PF, sondern #GP)      => bestanden
12/19 Selbsttest #UD: ungueltige Instruktion (ud2)                          => bestanden
13/19 Boot ueber UEFI (OVMF)                                                => bestanden
14/19 Produktname kommt nur aus branding.rs                                 => bestanden
15/19 Architekturgrenze und Codehygiene                                     => bestanden
16/19 Verdraengung: Wechsel ohne freiwilliges yield, Prioritaeten wirken    => bestanden
17/19 Ring 3: unprivilegiertes Programm, Systemaufruf, Schutzwall           => bestanden
18/19 ELF-Lader: Programm aus dem Startdateisystem, Muell wird abgewiesen   => bestanden
19/19 Capabilities: Handle-Negativtests im Boot-Log                         => bestanden
############ ALLE TESTS BESTANDEN ############
EXIT=0
```

Im Boot-Log nachweisbar: Memory-Map, Frame-Statistik, eigene Umschaltung des
Adressraums, Heap-Init mit Testallokation (Box/Vec/String), Timer-Ticks,
30 **erzwungene** Threadwechsel ohne ein einziges `yield`, ein Programm in
Ring 3 (`CS=0x0023 (RPL=3), CPL=3`), ein aus dem Startdateisystem geladenes
ELF64, die Handle-Negativtests und die Bilanz
`Selbsttestbilanz: 166/166 bestanden`.

## Fehlersuche

* Backtrace-Adressen auflösen:
  `llvm-addr2line -e target/x86_64-osum-none/release/osum -f -C 0x…`
  oder in `build/osum.map` nachschlagen.
* Das serielle Log des letzten `--check`-Laufs liegt in `build/boot.log`.
* Hängt QEMU, greift die Zeitgrenze in `run-qemu.sh` (25 s bis zur Abbruchmarke).
* Kein Startdateisystem im Log? Dann fehlte beim Bauen `nasm`, `ld` oder
  `python3` — `build.sh` sagt das in einer Zeile mit `Hinweis:`.
