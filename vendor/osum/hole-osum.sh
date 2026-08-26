#!/usr/bin/env bash
# vendor/osum/hole-osum.sh — besorgt den FESTGENAGELTEN Osum-Kernel und
# das Userland, das dazugehoert.
#
# WARUM DIESE DATEI EXISTIERT. Der Kernel von OrientOS wird nicht in
# diesem Repo geschrieben. Er ist ein eigenes Projekt (Osum, in Firn), und
# OrientOS ist das System DRUMHERUM: Marken, Userland-Zusammenstellung,
# Paketformat, die Abnahme des Ganzen. Dieselbe Teilung, die dieses Repo
# schon fuer den Firn-Uebersetzer benutzt (vendor/firn/) — und aus
# demselben Grund:
#
#   * FESTGENAGELT. Osum wird weiterentwickelt. Wuerde OrientOS immer
#     gegen den neuesten Stand bauen, waere bei jedem Fehler unklar, ob er
#     aus dem System oder aus dem Kernel kommt. Deshalb: EIN Commit, hier
#     eingetragen in vendor/osum/COMMIT, nachgezogen wird erst, wenn
#     ./test.sh gruen ist.
#   * MIT HERKUNFT. Es wird nichts kopiert, dessen Ursprung man spaeter
#     raten muss. Der Commit-Hash steht im Repo, das Abbild nicht.
#   * MIT SEINEM EIGENEN UEBERSETZER. Osum nagelt seinerseits einen
#     Firn-Commit fest (dort vendor/firn/COMMIT), und das ist ein ANDERER
#     als der von OrientOS. Der Kernel wird deshalb genau so gebaut, wie
#     sein eigenes Repo es sagt — nicht mit dem Uebersetzer dieses Repos.
#     Zwei Projekte, zwei Nagel, keine stille Vermischung.
#
# WAS SEIT DEM 26.08.2026 DAZUKAM: NICHT NUR DER KERN.
# Osums Runde K10 hat das Boot-Modul gebracht — der Kernel nimmt ein
# OFS-Dateisystem entgegen, das der Lader neben ihn legt, prueft dessen
# CRC32 und mountet es als Wurzel. Damit kann ein ISO ein Userland
# tragen, und dieses Skript baut deshalb auch die unprivilegierten
# Programme aus demselben festgenagelten Commit. WELCHE davon ins Produkt
# kommen, entscheidet OrientOS (`userland/PROGRAMME`) — gebaut werden
# hier alle.
#
# Eingecheckt ist nur COMMIT und dieses Skript; alles andere wird gebaut.
#
#   ./vendor/osum/hole-osum.sh          baut, wenn noetig
#   ./vendor/osum/hole-osum.sh --force  baut in jedem Fall neu
#
# Ergebnis (alles in .gitignore):
#   vendor/osum/osum.mb      Multiboot-Abbild (ELF32-Huelle)
#   vendor/osum/osum.mb.elf  dieselbe Sache als ELF64, mit Symbolen
#   vendor/osum/bin/*        die unprivilegierten Programme, ELF64 statisch
#   vendor/osum/mkfs.py      der Dateisystembauer aus demselben Commit
#   vendor/osum/.gebaut      der Commit, aus dem das alles entstand
set -euo pipefail
cd "$(dirname "$0")"
HIER=$(pwd)

COMMIT=$(cat COMMIT)
KURZ=${COMMIT:0:8}

FORCE=0
[[ ${1:-} == --force ]] && FORCE=1

if [[ $FORCE -eq 0 && -f $HIER/osum.mb && -f $HIER/mkfs.py && -d $HIER/bin \
      && -f $HIER/.gebaut && $(cat "$HIER/.gebaut") == "$COMMIT" ]]; then
    echo "Osum ist aktuell ($KURZ, $(ls "$HIER/bin" | wc -l) Programme)"
    exit 0
fi

# --- Wo liegt das Osum-Repo? Nur zum Bauen noetig.
kandidaten=()
[[ -n ${OSUM_REPO:-} ]] && kandidaten+=("$OSUM_REPO")
kandidaten+=("$HIER/../../../osum" "$HIER/../../osum" "$HOME/osum")
OSUM=""
for k in "${kandidaten[@]}"; do
    if [[ -d $k/.git ]] && git -C "$k" cat-file -e "$COMMIT^{commit}" 2>/dev/null; then
        OSUM=$(cd "$k" && pwd); break
    fi
