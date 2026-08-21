#!/usr/bin/env bash
# vendor/firn/hole-firnc.sh — besorgt den FESTGENAGELTEN Firn-Uebersetzer.
#
# Warum festgenagelt: Firn wird gerade aktiv weiterentwickelt (Sprachtyp `str`,
# Typ-Aliase, `f32`, spaeter `extern fn` und globaler Zustand). Wuerde osum
# immer gegen den neuesten Stand bauen, waere bei jedem Fehler unklar, ob er
# aus dem Kernel oder aus dem Uebersetzer kommt. Deshalb: EIN Commit, hier
# eingetragen, und nachgezogen wird erst, wenn `./test.sh` gruen ist.
#
# Der Uebersetzer selbst wird NICHT eingecheckt (2,8 MB Binaerdatei pro
# Version in der Historie). Eingecheckt ist nur der Commit-Hash; dieses
# Skript baut daraus.
#
#   ./vendor/firn/hole-firnc.sh          baut, wenn noetig
#   ./vendor/firn/hole-firnc.sh --force  baut in jedem Fall neu
set -euo pipefail
cd "$(dirname "$0")"
HIER=$(pwd)

COMMIT=$(cat COMMIT)
KURZ=${COMMIT:0:8}
# Wo das Firn-Repo liegt. Ueberschreibbar, falls es woanders ausgecheckt ist.
FIRN_REPO=${FIRN_REPO:-$HIER/../../../firn}
# Der Baubaum liegt bewusst AUSSERHALB des Repos: sonst haelt cargo den
# Uebersetzer fuer ein Mitglied unseres Workspace, und .cargo/config.toml
# von osum wuerde ihn fuer x86_64-osum-none statt fuer den Host bauen.
BAU=${FIRN_BAU_DIR:-${TMPDIR:-/tmp}}/firn-pin-$KURZ

FORCE=0
[[ ${1:-} == --force ]] && FORCE=1

if [[ $FORCE -eq 0 && -x $HIER/firnc && -f $HIER/.gebaut && $(cat "$HIER/.gebaut") == "$COMMIT" ]]; then
    echo "firnc ist aktuell ($KURZ)"
    exit 0
fi

if [[ ! -d $FIRN_REPO/.git ]]; then
    echo "Firn-Repo nicht gefunden: $FIRN_REPO" >&2
    echo "Pfad ueber FIRN_REPO=... setzen." >&2
    exit 1
fi

# `git archive` statt `git worktree`: legt NICHTS im Firn-Repo an. Dort laufen
# parallel Arbeitsbaeume anderer Runden, die nicht angefasst werden duerfen.
echo ">> Firn $KURZ auspacken"
rm -rf "$BAU"
mkdir -p "$BAU"
git -C "$FIRN_REPO" archive "$COMMIT" | tar -x -C "$BAU"

# Zielarchitektur ausdruecklich auf den HOST setzen. osums
# .cargo/config.toml stellt x86_64-osum-none als Standardziel ein; ein
# Uebersetzer, der auf dem blanken Kernelziel gebaut wird, findet kein `std`.
HOST=$(rustc -vV | sed -n 's/^host: //p')

echo ">> firnc bauen (Ziel $HOST)"
( cd "$BAU/compiler" && cargo build --release --target "$HOST" >/dev/null )

cp -f "$BAU/compiler/target/$HOST/release/firnc" "$HIER/firnc"
rm -rf "$HIER/lib"
cp -r "$BAU/lib" "$HIER/lib"
echo "$COMMIT" > "$HIER/.gebaut"
echo ">> fertig: vendor/firn/firnc ($KURZ)"
