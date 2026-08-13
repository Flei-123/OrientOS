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
#   ./run-qemu.sh --test-nx        Sprung in eine NX-Seite (muss #PF geben)
#   ./run-qemu.sh --test-gp        nicht kanonische Adresse (muss #GP geben)
#   ./run-qemu.sh --test-ud        ungueltige Instruktion (muss #UD geben)
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
        --test-nx) FEATURES=test-nx; MODE=check; shift ;;
        --test-gp) FEATURES=test-gp; MODE=check; shift ;;
        --test-ud) FEATURES=test-ud; MODE=check; shift ;;
        --no-posix) NOPOSIX=1; shift ;;
        --timeout) TIMEOUT="$2"; shift 2 ;;
        -h|--help) sed -n '2,13p' "$0"; exit 0 ;;
        *) echo "unbekannte Option: $1" >&2; exit 1 ;;
    esac
done

BUILD_ARGS=()
[[ -n "$FEATURES" ]] && BUILD_ARGS+=(--features "$FEATURES")
[[ $NOPOSIX -eq 1 ]] && BUILD_ARGS+=(--no-posix)
# IMMER neu bauen: sonst startet ein Lauf womoeglich das ISO des vorherigen
# Laufs (z. B. das --no-posix-Abbild) und prueft die falsche Konfiguration.
./build.sh "${BUILD_ARGS[@]}" || exit 1

# Eigene Kopie des Abbilds je Lauf. Laufen mehrere Pruefungen gleichzeitig
# (z. B. zwei test.sh nebeneinander), baut sonst der eine Lauf build/karstos.iso
# neu, waehrend der andere es gerade bootet — geprueft wuerde dann der falsche
# Kernel. Die Kopie wird am Ende wieder weggeraeumt.
# Beim interaktiven Lauf (exec, kein Aufraeumen moeglich) bleibt es beim
# gemeinsamen Abbild.
ISO="build/karstos.iso"
if [[ "$MODE" == check ]]; then
    ISO="build/karstos.$$.iso"
    cp -f build/karstos.iso "$ISO" || exit 1
    trap 'rm -f "$ISO"' EXIT
fi

