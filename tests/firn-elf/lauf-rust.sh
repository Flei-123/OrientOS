#!/usr/bin/env bash
# tests/firn-elf/lauf-rust.sh — faehrt den Massstab gegen die RUST-Fassung.
#
# Die Rust-Fassung des Pruefteils steht seit dem 23.08.2026 nicht mehr im
# Kernel; sie liegt als REFERENZMASSSTAB in alter-pruefteil.rs.txt. Dieses
# Skript uebersetzt sie auf dem Host und haelt sie gegen dieselben 53
# Falldateien wie tests/firn-elf/lauf.sh die Firn-Fassung.
#
# Warum das bleibt, statt geloescht zu werden: solange beide Fassungen zu
# jedem Fall dasselbe sagen, ist die Portierung nachweislich VERHALTENSGLEICH.
# Ohne diesen Vergleich wuesste man nur, dass die Neufassung Tests besteht --
# nicht, dass sie sich genauso verhaelt wie das, was sie ersetzt hat.
set -euo pipefail
cd "$(dirname "$0")/../.."

AUS=${1:-build/firn-elf}
mkdir -p "$AUS"

python3 tests/firn-elf/faelle.py tests/firn-elf/faelle >/dev/null

# Den Pruefteil aus der Referenzdatei schneiden (ohne den erklaerenden Kopf).
awk '/>>> PRUEFTEIL ANFANG/{drin=1; next} /<<< PRUEFTEIL ENDE/{drin=0} drin' \
    tests/firn-elf/alter-pruefteil.rs.txt > "$AUS/pruefteil.rs"

ZEILEN=$(wc -l < "$AUS/pruefteil.rs")
if [[ "$ZEILEN" -lt 100 ]]; then
    echo "Referenz-Pruefteil sieht zu kurz aus ($ZEILEN Zeilen)" >&2
    exit 1
fi

PARSER_QUELLE="$PWD/$AUS/pruefteil.rs" \
    rustc --edition 2021 -O -o "$AUS/rust-massstab" tests/firn-elf/rust-massstab.rs

"$AUS/rust-massstab" tests/firn-elf/faelle
