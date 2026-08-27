#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only
# run-osum.sh — startet das Produkt-ISO.
#
# Es gibt nur noch dieses eine Startskript. Bis zum 26.08.2026 stand
# daneben `run-qemu.sh` fuer den alten Rust-Kernel; der ist geloescht
# (KERNELWECHSEL.md § 7).
#
#   * Osum meldet sich ueber die SERIELLE Schnittstelle und beendet sich
#     am Ende seiner Stufen selbst ueber `isa-debug-exit`: 21 = sauber
#     beendet, 63 = an einer Ausnahme stehengeblieben. Ein Zeitlimit, das
#     zuschlaegt, ist ein Fehler.
#   * Was er tut, steht auf seiner Kommandozeile (`kmain.fi`, `mode_of`).
#     Die wird beim BAUEN in limine.conf geschrieben, nicht beim Starten —
#     deshalb baut dieses Skript vorher.
#   * `-cpu max` ist KEINE Kosmetik: QEMUs Vorgabeprozessor (`qemu64`)
#     kennt SMEP und SMAP nicht, und dann meldet der Kernel ehrlich
#     `smep=0 smap=0`. Das Produkt soll auf einem Prozessor gemessen
#     werden, der die Schutzbits hat.
#
#   ./run-osum.sh                       Boot ueber SeaBIOS, Ausgabe zeigen
#   ./run-osum.sh --uefi                Boot ueber OVMF
#   ./run-osum.sh --cmdline "osum caps" mit anderer Kommandozeile
#   ./run-osum.sh --script "ls /bin"    ein Shell-Skript hineinreichen
#   ./run-osum.sh --ohne-userland       ISO ohne das Boot-Modul bauen
#   ./run-osum.sh --log DATEI           serielle Ausgabe dorthin schreiben
#   ./run-osum.sh --timeout 120
#
# Rueckgabe: 0, wenn der Kernel sich mit 21 beendet hat.
set -uo pipefail
cd "$(dirname "$0")"

UEFI=0
TIMEOUT=180
CMDLINE=""
SCRIPT=""
LOG=""
MEM=512M
CPU=max
BAUARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --uefi) UEFI=1; shift ;;
        --cmdline) CMDLINE="$2"; shift 2 ;;
        --script) SCRIPT="$2"; shift 2 ;;
        --ohne-userland) BAUARGS+=(--ohne-userland); shift ;;
        --kaputte-summe) BAUARGS+=(--kaputte-summe); shift ;;
        --brand) BAUARGS+=(--brand "$2"); export BRAND="$2"; shift 2 ;;
        --cpu) CPU="$2"; shift 2 ;;
        --log) LOG="$2"; shift 2 ;;
        --timeout) TIMEOUT="$2"; shift 2 ;;
        -h|--help) sed -n '2,28p' "$0"; exit 0 ;;
        *) echo "unbekannte Option: $1" >&2; exit 1 ;;
    esac
done

source ./brand.sh

# Ein Skript ist nur ein Zusatz zur Kommandozeile; `script=` liest Osums
# `console_load` und schiebt es durch dieselbe Zeilendisziplin wie eine
# Tastatur. Ein Semikolon ist dort ein Zeilenumbruch.
if [[ -n "$SCRIPT" ]]; then
    if [[ -z "$CMDLINE" ]]; then
        CMDLINE="osum nokbd nosched noproc nofs noring3 script=$SCRIPT"
    else
        CMDLINE="$CMDLINE script=$SCRIPT"
    fi
fi
[[ -n "$CMDLINE" ]] && BAUARGS+=(--cmdline "$CMDLINE")

# IMMER neu bauen: die Kommandozeile steht im ISO, nicht im Aufruf.
./build.sh "${BAUARGS[@]}" >/dev/null || {
    echo "Bauen fehlgeschlagen" >&2; exit 1; }

# Eigene Kopie je Lauf, damit zwei gleichzeitige Pruefungen sich nicht das
# Abbild unter den Fuessen wegbauen.
ISO="build/${SLUG}.$$.iso"
cp -f "build/${SLUG}.iso" "$ISO" || exit 1
if [[ -z "$LOG" ]]; then
    LOG="build/osum-boot.$$.log"
    AUFRAEUMEN=1
else
    AUFRAEUMEN=0
fi
: > "$LOG"
trap '[[ $AUFRAEUMEN -eq 1 ]] && rm -f "$LOG"; rm -f "$ISO"' EXIT

QEMU=(qemu-system-x86_64
      -machine q35
      -cpu "$CPU"
      -m "$MEM"
      -cdrom "$ISO"
      -boot d
      -no-reboot
      -display none
      -serial "file:$LOG"
      -device isa-debug-exit,iobase=0xf4,iosize=0x04)

WIE=BIOS
if [[ $UEFI -eq 1 ]]; then
    OVMF=$(ls /usr/share/OVMF/OVMF_CODE*.fd /usr/share/ovmf/OVMF.fd 2>/dev/null | head -1)
    if [[ -z "$OVMF" ]]; then
        echo "OVMF nicht gefunden — bitte Paket ovmf installieren." >&2
        exit 2
    fi
    QEMU+=(-bios "$OVMF")
    WIE=UEFI
fi

T0=$SECONDS
timeout "$TIMEOUT" "${QEMU[@]}" >/dev/null 2>&1
RC=$?
DAUER=$((SECONDS - T0))

echo "--- Osum $(cut -c1-8 vendor/osum/COMMIT), Start ueber $WIE, CPU $CPU, ${DAUER} s ---"
echo "--- Kommandozeile: $(grep -m1 'cmdline:' build/isoroot/boot/limine/limine.conf | sed 's/^ *cmdline: //')"
# Die Steuerzeichen der UEFI-Firmware herausnehmen, sonst ist das
# Protokoll unlesbar.
sed -e 's/\x1b\[[0-9;=]*[a-zA-Z]//g' "$LOG"
echo "--- Beendigungscode $RC ---"

case "$RC" in
    21) echo "  [ ok ] der Kernel hat sich selbst beendet (21)" ;;
    63) echo "  [FEHL] der Kernel ist an einer Ausnahme stehengeblieben (63)"; exit 1 ;;
    124) echo "  [FEHL] Zeitlimit ${TIMEOUT} s erreicht — der Kernel kam nie ans Ende"; exit 1 ;;
    *)  echo "  [FEHL] unerwarteter Beendigungscode $RC"; exit 1 ;;
esac
exit 0
