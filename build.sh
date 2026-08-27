#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only
# OrientOS — das bootfaehige Produkt bauen.
#
# WAS DIESES SKRIPT SEIT DEM 26.08.2026 IST, und was es nicht mehr ist.
# Bis zum Kernelwechsel hat es einen Rust-Kernel uebersetzt (cargo,
# build-std, eigenes Target, Startdateisystem in einem eigenen Format).
# Dieser Kernel ist geloescht; er kommt aus dem Osum-Repo, in Firn,
# festgenagelt ueber `vendor/osum/COMMIT` und gebaut mit SEINEM eigenen
# Uebersetzer. Was hier passiert, ist deshalb kein Uebersetzen mehr,
# sondern ZUSAMMENSTELLEN — genau die Rolle, die OrientOS nach dem
# Wechsel hat (KERNELWECHSEL.md):
#
#   1. den festgenagelten Kernel holen (vendor/osum/hole-osum.sh),
#   2. aus den unprivilegierten Programmen desselben Commits und der
#      Liste in `userland/PROGRAMME` ein OFS-Dateisystem bauen,
#   3. dessen CRC32 rechnen und dem Kernel auf der Kommandozeile nennen,
#   4. Kernel und Dateisystem mit Limine zu einem ISO packen, das ueber
#      BIOS und ueber UEFI startet.
#
# DAS DATEISYSTEM IST EIN BOOT-MODUL, und das ist der Grund, warum das
# Produkt ueberhaupt ein Userland hat: ein ISO hat keine Platte, und was
# ein Multiboot-Lader neben den Kern legen kann, ist ein Modul. Osums
# Runde K10 nimmt eines entgegen, prueft die Summe und mountet es als
# Wurzel (dort `kernel/bootmod.fi`).
#
#   ./build.sh                     Produkt-ISO: build/<slug>.iso
#   ./build.sh --brand xoffi       andere Marke, gleicher Quelltext
#   ./build.sh --cmdline "..."     eigene Kommandozeile fuer den Kernel
#   ./build.sh --ohne-userland     GEGENPROBE: ISO ohne das Modul
#   ./build.sh --kaputte-summe     GEGENPROBE: falsche CRC32 im Aufruf
#   ./build.sh --dazu /t/x.sh=datei   eine weitere Datei ins Dateisystem
#                                  (mehrfach; Testschritte legen so ihre
#                                  Faelle hinein, ohne PROGRAMME anzufassen)
set -euo pipefail
cd "$(dirname "$0")"

BRAND="${BRAND:-}"
MIT_USERLAND=1
KAPUTT=0
DAZU=()
# Was der Osum-Kernel auf der Kommandozeile bekommt. Die Woerter sind in
# seinem `kmain.fi`/`mode_of`, `fb.parse`, `guard.parse` und
# `bootmod.parse` dokumentiert. `modfs` sagt: nimm das Boot-Modul als
# Wurzelplatte; `osum` sagt: starte /bin/sh davon.
CMDLINE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --brand) BRAND="$2"; shift 2 ;;
        --cmdline) CMDLINE="$2"; shift 2 ;;
        --ohne-userland) MIT_USERLAND=0; shift ;;
        --kaputte-summe) KAPUTT=1; shift ;;
        --dazu) DAZU+=("$2"); shift 2 ;;
        -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
        *) echo "unbekannte Option: $1" >&2; exit 1 ;;
    esac
done

# shellcheck source=brand.sh
source ./brand.sh
echo ">> Marke: ${OS_NAME} (${SLUG}, brands/${BRAND}.toml)"

# ---------------------------------------------------- 1. der Kernel
./vendor/osum/hole-osum.sh
KERNEL=vendor/osum/osum.mb
test -f "$KERNEL" || { echo "Osum-Abbild fehlt: $KERNEL" >&2; exit 1; }

ROOT=build/isoroot
rm -rf "$ROOT"
mkdir -p "$ROOT/boot/limine" "$ROOT/EFI/BOOT"
cp "$KERNEL" "$ROOT/boot/${KERNEL_PKG}"