done
if [[ -z $OSUM ]]; then
    echo "Das Osum-Repo mit dem Commit $KURZ wurde nicht gefunden." >&2
    echo "Gesucht in: ${kandidaten[*]}" >&2
    echo "Pfad ueber OSUM_REPO=/pfad/zu/osum setzen." >&2
    exit 1
fi

# Wo liegt Firn? Osum braucht es, um SEINEN festgenagelten Uebersetzer zu
# bauen. Der Pfad wird durchgereicht, weil der Baubaum ausserhalb beider
# Repos liegt und die Suchpfade von Osums eigenem Skript dort nicht mehr
# aufgehen.
FIRN=""
for k in ${FIRN_REPO:-} "$HIER/../../../firn" "$HIER/../../firn" "$HOME/firn"; do
    [[ -d ${k:-}/.git ]] && { FIRN=$(cd "$k" && pwd); break; }
done
if [[ -z $FIRN ]]; then
    echo "Das Firn-Repo wurde nicht gefunden (fuer Osums Uebersetzer)." >&2
    echo "Pfad ueber FIRN_REPO=/pfad/zu/firn setzen." >&2
    exit 1
fi

# Der Baubaum liegt bewusst AUSSERHALB beider Repos. `git archive` statt
# `git worktree`: legt NICHTS im Osum-Repo an — dort wird parallel
# gearbeitet.
BAU=${OSUM_BAU_DIR:-${TMPDIR:-/tmp}}/osum-pin-$KURZ
echo ">> Osum $KURZ aus $OSUM auspacken"
rm -rf "$BAU"
mkdir -p "$BAU"
git -C "$OSUM" archive "$COMMIT" | tar -x -C "$BAU"

echo ">> Osums eigenen Firn-Uebersetzer bauen ($(cut -c1-8 "$BAU/vendor/firn/COMMIT"))"
( cd "$BAU" && FIRN_REPO="$FIRN" bash vendor/firn/hole-firnc.sh >/dev/null )

echo ">> Kernel bauen (tools/build-kernel.sh aus dem Osum-Repo)"
( cd "$BAU" && bash tools/build-kernel.sh "$BAU/osum.mb" )

# --- Das Userland. Dieselben Schritte wie in Osums tools/userland/run.sh:
# ein Startstueck in Assembler, je Programm eine Firn-Uebersetzungseinheit,
# statisch gebunden gegen kernel/user/user.ld. Kein libc-Aufsatz, keine
# dynamische Bindung — der Lader dieses Kernels bindet nicht.
echo ">> Userland bauen (unprivilegierte Programme aus demselben Commit)"
rm -rf "$HIER/bin"
mkdir -p "$HIER/bin"
(
    cd "$BAU"
    export FIRNLIB="$BAU/lib"
    as --64 -o crt.o kernel/user/crt.s
    n=0
    for f in kernel/user/*.fi; do
        p=$(basename "$f" .fi)
        # ulib ist die gemeinsame Bibliothek der Programme, kein Programm.
        [[ $p == ulib ]] && continue
        vendor/firn/bin/firnc "$f" -o "$p.o" >/dev/null 2>&1 || continue
        ld -T kernel/user/user.ld --defsym=USER_ENTRY="_F0.u_start" \
           -o "$p.elf" crt.o "$p.o" 2>/dev/null || continue
        strip --strip-all "$p.elf"
        cp -f "$p.elf" "$HIER/bin/$p"
        n=$((n + 1))
    done
    echo "   $n Programme"
)
cp -f "$BAU/tools/osum/mkfs.py" "$HIER/mkfs.py"

cp -f "$BAU/osum.mb" "$HIER/osum.mb"
cp -f "$BAU/osum.mb.elf" "$HIER/osum.mb.elf"
echo "$COMMIT" > "$HIER/.gebaut"
rm -rf "$BAU"
echo ">> fertig: vendor/osum/osum.mb ($(stat -c%s "$HIER/osum.mb") Oktette, $KURZ),"
echo "   vendor/osum/bin/ ($(ls "$HIER/bin" | wc -l) Programme), vendor/osum/mkfs.py"
