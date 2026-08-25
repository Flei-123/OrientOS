# tests/step-24-osum-kernel.sh — wird von test.sh gesourct, nicht direkt
# gestartet.
#
# DER KERNELWECHSEL, gemessen. Ab hier ist der Kernel von OrientOS nicht
# mehr der Rust-Kernel dieses Repos, sondern Osum — ein eigenes Projekt in
# Firn, festgenagelt ueber vendor/osum/COMMIT. Dieser Schritt haelt drei
# Dinge fest, und alle drei sind Dinge, die stillschweigend kaputtgehen
# koennen:
#
#   1. DIE HERKUNFT IST NACHVOLLZIEHBAR. Im Repo liegt kein Kernelabbild,
#      sondern ein Commit-Hash. Das Abbild wird daraus gebaut, und der
#      Bauvorgang schreibt mit, aus welchem Commit es kam.
#   2. DAS PRODUKT-ISO ENTHAELT DIESEN KERNEL. Nicht den Rust-Kernel und
#      nicht ein Abbild von gestern.
#   3. ES BOOTET UEBER BIOS **UND** UEBER UEFI. Der UEFI-Weg ist der
#      Grund, warum Osums Multiboot-Kopf geaendert werden musste — ohne
#      Flag-Bit 2 bricht Limine dort mit "Cannot use text mode with UEFI"
#      ab. Beide Wege werden hier wirklich gefahren, nicht behauptet.
step "Osum ist die Kernelgrundlage: gebaut aus vendor/osum, BIOS und UEFI"
osum_kernel() {
    local rc=0

    # --- 1. Herkunft
    if [[ ! -f vendor/osum/COMMIT ]]; then
        echo "  [FEHL] vendor/osum/COMMIT fehlt — der Kernel haette keine Herkunft"
        return 1
    fi
    local commit
    commit=$(cat vendor/osum/COMMIT)
    if [[ ! "$commit" =~ ^[0-9a-f]{40}$ ]]; then
        echo "  [FEHL] vendor/osum/COMMIT ist kein Commit-Hash: $commit"
        rc=1
    else
        echo "  [ ok ] Kernel festgenagelt auf Osum ${commit:0:8}"
    fi
    # Gegenprobe: im Repo darf KEIN Kernelabbild eingecheckt sein. Faende
    # sich eines, waere nicht mehr gesagt, welcher Stand gemessen wurde.
    if git ls-files --error-unmatch vendor/osum/osum.mb >/dev/null 2>&1; then
        echo "  [FEHL] vendor/osum/osum.mb ist eingecheckt — es gehoert gebaut, nicht abgelegt"
        rc=1
    else
        echo "  [ ok ] kein Kernelabbild im Repo, nur der Commit und das Bauskript"
    fi

    ./vendor/osum/hole-osum.sh >/dev/null || {
        echo "  [FEHL] vendor/osum/hole-osum.sh ist fehlgeschlagen"; return 1; }
    if [[ "$(cat vendor/osum/.gebaut 2>/dev/null)" != "$commit" ]]; then
        echo "  [FEHL] das gebaute Abbild stammt nicht aus dem festgenagelten Commit"
        rc=1
    else
        echo "  [ ok ] das gebaute Abbild stammt aus ${commit:0:8}"
    fi

    # --- 2. Das Produkt-ISO enthaelt diesen Kernel
    ./build.sh --kernel osum >/dev/null || {
        echo "  [FEHL] ./build.sh --kernel osum ist fehlgeschlagen"; return 1; }
    local iso="build/${SLUG}.iso"
    if [[ ! -s "$iso" ]]; then
        echo "  [FEHL] $iso fehlt"
        return 1
    fi
    if ! cmp -s vendor/osum/osum.mb build/isoroot/boot/osum; then
        echo "  [FEHL] das Abbild im ISO ist nicht das gebaute Osum-Abbild"
        rc=1
    else
        echo "  [ ok ] $iso enthaelt genau vendor/osum/osum.mb ($(( $(stat -c%s vendor/osum/osum.mb) / 1024 )) KiB)"
    fi
    if grep -q '^ *protocol: multiboot1' build/isoroot/boot/limine/limine.conf; then
        echo "  [ ok ] Limine startet ihn ueber das Multiboot-Protokoll"
    else
        echo "  [FEHL] limine.conf nennt nicht das Multiboot-Protokoll"
        rc=1
    fi

    # --- 3. Der Kopf, der den UEFI-Start ueberhaupt erlaubt
    local video
    video=$(python3 - vendor/osum/osum.mb <<'PY'
import struct, sys
d = open(sys.argv[1], 'rb').read()
for off in range(0, min(len(d), 8192) - 48, 4):
    if d[off:off+4] == b'\x02\xb0\xad\x1b':
        magie, flags, pruef = struct.unpack_from('<3I', d, off)
        modus = struct.unpack_from('<I', d, off + 32)[0]
        print(flags, (magie + flags + pruef) & 0xFFFFFFFF, modus)
        break
else:
    print("0 1 9")
PY
)
    set -- $video
    if [[ "$2" == 0 && $(( $1 & 4 )) -ne 0 && "$3" == 0 ]]; then
        echo "  [ ok ] Multiboot-Kopf: Pruefsumme geht auf, Bit 2 gesetzt, linearer Rahmenpuffer"
    else
        echo "  [FEHL] Multiboot-Kopf: flags=$1 pruefsumme=$2 mode_type=$3"
        echo "         Ohne Bit 2 und mode_type 0 bricht Limine unter UEFI ab."
        rc=1
    fi

    # --- 4. Wirklich starten. Zweimal.
    local log
    log=$(mktemp)
    if ./run-osum.sh --log "$log" > build/osum-bios.log 2>&1; then
        echo "  [ ok ] Boot ueber SeaBIOS: der Kernel beendet sich selbst (21)"
    else
        echo "  [FEHL] Boot ueber SeaBIOS fehlgeschlagen"
        tail -15 build/osum-bios.log | sed 's/^/         /'
        rc=1
    fi
    local muster
    for muster in \
        'firn kernel r62, profile kernel' \
        '^mb: flags=' \
        '^mmap: [0-9]+ entries' \
        '^pci: devices=[1-9]' \
        '^smp: online=[1-9]' \
        '^kernel: done'
    do
        if grep -qaE "$muster" "$log"; then
            echo "  [ ok ] BIOS: /$muster/"
        else
            echo "  [FEHL] BIOS: /$muster/ fehlt"
            rc=1
        fi
    done
    # Die Speicherkarte kommt vom Lader. Sie MUSS Eintraege haben, sonst
    # haette der Kernel keinen Speicher gefunden und liefe auf Glueck.
    local eintraege
    eintraege=$(grep -aoE '^mmap: [0-9]+' "$log" | head -1 | grep -oE '[0-9]+')
    if [[ "${eintraege:-0}" -ge 5 ]]; then
        echo "  [ ok ] BIOS: Speicherkarte mit $eintraege Eintraegen vom Lader"
    else
        echo "  [FEHL] BIOS: Speicherkarte mit ${eintraege:-0} Eintraegen"
        rc=1
    fi

    if ls /usr/share/OVMF/OVMF_CODE*.fd >/dev/null 2>&1 \
       || ls /usr/share/ovmf/OVMF.fd >/dev/null 2>&1; then
        local ulog
        ulog=$(mktemp)
        if ./run-osum.sh --uefi --log "$ulog" > build/osum-uefi.log 2>&1; then
            echo "  [ ok ] Boot ueber UEFI (OVMF): der Kernel beendet sich selbst (21)"
        else
            echo "  [FEHL] Boot ueber UEFI fehlgeschlagen — genau das ging vor dem Kernelwechsel nicht"
            tail -15 build/osum-uefi.log | sed 's/^/         /'
            rc=1
        fi
        for muster in 'firn kernel r62' '^mmap: [0-9]+ entries' '^kernel: done'; do
            grep -qaE "$muster" "$ulog" \
                && echo "  [ ok ] UEFI: /$muster/" \
                || { echo "  [FEHL] UEFI: /$muster/ fehlt"; rc=1; }
        done
        # Die Speicherkarte einer UEFI-Firmware ist eine ANDERE als die
        # von SeaBIOS. Sind beide gleich, wurde in Wahrheit zweimal
        # dasselbe gestartet.
        local ueintraege
        ueintraege=$(grep -aoE '^mmap: [0-9]+' "$ulog" | head -1 | grep -oE '[0-9]+')
        if [[ -n "${ueintraege:-}" && "${ueintraege:-0}" -ne "${eintraege:-0}" ]]; then
            echo "  [ ok ] UEFI liefert eine andere Speicherkarte als BIOS ($ueintraege statt $eintraege) — es waren zwei echte Starts"
        else
            echo "  [FEHL] BIOS und UEFI melden dieselbe Speicherkarte (${eintraege:-?}) — der zweite Start ist nicht belegt"
            rc=1
        fi
        rm -f "$ulog"
    else
        echo "  (OVMF nicht installiert — der UEFI-Start wird uebersprungen)"
    fi
    rm -f "$log"
    return $rc
}
run osum_kernel
