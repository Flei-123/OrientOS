#!/usr/bin/env bash
# tests/firn-elf/lauf.sh — uebersetzt kernel/firn/elf.fi und stellt es
# denselben 53 Fragen wie die Rust-Fassung (tests/firn-elf/lauf-rust.sh).
#
# Das Firn-Objekt wird im KERNELPROFIL uebersetzt — also genau so, wie es im
# Kernelabbild landet — und dann mit dem C-Pruefstand gelinkt.
set -euo pipefail
cd "$(dirname "$0")/../.."

FIRNC=vendor/firn/firnc
[[ -x $FIRNC ]] || ./vendor/firn/hole-firnc.sh

AUS=${1:-build/firn-elf-test}
mkdir -p "$(dirname "$AUS")"

# Die Faelle werden bei jedem Lauf neu erzeugt: so kann kein alter Stand
# durchrutschen, wenn jemand faelle.py aendert und das Erzeugen vergisst.
python3 tests/firn-elf/faelle.py tests/firn-elf/faelle >/dev/null

FIRNLIB=$PWD/vendor/firn/lib "$FIRNC" -o "$AUS.o" kernel/firn/elf.fi

OFFEN=$(nm -u --format=posix "$AUS.o" | awk '{print $1}' | grep -v '^osum_panic$' || true)
if [[ -n $OFFEN ]]; then
    echo "elf.fi hat unerlaubte undefinierte Symbole:" >&2
    echo "$OFFEN" >&2
    exit 1
fi

cc -O2 -Wall -Wextra -no-pie -o "$AUS" tests/firn-elf/pruefstand.c "$AUS.o"
"$AUS" tests/firn-elf/faelle
