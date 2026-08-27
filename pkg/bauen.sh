#!/usr/bin/env bash
# pkg/bauen.sh — aus dem festgenagelten Osum-Stand PAKETE machen.
#
# WAS HIER ZU EINEM PAKET WIRD, und woher es kommt. Osums Runde K15 legt
# je Programm ein Buendel an: `assets/apps/<name>.prog/` mit INFO
# (Anzeigename, Beschreibung, Schluesselwoerter), `symbol.txt` und
# `daten/`. `vendor/osum/hole-osum.sh` holt beides aus DEMSELBEN
# festgenagelten Commit — die Programme nach `vendor/osum/bin/`, die
# Buendeldaten nach `vendor/osum/apps/`.
#
# Dieses Skript setzt sie zusammen:
#
#     vendor/osum/apps/<name>.prog/INFO       Anzeigename, keys, fassung
#     vendor/osum/apps/<name>.prog/start.txt  WELCHES Programm `start` ist
#     vendor/osum/bin/<programm>              die ausfuehrbare Datei
#            |
#            +-> build/pakete/<name>.opk
#
# DER ANZEIGENAME WIRD NICHT ABGESCHRIEBEN. Er steht in Osums INFO, und
# er wird von dort gelesen — dieselbe Regel, die Osum selbst aufstellt
# („eine Beschriftung, die einkompiliert ist, laesst sich nicht
# austauschen"). Ein Rezept in `pkg/rezepte/` waere die zweite Stelle,
# an der derselbe Name steht, und die zweite Stelle ist immer die, die
# irgendwann nicht mehr stimmt. Deshalb entstehen die Rezepte HIER, beim
# Bauen, und werden nicht eingecheckt.
#
#   ./pkg/bauen.sh                 alle Buendel als Pakete + Quelle
#   ./pkg/bauen.sh --ohne-quelle   nur die Pakete
set -euo pipefail
cd "$(dirname "$0")/.."

AUS=build/pakete
QUELLE=build/quelle
SCHLUESSEL=build/schluessel
MIT_QUELLE=1
[[ ${1:-} == --ohne-quelle ]] && MIT_QUELLE=0

test -d vendor/osum/apps || {
    echo "vendor/osum/apps fehlt — ./vendor/osum/hole-osum.sh laufen lassen" >&2
    exit 1; }

rm -rf "$AUS"; mkdir -p "$AUS"

# `start.txt` nennt die ausfuehrbare Datei als absoluten Pfad im
# GASTSYSTEM (`/bin/explorer`). Uebersetzt wird das hier in den Pfad im
# Baubaum — die Datei stammt aus demselben Commit.
feld() { sed 's/#.*//' "$1" | grep -m1 "^$2=" | cut -d= -f2- | sed 's/[[:space:]]*$//'; }

n=0
for d in vendor/osum/apps/*.prog; do
    [[ -d $d ]] || continue
    voll=$(basename "$d")            # explorer.prog
    name=${voll%.prog}               # explorer
    prog=$(grep -v '^#' "$d/start.txt" | grep -m1 '^/' | tr -d '[:space:]')
    datei=vendor/osum/bin/$(basename "$prog")
    if [[ ! -f $datei ]]; then
        echo "   $name uebersprungen: $prog gibt es nicht in vendor/osum/bin" >&2
        continue
    fi
    rez=$AUS/$name.rezept
    {
        echo "# Erzeugt von pkg/bauen.sh aus vendor/osum/apps/$voll."
        echo "# NICHT von Hand aendern — der Anzeigename steht in Osums INFO."
        echo "name=$name"
        echo "fassung=$(feld "$d/INFO" fassung).0.0"
        echo "titel=$(feld "$d/INFO" name)"
        echo "info=$(feld "$d/INFO" info)"
        echo "keys=$(feld "$d/INFO" keys)"
        # DIE DREI TOEPFE PLUS DIE KONSOLE. Was eine Anwendung bekommt,
        # steht im Paket und nicht in ihrem Quelltext (PACKAGING.md § 7).
        echo "handle=config"
        echo "handle=state"
        echo "handle=cache"
        echo "handle=konsole"
        echo "datei=start ../../$datei"
        echo "datei=INFO ../../$d/INFO"
        [[ -f $d/symbol ]] && echo "datei=symbol ../../$d/symbol"
        if [[ -d $d/daten ]]; then
            for f in "$d"/daten/*; do
                [[ -f $f ]] && echo "datei=daten/$(basename "$f") ../../$f"
            done
        fi
    } > "$rez"
    python3 pkg/opk.py bauen "$rez" -o "$AUS/$name.opk"
    n=$((n + 1))
done

# Von Hand geschriebene Rezepte, falls es welche gibt. Sie sind der Weg
# fuer alles, was KEIN Osum-Buendel ist.
shopt -s nullglob
for rez in pkg/rezepte/*.rezept; do
    name=$(grep -m1 '^name=' "$rez" | cut -d= -f2)
    python3 pkg/opk.py bauen "$rez" -o "$AUS/$name.opk"
    n=$((n + 1))
done
shopt -u nullglob

echo ">> $n Paket(e) in $AUS"

if [[ $MIT_QUELLE -eq 1 ]]; then
    # DER SCHLUESSEL WIRD NICHT EINGECHECKT und auch nicht wiederverwendet:
    # er entsteht bei jedem Bau neu. Fuer eine Quelle, die wirklich
    # verteilt, waere das falsch — dann muss der oeffentliche Teil fest
    # sein, sonst hat die Signatur keinen Wert. Solange die Quelle im
    # Bauverzeichnis liegt und niemand von aussen daraus installiert, ist
    # ein Schluessel je Lauf ehrlicher als einer, der im Repo liegt und
    # damit ohnehin niemandem gehoert.
    rm -rf "$QUELLE" "$SCHLUESSEL"
    mkdir -p "$QUELLE"
    python3 pkg/opk.py schluessel "$SCHLUESSEL" >/dev/null
    cp "$AUS"/*.opk "$QUELLE"/
    python3 pkg/opk.py quelle "$QUELLE" --schluessel "$SCHLUESSEL/geheim.key"
fi