QEMU=(qemu-system-x86_64
      -machine q35
      -m "$MEM"
      -cdrom "$ISO"
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
    mkdir -p build
    # Eigener Logname je Lauf (PID): laufen zwei Pruefungen gleichzeitig,
    # ueberschreiben sie sich sonst gegenseitig das Protokoll und werten das
    # Log des jeweils anderen Kernels aus. build/boot.log bleibt als Kopie
    # des zuletzt beendeten Laufs zum Nachlesen erhalten.
    LOG="build/boot.$$.log"
    : > "$LOG"
    trap 'cp -f "$LOG" build/boot.log 2>/dev/null; rm -f "$LOG" "$ISO"' EXIT
    # Abbruchmarke je Testart — dadurch endet der Lauf, sobald das Ergebnis
    # feststeht, statt stur bis zum Zeitlimit zu warten.
    case "$FEATURES" in
        test-doublefault) DONE='Kein Weiterlaufen moeglich' ;;
        test-pagefault|test-panic|test-rodata|test-nx|test-gp|test-ud) DONE='KERNEL PANIC' ;;
        *) DONE='Startvorgang abgeschlossen' ;;
    esac
    T0=$SECONDS
    "${QEMU[@]}" -display none -serial "file:$LOG" >/dev/null 2>&1 &
    QPID=$!
    HIT=0
    for _ in $(seq 1 $((TIMEOUT * 5))); do
        sleep 0.2
        if grep -qE "$DONE" "$LOG" 2>/dev/null; then HIT=1; break; fi
        kill -0 "$QPID" 2>/dev/null || break
    done
    DAUER=$((SECONDS - T0))
    kill "$QPID" 2>/dev/null
    wait "$QPID" 2>/dev/null
    echo "--- serielle Ausgabe (${LOG}) ---"
    cat "$LOG"
    echo "--- Auswertung ---"
    fail=0
    # Abbruchmarke erreicht? Ohne diese Pruefung meldet ein Kernel, der mitten
    # im Start haengenbleibt, nur "Muster fehlt"; die Ursache — er kam nie ans
    # Ziel und QEMU lief in die Zeitgrenze — bliebe im Dunkeln.
    if [[ $HIT -eq 1 ]]; then
        echo "  [ ok ] Abbruchmarke '$DONE' nach ${DAUER} s erreicht (Grenze ${TIMEOUT} s)"
    else
        echo "  [FEHL] Abbruchmarke '$DONE' nicht erreicht — ${DAUER} s, Grenze ${TIMEOUT} s"
        fail=1
    fi
    check() {  # check <Beschreibung> <Muster>: Muster MUSS vorkommen
        if grep -qE "$2" "$LOG"; then
            echo "  [ ok ] $1"
        else
            echo "  [FEHL] $1  (Muster: $2)"
            fail=1
        fi
    }
    checknot() {  # checknot <Beschreibung> <Muster>: Muster darf NICHT vorkommen
        if grep -qE "$2" "$LOG"; then
            echo "  [FEHL] $1  (verbotenes Muster gefunden: $2)"
            fail=1
        else
            echo "  [ ok ] $1"
        fi
    }
    order() {  # order <Beschreibung> <Muster1> <Muster2>: 1 muss VOR 2 stehen
        local a b
        a=$(grep -nE "$2" "$LOG" | head -1 | cut -d: -f1)
        b=$(grep -nE "$3" "$LOG" | head -1 | cut -d: -f1)
        if [[ -n "$a" && -n "$b" && "$a" -lt "$b" ]]; then
            echo "  [ ok ] $1 (Zeile $a vor Zeile $b)"
        else
            echo "  [FEHL] $1  (Reihenfolge '$2' vor '$3' nicht belegt: '$a' / '$b')"
            fail=1
        fi
    }
    check "Kernel meldet sich"            'karst v.* Kernel von Karstos'
    check "Bootloader nennt sich"         'Bootloader  : \S+ \S+'
    check "HHDM-Fenster bekannt"          'HHDM-Fenster: virt = phys \+ 0x[0-9a-f]+'
    check "Kernelabbild vermessen"        'gesamt [0-9]+ KiB geladenes Abbild \([0-9]+ Seiten'
    check "Rechte je Sektion gemeldet"    '\.rodata.*\(([0-9]+) KiB, R\)'
    check "#DF hat eigenen Notfallstapel" 'eigenem Notfallstapel \(0x[0-9a-f]+\)'
    check "CPU erkannt"                   'CPU         : \S'
    check "Framebuffer-Textkonsole steht" 'Textkonsole [0-9]+x[0-9]+'
    check "Stapel vom Bootloader"         'Stapel      : [0-9]+ KiB angefordert, gewaehrt'
    check "native ABI antwortet"          'karst-native: Version=[0-9]+'
    check "Memory-Map gelesen"            'Memory-Map \([0-9]+ Eintraege\)'
    check "Frame-Allocator laeuft"        'Frames      : [0-9]+ verwaltet'
    check "eigener Adressraum aktiv"      'Eigener Adressraum aktiv'
    check "Paging-Selbsttest ok"          'Selbsttest Paging:.*schreiben/lesen ok, translate ok, unmap ok'
    check "Heap eingerichtet"             'Kernel-Heap : [0-9]+ KiB'
    check "Heap-Allokation funktioniert"  'Testallokation: Vec mit 1024 Elementen'
    check "Breakpoint-Trap kehrt zurueck" 'Selbsttest: #BP ausgeloest und sauber fortgesetzt'
    check "Timer-Interrupts kommen an"    'Tick 5 —'
    check "MapError-Selbsttest laeuft"     'Selbsttest MapError: [0-9]+/[0-9]+ Fehlerfaelle nachweisbar ausgeloest'
    check "Frame-Selbsttest laeuft"        'Selbsttest Frames: [0-9]+/[0-9]+ Zusagen erfuellt'
    check "Heap-Selbsttest laeuft"         'Selbsttest Heap: [0-9]+/[0-9]+ Zusagen erfuellt'
    check "Heap nach Freigabe wieder leer" 'nach Freigabe: belegt 0 B von [0-9]+ B'
    check "Scheduler meldet seinen Stand"  '(kein Scheduler aktiv|Threadwechsel: [0-9]+ Wechsel, zurueck in kmain)'
    check "Startbilanz vorhanden"          'Startbilanz : [0-9]+ Frames frei'
    check "Startvorgang abgeschlossen"    'Startvorgang abgeschlossen'
    # Der Kernel nennt selbst, welcher Fehler-Selbsttest einkompiliert ist.
    # Damit kann ein Testlauf nicht versehentlich als normaler Boot durchgehen
    # (und umgekehrt): das Log muss zur Kommandozeile passen.
    if [[ -z "$FEATURES" ]]; then
        check "Konfiguration: normaler Boot" 'Fehler-Selbsttest keine \(normaler Boot\)'
    else
        check "Konfiguration nennt $FEATURES" "Fehler-Selbsttest $FEATURES"
    fi
    if [[ $NOPOSIX -eq 1 ]]; then
        check "Konfiguration: POSIX nein"  'POSIX nein'
    else
        check "Konfiguration: POSIX ja"    'POSIX ja'
    fi
    # Reihenfolge der Inbetriebnahme: erst eigener Adressraum, dann Heap, dann
    # Timer. Faellt der Kernel in eine falsche Reihenfolge zurueck, ist das ein
    # echter Fehler, auch wenn alle Einzelzeilen vorhanden sind.
    order "Adressraum vor Heap"   'Eigener Adressraum aktiv' 'Kernel-Heap : '
    order "Heap vor Timer"        'Kernel-Heap : '           'Zeitgeber laeuft mit'
    order "Timer vor Startbilanz" 'Zeitgeber laeuft mit'     'Startbilanz : '
    # Bilanzzeile: alle Selbsttests dieses Boots muessen bestanden sein, d. h.
    # die beiden Zahlen <bestanden>/<gesamt> muessen gleich sein.
    if grep -qE 'Selbsttestbilanz: [0-9]+/[0-9]+ bestanden' "$LOG"; then
        bilanz=$(grep -oE 'Selbsttestbilanz: [0-9]+/[0-9]+' "$LOG" | tail -1 | awk '{print $2}')
        if [[ "${bilanz%%/*}" == "${bilanz##*/}" ]]; then
            echo "  [ ok ] alle Selbsttests bestanden ($bilanz)"
        else
            echo "  [FEHL] Selbsttestbilanz unvollstaendig ($bilanz)"
            fail=1
        fi
    else
        echo "  [FEHL] keine Selbsttestbilanz im Log"
        fail=1
    fi
    checknot "kein Selbsttest meldet FEHLER" 'Selbsttest [A-Za-z]+:.*FEHLER'
    # Kein Modul darf irgendwo im Boot FEHLER oder WARNUNG melden. Die
    # absichtlich ausgeloesten Ausnahmen melden sich als CPU-AUSNAHME, nicht so.
    checknot "keine FEHLER-Meldung im Log"   'FEHLER'
    checknot "keine WARNUNG im Log"          'WARNUNG'
    # Nur beim normalen Boot: dort darf ueberhaupt keine CPU-Ausnahme und kein
    # Panic auftreten. Die --test-*-Laeufe loesen bewusst welche aus.
    if [[ -z "$FEATURES" ]]; then
        checknot "keine unerwartete CPU-Ausnahme" 'CPU-AUSNAHME'
        checknot "kein Panic beim normalen Boot"  'KERNEL PANIC'
    fi

    if [[ $NOPOSIX -eq 1 ]]; then
        check "POSIX wirklich draussen"    'posix-Schicht NICHT einkompiliert'
    else
        check "POSIX uebersetzt EBADF"     'read\(2\) auf unbekannten Fd -> -9'
    fi
    case "$FEATURES" in
        test-pagefault)   check "Page Fault wird gemeldet"  '#PF Page Fault' ;;
        test-doublefault)
            check "Double Fault wird gemeldet"  '#DF Double Fault'
            check "#DF laeuft auf IST-Stapel"   'IST-Stapel: 0x[0-9a-f]+ \(eigener Stack'
            check "kein Triple Fault (Kernel lebt)" 'Kein Weiterlaufen moeglich'
            ;;
        test-panic)
            check "Panic-Handler meldet sich"   'KERNEL PANIC'
            check "Panic nennt Ort im Quelltext" 'Ort *: .*\.rs:[0-9]+:[0-9]+'
            check "Panic nennt den Grund"        'Grund *: Selbsttest des Panic-Handlers'
            check "Backtrace hat Frames"         '#0 +ip=0xffffffff8[0-9a-f]+'
            check "Panic nennt die Laufzeit"     'Laufzeit: [0-9]+ Ticks bei [0-9]+ Hz'
            check "Panic nennt den Interruptzustand" 'Zustand : Interrupts beim Eintritt (an|aus), jetzt abgeschaltet'
            check "Backtrace zaehlt seine Frames" 'Backtrace: [1-9][0-9]* Frame\(s\) aufgeloest'
            checknot "kein Doppel-Panic"          'DOPPEL-PANIC'
            ;;
        test-rodata)
            check "Schreiben auf .rodata gibt #PF" '#PF Page Fault'
            check "Ursache: Schutzverletzung beim Schreiben" 'Ursache *: Schutzverletzung, Schreibzugriff'
            checknot "kein Schreibzugriff durchgekommen" 'Schreiben auf .rodata bei .* war erlaubt'
            ;;
        test-nx)
            check "Ausfuehren von .data gibt #PF" '#PF Page Fault'
            check "als Instruktionsabruf erkannt" 'Hinweis *: Zugriff war ein Instruktionsabruf'
            checknot "kein Code aus .data ausgefuehrt" 'Ausfuehren von .data bei .* war erlaubt'
            ;;
        test-gp)
            check "nicht kanonische Adresse gibt #GP" '#GP allgemeine Schutzverletzung'
            check "Fehlercode wird gemeldet"          'Fehlercode: 0x[0-9a-f]+'
            checknot "kein #PF statt #GP"             '#PF Page Fault'
            checknot "Zugriff nicht durchgekommen"    'nicht kanonische Adresse .* war erlaubt'
            ;;
        test-ud)
            check "ud2 gibt #UD"                  '#UD ungueltige Instruktion'
            check "RIP der Ausnahme wird genannt" 'RIP      : 0xffffffff8[0-9a-f]+'
            checknot "ud2 nicht uebergangen"      'ud2 hat keine Ausnahme ausgeloest'
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
