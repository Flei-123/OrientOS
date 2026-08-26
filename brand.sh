#!/usr/bin/env bash
# Markenaufloesung fuer die Bauskripte. Wird von build.sh, run-osum.sh und
# test.sh eingebunden (source), nie direkt aufgerufen.
#
# Setzt: BRAND, OS_NAME, SLUG, KERNEL_PKG.
#
# WOHER DIE STANDARDMARKE KOMMT — und warum das seit dem 26.08.2026 anders
# ist. Bis zum Kernelwechsel stand sie in `kernel/Cargo.toml` unter
# `[package.metadata.branding]`, weil der Kernel ein Cargo-Projekt dieses
# Repos war. Er ist es nicht mehr (KERNELWECHSEL.md): der Kernel kommt aus
# dem Osum-Repo und weiss von Marken nichts. Also steht die Standardmarke
# jetzt dort, wo die Marken stehen — in `brands/`, und `brands/STANDARD`
# nennt sie beim Namen.
#
# Ein Tippfehler im Markennamen bricht ab und faellt NICHT still auf die
# Standardmarke zurueck.

# Liest `schluessel = "wert"` aus einer flachen TOML-artigen Datei.
# Findet sich nichts, ist das KEIN Fehler (leere Ausgabe, Rueckgabe 0) — die
# Aufrufer haben Ersatzwerte. Ohne das `|| true` wuerde `set -e` in build.sh
# beim ersten fehlenden Feld abbrechen.
brand_feld() {   # brand_feld <datei> <schluessel>
    { sed 's/#.*//' "$1" 2>/dev/null \
        | grep -m1 "^[[:space:]]*$2[[:space:]]*=" \
        | sed 's/.*=[[:space:]]*"\(.*\)".*/\1/'; } || true
}

BRAND="${BRAND:-}"
if [[ -z "$BRAND" ]]; then
    BRAND=$(sed 's/#.*//' brands/STANDARD 2>/dev/null | tr -d '[:space:]')
fi
BRAND_FILE="brands/${BRAND}.toml"
if [[ ! -f "$BRAND_FILE" ]]; then
    echo "unbekannte Marke: ${BRAND:-<keine>}" >&2
    echo "vorhanden: $(ls brands/*.toml 2>/dev/null | xargs -r -n1 basename | sed 's/\.toml$//' | tr '\n' ' ')" >&2
    exit 1
fi
export BRAND
OS_NAME=$(brand_feld "$BRAND_FILE" os-name)
SLUG=$(brand_feld "$BRAND_FILE" slug)

[[ -n "${OS_NAME:-}" ]] || OS_NAME="Unbenannt"
[[ -n "${SLUG:-}" ]]    || SLUG=$(echo "$OS_NAME" | tr '[:upper:]' '[:lower:]')

# Der Kernel heisst in JEDER Marke `osum` — so wie NT in jeder
# Windows-Ausgabe NT heisst. Eine Marke, die das doch anders will, setzt
# `kernel-name`; das benennt dann nur die Datei im Abbild um, nicht das
# Osum-Repo.
KERNEL_PKG=$(brand_feld "$BRAND_FILE" kernel-name)
[[ -n "${KERNEL_PKG:-}" ]] || KERNEL_PKG=osum

export OS_NAME SLUG KERNEL_PKG
