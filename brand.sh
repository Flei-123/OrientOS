#!/usr/bin/env bash
# Markenaufloesung fuer die Bauskripte. Wird von build.sh und run-qemu.sh
# eingebunden (source), nie direkt aufgerufen.
#
# Setzt: BRAND (Markenname oder leer), OS_NAME, SLUG.
# Quelle: brands/$BRAND.toml, sonst [package.metadata.branding] in
# kernel/Cargo.toml. Dieselbe Reihenfolge wie in kernel/build.rs.
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
if [[ -n "$BRAND" ]]; then
    BRAND_FILE="brands/${BRAND}.toml"
    if [[ ! -f "$BRAND_FILE" ]]; then
        echo "unbekannte Marke: $BRAND" >&2
        echo "vorhanden: $(ls brands/*.toml 2>/dev/null | xargs -r -n1 basename | sed 's/\.toml$//' | tr '\n' ' ')" >&2
        exit 1
    fi
    OS_NAME=$(brand_feld "$BRAND_FILE" os-name)
    SLUG=$(brand_feld "$BRAND_FILE" slug)
    export BRAND
else
    OS_NAME=$(brand_feld kernel/Cargo.toml os-name)
    SLUG=$(brand_feld kernel/Cargo.toml slug)
fi

[[ -n "${OS_NAME:-}" ]] || OS_NAME="Unbenannt"
[[ -n "${SLUG:-}" ]]    || SLUG=$(echo "$OS_NAME" | tr '[:upper:]' '[:lower:]')
# Kernel-Paketname (fuer Pruefmuster, die den Banner betreffen).
KERNEL_PKG=$(brand_feld kernel/Cargo.toml kernel-name)
if [[ -z "${KERNEL_PKG:-}" ]]; then
    KERNEL_PKG=$({ sed 's/#.*//' kernel/Cargo.toml | grep -m1 '^name = ' | sed 's/.*"\(.*\)".*/\1/'; } || true)
fi
export OS_NAME SLUG KERNEL_PKG
