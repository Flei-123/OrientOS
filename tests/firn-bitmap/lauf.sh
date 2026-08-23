#!/usr/bin/env bash
# tests/firn-bitmap/lauf.sh -- uebersetzt kernel/firn/bitmap.fi und stellt es
# denselben 23 Fragen wie die Rust-Fassung in libs/osum-mem/src/bitmap.rs.
#
# Das Firn-Objekt wird im KERNELPROFIL uebersetzt -- also genau so, wie es
# spaeter im Kernelabbild landet -- und dann mit dem C-Pruefstand gelinkt.
# Geprueft wird damit dasselbe Objekt, nicht eine host-taugliche Variante.
set -euo pipefail
cd "$(dirname "$0")/../.."

FIRNC=vendor/firn/firnc
[[ -x $FIRNC ]] || ./vendor/firn/hole-firnc.sh

AUS=${1:-build/firn-bitmap-test}
mkdir -p "$(dirname "$AUS")"

FIRNLIB=$PWD/vendor/firn/lib "$FIRNC" -o "$AUS.o" kernel/firn/bitmap.fi

# Freistehend heisst: ausser `osum_panic` darf nichts offen sein. Der
# Pruefstand definiert `osum_panic` selbst und laesst jeden Anschlag als
# fehlgeschlagenen Test durchfallen.
OFFEN=$(nm -u --format=posix "$AUS.o" | awk '{print $1}' | grep -v '^osum_panic$' || true)
if [[ -n $OFFEN ]]; then
    echo "bitmap.fi hat unerlaubte undefinierte Symbole:" >&2
    echo "$OFFEN" >&2
    exit 1
fi

# -no-pie: das Kernelobjekt ist nicht positionsunabhaengig uebersetzt.
cc -O2 -Wall -Wextra -no-pie -o "$AUS" tests/firn-bitmap/pruefstand.c "$AUS.o"
"$AUS"