# -------------------------------------------------- 2. das Userland
#
# `userland/PROGRAMME` sagt, WAS ins Produkt kommt; gebaut hat die
# Programme das Osum-Repo (vendor/osum/bin/). Das ist die Trennung, um
# die es geht: Osum liefert die Programme, OrientOS stellt das Produkt
# zusammen. Eine Marke, die ein anderes Userland will, bekommt spaeter
# eine eigene Liste — der Quelltext bleibt derselbe.
IMG=""
CRC=""
if [[ $MIT_USERLAND -eq 1 ]]; then
    test -x vendor/osum/mkfs.py -o -f vendor/osum/mkfs.py \
        || { echo "vendor/osum/mkfs.py fehlt — hole-osum.sh neu laufen lassen" >&2; exit 1; }
    BLOCKS=$(sed 's/#.*//' userland/PROGRAMME | grep -m1 '^bloecke:' \
             | sed 's/^bloecke:[[:space:]]*//' | tr -d '[:space:]')
    [[ -n "$BLOCKS" ]] || BLOCKS=4096
    # Die Groesse der Inodetabelle. Sie steht seit Osums zweitem
    # K15-Nachtrag im Superblock und wird von `fs.mount` von dort
    # gelesen; ohne Angabe bleiben es 128, und ein Abbild ohne
    # `--inodes=` ist Oktett fuer Oktett dasselbe wie vorher. Ein
    # Paketbaum braucht mehr Namen als 128: je Paket ein Store-Eintrag
    # mit seinen Dateien, dasselbe noch einmal unter /apps, drei Toepfe
    # je Nutzer und je Generation zwei Dateien.
    INODES=$(sed 's/#.*//' userland/PROGRAMME | grep -m1 '^inodes:' \
             | sed 's/^inodes:[[:space:]]*//' | tr -d '[:space:]')
    SPEC=()
    [[ -n "$INODES" ]] && SPEC+=("--inodes=$INODES")
    SPEC+=("/bin/")
    PAKETE=()
    FEHLT=""
    while read -r zeile; do
        zeile=${zeile%%#*}
        zeile=$(echo "$zeile" | xargs || true)
        [[ -z "$zeile" ]] && continue
        case "$zeile" in
            bloecke:*|inodes:*) continue ;;
            paket\ *)
                # `paket <name>` — wird weiter unten in EINEM Zug
                # installiert; hier nur gesammelt, weil die Reihenfolge
                # der Angaben fuer mkfs.py sonst nicht stimmt.
                set -- $zeile
                PAKETE+=("$2")
                ;;
            datei\ *)
                # `datei <zielpfad> <quelle>` — eine Datei aus DIESEM Repo.
                set -- $zeile
                SPEC+=("$2=$3")
                [[ -f "$3" ]] || FEHLT="$FEHLT $3"
                ;;
            verzeichnis\ *)
                # mkfs.py erkennt ein Verzeichnis am Schraegstrich am Ende.
                set -- $zeile
                SPEC+=("${2%/}/")
                ;;
            *)
                if [[ -f "vendor/osum/bin/$zeile" ]]; then
                    SPEC+=("/bin/$zeile=vendor/osum/bin/$zeile")
                else
                    FEHLT="$FEHLT vendor/osum/bin/$zeile"
                fi
                ;;
        esac
    done < userland/PROGRAMME
    # ------------------------------------------------------ die Pakete
    #
    # DAS IST DER UNTERSCHIED ZU `/bin`. Ein Programm unter `/bin` wird
    # hier kopiert; ein Paket wird INSTALLIERT — von `pkg/opk.py` in
    # eine Wurzel unter `build/wurzel`, mit Store, Generation, den drei
    # Toepfen und harten Verweisen. Was ins Abbild kommt, ist das
    # ERGEBNIS dieser Installation und nicht eine Liste von Dateien.
    # Uebersetzt wird es von `pkg/mkfs-spec.py`, das die harten Verweise
    # als `<neu>@<vorhanden>` weitergibt — sonst laege der Explorer
    # zweimal auf einer Platte von zwei Megaoktett.
    if [[ ${#PAKETE[@]} -gt 0 ]]; then
        mkdir -p build
        ./pkg/bauen.sh > build/pakete.log 2>&1 || {
            echo "pkg/bauen.sh fehlgeschlagen:" >&2; cat build/pakete.log >&2; exit 1; }
        WURZEL=build/wurzel
        rm -rf "$WURZEL"
        for p in "${PAKETE[@]}"; do
            test -f "build/pakete/$p.opk" || {
                echo "userland/PROGRAMME nennt das Paket '$p', das es nicht gibt" >&2
                exit 1; }
            python3 pkg/opk.py installieren --wurzel "$WURZEL" \
                    --quelle build/quelle --schluessel build/quelle/oeffentlich.key \
                    "$p" >> build/pakete.log 2>&1 || {
                echo "opk installieren $p fehlgeschlagen:" >&2
                tail -5 build/pakete.log >&2; exit 1; }
        done
        while read -r z; do SPEC+=("$z"); done < <(python3 pkg/mkfs-spec.py "$WURZEL")
        echo ">> Pakete: ${#PAKETE[@]} installiert (${PAKETE[*]}), Generation $(cat "$WURZEL/system/AKTUELL")"
    fi

    # Was ein Testschritt zusaetzlich hineinlegen will. Das laeuft NICHT
    # ueber userland/PROGRAMME: die Liste beschreibt das Produkt, nicht
    # den Aufbau eines einzelnen Nachweises.
    for d in ${DAZU+"${DAZU[@]}"}; do
        SPEC+=("$d")
        # Ein Eintrag OHNE `=` ist ein Verzeichnis (mkfs.py erkennt es am
        # Schraegstrich); nur bei einem mit `=` gibt es eine Quelldatei,
        # die es geben muss.
        case "$d" in
            *=*) q=${d#*=}; [[ -f "$q" ]] || FEHLT="$FEHLT $q" ;;
        esac
    done
    if [[ -n "$FEHLT" ]]; then
        echo "userland/PROGRAMME nennt, was es nicht gibt:$FEHLT" >&2
        exit 1
    fi
    mkdir -p build
    IMG="build/${SLUG}-userland.img"
    python3 vendor/osum/mkfs.py build "$IMG" "$BLOCKS" "${SPEC[@]}" >/dev/null
    CRC=$(python3 -c 'import zlib,sys;print("%08x"%zlib.crc32(open(sys.argv[1],"rb").read()))' "$IMG")
    if [[ $KAPUTT -eq 1 ]]; then
        # Die Gegenprobe zur Pruefsumme: der Kernel bekommt eine Summe
        # genannt, die nicht zu den Daten passt, und MUSS das Modul dann
        # liegen lassen.
        CRC=deadbeef
    fi
    cp "$IMG" "$ROOT/boot/${SLUG}-userland.img"
    ANZ=0
    for e in "${SPEC[@]}"; do [[ $e == --* ]] || ANZ=$((ANZ + 1)); done
    echo ">> Userland: $IMG ($((BLOCKS * 512 / 1024)) KiB, $((ANZ - 1)) Eintraege), CRC32 0x${CRC}"
fi

# ------------------------------------------------- 3. die Kommandozeile
if [[ -z "$CMDLINE" ]]; then
    if [[ $MIT_USERLAND -eq 1 ]]; then
        CMDLINE="osum nokbd nosched noproc nofs noring3 modfs modcrc=${CRC}"
    else
        CMDLINE="osum nokbd nosched noproc nofs noring3"
    fi
elif [[ $MIT_USERLAND -eq 1 && "$CMDLINE" != *modcrc=* ]]; then
    # WICHTIG: vor `script=`, nicht dahinter. Osums `console_load` liest
    # alles hinter `script=` bis zum Ende der Zeile als Skript -- ein
    # `modfs` dahinter waere ein Shell-Befehl und kein Kernelwort. Genau
    # das ist einmal passiert und stand als `osum$ exit modfs modcrc=...`
    # im Protokoll.
    if [[ "$CMDLINE" == *script=* ]]; then
        CMDLINE="${CMDLINE%%script=*}modfs modcrc=${CRC} script=${CMDLINE#*script=}"
    else
        CMDLINE="$CMDLINE modfs modcrc=${CRC}"
    fi
fi

# ------------------------------------------------------- 4. das ISO
#
# Limine, Protokoll multiboot1. DAS IST DER UEFI-PFAD: Osums
# Multiboot-Kopf verlangt seit seinem Commit c4427fa einen linearen
# Rahmenpuffer (Flag-Bit 2). Ohne den bricht Limine unter UEFI mit
# "multiboot1: Cannot use text mode with UEFI" ab; mit ihm bootet
# dasselbe Abbild ueber SeaBIOS und ueber OVMF.
{
    echo "# Erzeugt von build.sh. Nicht von Hand aendern."
    echo "timeout: 0"
    echo "verbose: yes"
    echo
    echo "/${OS_NAME}"
    echo "    protocol: multiboot1"
    echo "    path: boot():/boot/${KERNEL_PKG}"
    echo "    cmdline: ${CMDLINE}"
    if [[ $MIT_USERLAND -eq 1 ]]; then
        echo "    module_path: boot():/boot/${SLUG}-userland.img"
        echo "    module_string: userland"
    fi
} > "$ROOT/boot/limine/limine.conf"

cp vendor/limine/limine-bios.sys \
   vendor/limine/limine-bios-cd.bin \
   vendor/limine/limine-uefi-cd.bin "$ROOT/boot/limine/"
cp vendor/limine/BOOTX64.EFI "$ROOT/EFI/BOOT/"

xorriso -as mkisofs -quiet -R -r -J \
    -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    -hfsplus -apm-block-size 2048 \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image \
    --protective-msdos-label \
    "$ROOT" -o "build/${SLUG}.iso"
if [[ ! -x vendor/limine/limine ]]; then
    make -s -C vendor/limine >/dev/null
fi
vendor/limine/limine bios-install "build/${SLUG}.iso" >/dev/null

echo ">> Kernel: Osum $(cut -c1-8 vendor/osum/COMMIT) (Firn), Kommandozeile: ${CMDLINE}"
echo ">> fertig: build/${SLUG}.iso ($(( $(stat -c%s "build/${SLUG}.iso") / 1024 )) KiB), Kernel $(( $(stat -c%s "$KERNEL") / 1024 )) KiB"
