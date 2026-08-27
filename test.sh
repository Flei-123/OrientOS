#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only
# Die Abnahme von OrientOS — des GESAMTSYSTEMS.
#
# WAS HIER SEIT DEM 26.08.2026 GEMESSEN WIRD, und was nicht mehr.
# Bis zum Kernelwechsel hat dieser Lauf einen Rust-Kernel gebaut und
# gebootet (cargo, build-std, dreizehn QEMU-Starts). Dieser Kernel ist
# geloescht — er kommt aus dem Osum-Repo, und DORT wird er abgenommen
# (`./test.sh`, 15 Abschnitte, ueber 1100 Zusagen). Dieses Repo misst,
# was DIESES Repo tut:
#
#   * dass der Kernel eine nachvollziehbare HERKUNFT hat und kein Abbild
#     im Baum liegt,
#   * dass kein Rust mehr da ist, wo es keines mehr geben soll,
#   * dass aus Kernel und Programmen ein PRODUKT wird, das ueber BIOS
#     und ueber UEFI startet,
#   * dass in diesem Produkt ein USERLAND laeuft — eine Shell aus einem
#     Boot-Modul, das dieses Repo zusammengestellt hat,
#   * dass die Marken funktionieren (ein Quellbaum, zwei Produkte),
#   * und dass die Dokumente dasselbe sagen wie der Baum.
#
# Jede Zusage hat eine Gegenprobe. Eine Eigenschaft ohne Gegenprobe ist
# eine Behauptung.
#
# Exitcode 0 = alles bestanden.
set -uo pipefail
cd "$(dirname "$0")"

# Markenaufloesung (brand_feld, OS_NAME, SLUG) — dieselbe Quelle wie build.sh.
# shellcheck source=brand.sh
source ./brand.sh
FAIL=0
ZUSAGEN=0
# Die Nachweise stehen als eigene Dateien in tests/step-*.sh (jede ruft
# `step` und `run` selbst). Dadurch kann an mehreren Stellen gleichzeitig
# gearbeitet werden, ohne dass sich zwei Leute in dieser Datei ins Gehege
# kommen.
EXTRA=(tests/step-*.sh)
[[ -e "${EXTRA[0]}" ]] || EXTRA=()
# Die Schritte werden GEZAEHLT, nicht geschaetzt: eine Datei unter tests/
# darf mehr als einen Schritt enthalten, und eine Zahl, die niemand
# nachzaehlt, ist irgendwann falsch (der Lauf endete schon einmal mit
# "11/9").
TOTAL=$(cat "$0" ${EXTRA+"${EXTRA[@]}"} | grep -c '^step "')
NR=0
step() { NR=$((NR + 1)); echo; echo "################ $NR/$TOTAL $* ################"; }
run()  { if "$@"; then echo "  => bestanden"; else echo "  => FEHLGESCHLAGEN"; FAIL=1; fi }
# Jede einzelne Zusage wird gezaehlt. Eine Sammelzahl allein waere zu
# leicht gruen zu bekommen.
ok()   { ZUSAGEN=$((ZUSAGEN + 1)); echo "  [ ok ] $*"; }
nok()  { ZUSAGEN=$((ZUSAGEN + 1)); echo "  [FEHL] $*"; RC=1; }
export -f step run ok nok 2>/dev/null || true

step "Herkunft: der Kernel ist festgenagelt, im Repo liegt kein Abbild"
herkunft() {
    RC=0
    local c f
    if [[ ! -f vendor/osum/COMMIT ]]; then
        nok "vendor/osum/COMMIT fehlt — der Kernel haette keine Herkunft"
        return 1
    fi
    c=$(cat vendor/osum/COMMIT)
    if [[ "$c" =~ ^[0-9a-f]{40}$ ]]; then
        ok "Kernel festgenagelt auf Osum ${c:0:8}"
    else
        nok "vendor/osum/COMMIT ist kein Commit-Hash: $c"
    fi
    f=$(cat vendor/firn/COMMIT 2>/dev/null || echo -)
    if [[ "$f" =~ ^[0-9a-f]{40}$ ]]; then
        ok "der Firn-Uebersetzer dieses Repos ist festgenagelt auf ${f:0:8}"
    else
        nok "vendor/firn/COMMIT ist kein Commit-Hash: $f"
    fi
    # Im Repo darf KEIN Kernelabbild und kein Programm eingecheckt sein.
    # Faende sich eines, waere nicht mehr gesagt, welcher Stand gemessen
    # wurde.
    local schmutz=""
    for f in vendor/osum/osum.mb vendor/osum/osum.mb.elf vendor/osum/mkfs.py; do
        git ls-files --error-unmatch "$f" >/dev/null 2>&1 && schmutz="$schmutz $f"
    done
    if git ls-files 'vendor/osum/bin/*' | grep -q .; then
        schmutz="$schmutz vendor/osum/bin/"
    fi
    if [[ -n "$schmutz" ]]; then
        nok "im Repo liegen gebaute Osum-Artefakte:$schmutz"
    else
        ok "kein Kernelabbild, kein Programm, kein mkfs.py im Repo — nur COMMIT und das Bauskript"
    fi
    # ...und das Bauskript baut wirklich aus genau diesem Commit.
    ./vendor/osum/hole-osum.sh >/dev/null || { nok "hole-osum.sh ist fehlgeschlagen"; return 1; }
    if [[ "$(cat vendor/osum/.gebaut 2>/dev/null)" == "$c" ]]; then
        ok "das gebaute Abbild stammt aus ${c:0:8}"
    else
        nok "das gebaute Abbild stammt nicht aus dem festgenagelten Commit"
    fi
    local n
    n=$(ls vendor/osum/bin 2>/dev/null | wc -l)
    if [[ "$n" -ge 20 ]]; then
        ok "dasselbe Skript hat $n unprivilegierte Programme aus demselben Commit gebaut"
    else
        nok "nur $n Programme in vendor/osum/bin — erwartet mindestens 20"
    fi
    if [[ -f vendor/osum/mkfs.py ]]; then
        ok "und den Dateisystembauer (vendor/osum/mkfs.py) aus demselben Commit"
    else
        nok "vendor/osum/mkfs.py fehlt"
    fi
    return $RC
}
run herkunft

