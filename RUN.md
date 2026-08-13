# RUN — Karstos bauen, starten, prüfen

Kernel `karst`, Betriebssystem `Karstos`, Rust `no_std`, Ziel `x86_64-karst-none`.
Alle Befehle laufen im Projektwurzelverzeichnis, alle Pfade sind relativ.

## Voraussetzungen

```sh
rustup toolchain install nightly && rustup default nightly
rustup component add rust-src llvm-tools
sudo apt install qemu-system-x86 xorriso mtools nasm ovmf
```

Netzzugang zu crates.io ist nicht nötig; Limine liegt vorgebaut in `vendor/limine/`.
`build-std` steht bewusst als Flag in `build.sh` und **nicht** in
`.cargo/config.toml` — sonst scheitern die Host-Tests an `duplicate lang item`.

## Der kurze Weg

```sh
./build.sh          # Kernel + bootfähiges build/karstos.iso
./run-qemu.sh       # interaktiver Start, serielle Ausgabe im Terminal (Strg-C beendet)
./test.sh           # der vollständige Nachweis: 14 Schritte, Exitcode 0 = alles grün
```

## Einzelne Schalter

| Befehl | Wirkung |
|---|---|
| `./build.sh` | Release-Build + ISO + Symbolkarte `build/karst.map` |
| `./build.sh --debug` | Debug-Build |
| `./build.sh --no-posix` | Gegenprobe ohne POSIX-Schicht (`--no-default-features`) |
| `./run-qemu.sh --check` | bootet und prüft 38 Merkmale im Log, Exitcode 0/1 |
| `./run-qemu.sh --check --no-posix` | muss „posix-Schicht NICHT einkompiliert“ melden |
| `./run-qemu.sh --check --uefi` | Boot über OVMF statt SeaBIOS |
| `./run-qemu.sh --test-pagefault` | absichtlicher `#PF`, CR2 und Ursache im Klartext |
| `./run-qemu.sh --test-doublefault` | **echter** `#DF` auf eigenem IST-Stapel (kein `int 8`) |
| `./run-qemu.sh --test-panic` | Panic-Handler mit Backtrace |
| `./run-qemu.sh --test-rodata` | Schreibversuch auf `.rodata` muss `#PF` auslösen |
| `./run-qemu.sh --test-nx` | Ausführungsversuch auf Daten muss `#PF` auslösen |
| `./run-qemu.sh --test-gp` | nicht kanonische Adresse muss `#GP` auslösen |
| `./run-qemu.sh --test-ud` | ungültige Instruktion (`ud2`) |

Host-Tests der hardwarefreien Logik allein (129 Tests, laufen in Sekunden):

```sh
cargo test --target x86_64-unknown-linux-gnu \
    -p karst-mem -p karst-abi-native -p karst-abi-posix
```

## Was `./test.sh` abdeckt

1. Host-Tests (129) · 2. Build mit POSIX · 3. Build ohne POSIX ·
4. Boot BIOS · 5. Boot ohne POSIX · 6. `#PF` · 7. echter `#DF` · 8. Panic ·
9. `.rodata` schreibgeschützt · 10. NX · 11. `#GP` · 12. `#UD` ·
13. Boot über UEFI · 14. Architekturgrenze und Codehygiene

Schritt 14 prüft unter anderem, dass **keine** x86-Details außerhalb
`kernel/src/arch/` im Code stehen und dass jedes `test-*`-Feature aus
`kernel/Cargo.toml` auch einen Schalter in `run-qemu.sh` hat.

## Belegter Zustand (letzter vollständiger Lauf)

```
$ ./test.sh ; echo "EXIT=$?"
 1/14 Host-Tests der architekturunabhaengigen Crates    => bestanden
 2/14 Kernel baut (Release, mit POSIX)                  => bestanden
 3/14 Kernel baut OHNE POSIX-Schicht                    => bestanden
 4/14 Boot in QEMU (BIOS)                               => bestanden
 5/14 Boot in QEMU ohne POSIX-Schicht                   => bestanden
 6/14 Selbsttest Page Fault                             => bestanden
 7/14 Selbsttest Double Fault (echter #DF)              => bestanden
 8/14 Selbsttest Panic-Handler                          => bestanden
 9/14 Selbsttest .rodata schreibgeschuetzt              => bestanden
10/14 Selbsttest NX                                     => bestanden
11/14 Selbsttest #GP                                    => bestanden
12/14 Selbsttest #UD                                    => bestanden
13/14 Boot ueber UEFI (OVMF)                            => bestanden
14/14 Architekturgrenze und Codehygiene                 => bestanden
############ ALLE TESTS BESTANDEN ############
EXIT=0
```

Im Boot-Log nachweisbar: Memory-Map, Frame-Statistik, eigene CR3-Umschaltung,
Heap-Init mit Testallokation (Box/Vec/String) samt Bytes vor/nach Freigabe,
mindestens 5 Timer-Ticks, drei Scheduler-Selbsttests und die Bilanz
`Selbsttestbilanz: 79/79 bestanden`.

## Fehlersuche

* Backtrace-Adressen auflösen:
  `llvm-addr2line -e target/x86_64-karst-none/release/karst -f -C 0x…`
  oder in `build/karst.map` nachschlagen.
* Das serielle Log des letzten `--check`-Laufs liegt in `build/boot.log`.
* Hängt QEMU, greift die Zeitgrenze in `run-qemu.sh` (25 s bis zur Abbruchmarke).
