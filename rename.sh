#!/usr/bin/env bash
# Benennt Kernel und Betriebssystem im GESAMTEN Baum um.
#
#   ./rename.sh <neuer-kernel-name> <neuer-os-name>
#   ./rename.sh nova Novaos
#
# Der Kernelname muss ein gueltiger Cargo-Paketname sein (klein, a-z0-9-).
# Danach laeuft ./build.sh zur Gegenprobe. Siehe RENAME.md.
set -euo pipefail
cd "$(dirname "$0")"

if [[ $# -ne 2 ]]; then
    sed -n '2,9p' "$0"
    exit 1
fi

NEW_K="$1"
NEW_OS="$2"

if [[ ! "$NEW_K" =~ ^[a-z][a-z0-9-]*$ ]]; then
    echo "Kernelname muss klein geschrieben sein und zu ^[a-z][a-z0-9-]*$ passen: $NEW_K" >&2
    exit 1
fi
if [[ ! "$NEW_OS" =~ ^[A-Za-z][A-Za-z0-9-]*$ ]]; then
    echo "OS-Name muss zu ^[A-Za-z][A-Za-z0-9-]*$ passen: $NEW_OS" >&2
    exit 1
fi

# Alte Namen aus den Metadaten lesen — nicht raten.
OLD_K=$(grep -m1 '^name = ' kernel/Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
OLD_OS=$(grep -m1 '^os-name = ' kernel/Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
OLD_OS_LC=$(echo "$OLD_OS" | tr '[:upper:]' '[:lower:]')
NEW_OS_LC=$(echo "$NEW_OS" | tr '[:upper:]' '[:lower:]')

echo ">> Kernel: $OLD_K -> $NEW_K"
echo ">> OS    : $OLD_OS -> $NEW_OS"
if [[ "$OLD_K" == "$NEW_K" && "$OLD_OS" == "$NEW_OS" ]]; then
    echo ">> nichts zu tun"
    exit 0
fi

mv_if() { [[ -e "$1" ]] && mv "$1" "$2" && echo "   umbenannt: $1 -> $2"; return 0; }

echo ">> Verzeichnisse und Dateien"
for suffix in mem abi-native abi-posix; do
    mv_if "libs/${OLD_K}-${suffix}" "libs/${NEW_K}-${suffix}"
done
mv_if "x86_64-${OLD_K}-none.json"          "x86_64-${NEW_K}-none.json"
mv_if "x86_64-${OLD_K}-none.json.VERIFIED" "x86_64-${NEW_K}-none.json.VERIFIED"

echo ">> Textersetzung"
# Reihenfolge ist entscheidend: der OS-Name enthaelt den Kernelnamen als
# Teilzeichenkette (karstos <- karst). Erst OS, dann Kernel.
FILES=$(find . -type f \
        -not -path './.git/*' -not -path './target/*' \
        -not -path './vendor/*' -not -path './build/*' \
        -not -name '*.iso' -not -name '*.log' -not -name 'rename.sh')

for f in $FILES; do
    # Nur Textdateien anfassen.
    file --mime-encoding "$f" | grep -q 'binary' && continue
    perl -pi -e "
        s/\Q${OLD_OS}\E/${NEW_OS}/g;
        s/\Q${OLD_OS_LC}\E/${NEW_OS_LC}/g;
        s/\b\Q${OLD_K}\Efs\b/${NEW_K}fs/g;
        s/\b\Q${OLD_K}\E_/${NEW_K}_/g;
        s/\b\Q${OLD_K}\E-/${NEW_K}-/g;
        s/\b\Q${OLD_K}\E\b/${NEW_K}/g;
    " "$f"
done
# Unterstriche in Rust-Pfaden (karst_mem) fangen die Regeln oben nur, wenn der
# alte Name keinen Bindestrich enthaelt. Sicherheitshalber noch einmal gezielt:
OLD_K_US=${OLD_K//-/_}
NEW_K_US=${NEW_K//-/_}
if [[ "$OLD_K_US" != "$OLD_K" ]]; then
    for f in $FILES; do
        file --mime-encoding "$f" | grep -q 'binary' && continue
        perl -pi -e "s/\b\Q${OLD_K_US}\E/${NEW_K_US}/g;" "$f"
    done
fi

echo ">> Cargo.lock neu erzeugen"
rm -f Cargo.lock

echo ">> Gegenprobe: bauen"
./build.sh >/dev/null
echo ">> Gegenprobe: booten"
./run-qemu.sh --check | tail -3

echo
echo ">> fertig. Kernel heisst jetzt '${NEW_K}', das System '${NEW_OS}'."
echo "   Kontrolle: grep -rn '${OLD_K}\\|${OLD_OS}' --exclude-dir={.git,target,vendor,build} . | head"