step "Der Rust-Kernel ist weg — und die eine Vorlage steht mit Begruendung da"
kein_rust() {
    RC=0
    local n
    # Gezaehlt wird, was git kennt: eine Datei im Arbeitsverzeichnis, die
    # niemand eingecheckt hat, ist kein Bestandteil des Projekts.
    n=$(git ls-files '*.rs' | grep -v '^vorlage/' | wc -l)
    if [[ "$n" -eq 0 ]]; then
        ok "kein Rust im Baum ausser der Vorlage (0 Dateien)"
    else
        nok "$n Rust-Dateien ausserhalb von vorlage/:"
        git ls-files '*.rs' | grep -v '^vorlage/' | sed 's/^/         /'
    fi
    local d
    for d in kernel/src libs Cargo.toml Cargo.lock .cargo/config.toml \
             x86_64-osum-none.json run-qemu.sh; do
        if git ls-files --error-unmatch "$d" >/dev/null 2>&1; then
            nok "$d ist noch eingecheckt"
        else
            ok "$d ist geloescht"
        fi
    done
    # ...aber die Historie hat es noch. Geloescht heisst nicht vergessen.
    if git log --oneline -1 -- kernel/src >/dev/null 2>&1 \
       && [[ -n "$(git log --oneline -- kernel/src | head -1)" ]]; then
        ok "die Historie kennt kernel/src weiterhin ($(git log --oneline -- kernel/src | wc -l) Commits)"
    else
        nok "die Historie kennt kernel/src nicht mehr — es wurde nicht mit git rm geloescht"
    fi
    # Die C-Dateien des alten Kernels und seiner Pruefstaende.
    n=$(git ls-files '*.c' '*.h' | grep -v '^vendor/limine/' | wc -l)
    if [[ "$n" -eq 0 ]]; then
        ok "keine C-Datei mehr ausser denen des Bootladers (vendor/limine)"
    else
        nok "$n C-Dateien ausserhalb von vendor/limine:"
        git ls-files '*.c' '*.h' | grep -v '^vendor/limine/' | sed 's/^/         /'
    fi
    # DIE EINE VORLAGE. Sie darf da sein, sie darf NICHT gebaut werden,
    # und sie muss sagen, warum sie da ist.
    if [[ -f vorlage/arch_iface.rs ]]; then
        ok "vorlage/arch_iface.rs steht da ($(wc -l < vorlage/arch_iface.rs) Zeilen)"
    else
        nok "vorlage/arch_iface.rs fehlt — der offene Punkt aus KERNELWECHSEL.md § 4.2 waere still verschwunden"
        return 1
    fi
    if grep -q 'VORLAGE — KEIN GEBAUTER CODE' vorlage/arch_iface.rs; then
        ok "sie sagt selbst, dass sie nicht uebersetzt wird"
    else
        nok "vorlage/arch_iface.rs erklaert nicht, dass sie Vorlage ist"
    fi
    if [[ ! -f Cargo.toml && ! -f kernel/Cargo.toml ]]; then
        ok "und es gibt nichts, was sie uebersetzen koennte (kein Cargo.toml im Baum)"
    else
        nok "es gibt noch ein Cargo.toml — die Vorlage waere kein Text, sondern Code"
    fi
    if grep -qF 'vorlage/arch_iface.rs' KERNELWECHSEL.md && grep -qF 'vorlage/arch_iface.rs' ROADMAP.md; then
        ok "KERNELWECHSEL.md und ROADMAP.md nennen sie beide"
    else
        nok "die Vorlage wird in KERNELWECHSEL.md oder ROADMAP.md nicht genannt"
    fi
    return $RC
}
run kein_rust

for f in "${EXTRA[@]}"; do
    # shellcheck source=/dev/null
    source "$f"
done

echo
echo "=================================================================="
if [[ $FAIL -eq 0 ]]; then
    echo "############ ALLE $TOTAL SCHRITTE BESTANDEN, $ZUSAGEN Zusagen ############"
else
    echo "############ TESTS FEHLGESCHLAGEN ($ZUSAGEN Zusagen geprueft) ############"
fi
exit $FAIL
