#!/usr/bin/env bash
# karst in QEMU starten.
#
#   ./run-qemu.sh                  normaler Boot, serielle Ausgabe im Terminal
#   ./run-qemu.sh --check          Boot pruefen und beenden (fuer CI, Exitcode!)
#   ./run-qemu.sh --uefi           ueber OVMF statt SeaBIOS booten
#   ./run-qemu.sh --test-pagefault
#   ./run-qemu.sh --test-doublefault
#   ./run-qemu.sh --test-panic
#   ./run-qemu.sh --test-rodata    Schreibversuch auf .rodata (muss #PF geben)
set -uo pipefail
cd "$(dirname "$0")"

MODE=run
FEATURES=""
UEFI=0
NOPOSIX=0
TIMEOUT=25
MEM=512M

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) MODE=check; shift ;;
        --uefi) UEFI=1; shift ;;
        --test-pagefault) FEATURES=test-pagefault; MODE=check; shift ;;
        --test-doublefault) FEATURES=test-doublefault; MODE=check; shift ;;
        --test-panic) FEATURES=test-panic; MODE=check; shift ;;
        --test-rodata) FEATURES=test-rodata; MODE=check; shift ;;
        --no-posix) NOPOSIX=1; shift ;;
        --timeout) TIMEOUT="$2"; shift 2 ;;
        -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
        *) echo "unbekannte Option: $1" >&2; exit 1 ;;
    esac
done

BUILD_ARGS=()
[[ -n "$FEATURES" ]] && BUILD_ARGS+=(--features "$FEATURES")
[[ $NOPOSIX -eq 1 ]] && BUILD_ARGS+=(--no-posix)
# IMMER neu bauen: sonst startet ein Lauf womoeglich das ISO des vorherigen
# Laufs (z. B. das --no-posix-Abbild) und prueft die falsche Konfiguration.
./build.sh "${BUILD_ARGS[@]}" || exit 1

QEMU=(qemu-system-x86_64
      -machine q35
      -m "$MEM"
      -cdrom build/karstos.iso
      -boot d
      -no-reboot
      -no-shutdown
      -device isa-debug-exit,iobase=0xf4,iosize=0x04)

if [[ $UEFI -eq 1 ]]; then
    OVMF=$(ls /usr/share/OVMF/OVMF_CODE*.fd /usr/share/ovmf/OVMF.fd 2>/dev/null | head -1)
    if [[ -z "$OVMF" ]]; then
        echo "OVMF nicht gefunden — bitte Paket ovmf installieren." >&2
        exit 1
    fi
    QEMU+=(-bios "$OVMF")
fi

if [[ "$MODE" == check ]]; then
    LOG=build/boot.log
    mkdir -p build
    : > "$LOG"
    # Abbruchmarke je Testart — dadurch endet der Lauf, sobald das Ergebnis
    # feststeht, statt stur bis zum Zeitlimit zu warten.
    case "$FEATURES" in
        test-doublefault) DONE='Kein Weiterlaufen moeglich' ;;
        test-pagefault|test-panic|test-rodata) DONE='KERNEL PANIC' ;;
        *) DONE='Startvorgang abgeschlossen' ;;
    esac
    "${QEMU[@]}" -display none -serial "file:$LOG" >/dev/null 2>&1 &
    QPID=$!
    for _ in $(seq 1 $((TIMEOUT * 5))); do
        sleep 0.2
        grep -qE "$DONE" "$LOG" 2>/dev/null && break
        kill -0 "$QPID" 2>/dev/null || break
    done
    kill "$QPID" 2>/dev/null
    wait "$QPID" 2>/dev/null
    echo "--- serielle Ausgabe (${LOG}) ---"
    cat "$LOG"
    echo "--- Auswertung ---"
    fail=0
    check() {  # check <Beschreibung> <Muster>
        if grep -qE "$2" "$LOG"; then
            echo "  [ ok ] $1"
        else
            echo "  [FEHL] $1  (Muster: $2)"
            fail=1
        fi
    }
    check "Kernel meldet sich"            'karst v.* Kernel von Karstos'
    check "Memory-Map gelesen"            'Memory-Map \([0-9]+ Eintraege\)'
    check "Frame-Allocator laeuft"        'Frames      : [0-9]+ verwaltet'
    check "eigener Adressraum aktiv"      'Eigener Adressraum aktiv'
    check "Paging-Selbsttest ok"          'Selbsttest Paging:.*schreiben/lesen ok, translate ok, unmap ok'
    check "Heap eingerichtet"             'Kernel-Heap : [0-9]+ KiB'
    check "Heap-Allokation funktioniert"  'Testallokation: Vec mit 1024 Elementen'
    check "Breakpoint-Trap kehrt zurueck" 'Selbsttest: #BP ausgeloest und sauber fortgesetzt'
    check "Timer-Interrupts kommen an"    'Tick 5 —'
    check "Startvorgang abgeschlossen"    'Startvorgang abgeschlossen'
    if [[ $NOPOSIX -eq 1 ]]; then
        check "POSIX wirklich draussen"    'posix-Schicht NICHT einkompiliert'
    else
        check "POSIX uebersetzt EBADF"     'read\(2\) auf unbekannten Fd -> -9'
    fi
    case "$FEATURES" in
        test-pagefault)   check "Page Fault wird gemeldet"  '#PF Page Fault' ;;
        test-doublefault) check "Double Fault wird gemeldet" '#DF Double Fault' ;;
        test-panic)       check "Panic-Handler meldet sich"  'KERNEL PANIC' ;;
        test-rodata)
            check "Schreiben auf .rodata gibt #PF" '#PF Page Fault'
            check "Ursache: Schutzverletzung beim Schreiben" 'Ursache *: Schutzverletzung, Schreibzugriff'
            ;;
    esac
    if [[ $fail -eq 0 ]]; then
        echo "ERGEBNIS: alle Pruefungen bestanden."
    else
        echo "ERGEBNIS: FEHLGESCHLAGEN."
    fi
    exit $fail
else
    exec "${QEMU[@]}" -display none -serial stdio
fi
