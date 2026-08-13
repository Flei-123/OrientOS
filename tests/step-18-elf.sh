# tests/step-18-elf.sh — wird von test.sh gesourct, nicht direkt gestartet.
# Nachweis des Weges "Bootloader-Modul -> Archiv -> ELF64 -> abgebildete
# Seiten": ein echtes, statisch gebundenes Programm aus dem Startdateisystem
# wird geladen, und jede Sorte Muell wird mit einem definierten Fehler
# abgewiesen, ohne den Kernel anzuhalten.
step "ELF-Lader: Programm aus dem Startdateisystem, Muell wird abgewiesen"
elf_check() {
    ./run-qemu.sh --test-elf || return 1
    local log=build/boot.log rc=0 muster
    for muster in \
        'Initramfs   : [1-9][0-9]* Eintraege, [0-9]+ B' \
        'Eintrag [0-9]+: hello ' \
        'Eintrag [0-9]+: kaputt.elf ' \
        'Archiv-Negativtest: ([0-9]+)/\1 Faelle wie erwartet abgewiesen' \
        'ELF-Negativtest: ([0-9]+)/\1 Faelle wie erwartet abgewiesen' \
        'ELF-Lader   : hello, [0-9]+ Segmente geladen, Einsprung 0x[0-9a-f]+' \
        'Segment 0: 0x[0-9a-f]+ [0-9]+ KiB RX ' \
        'Segment 1: 0x[0-9a-f]+ [0-9]+ KiB RW ' \
        'Stapel: 0x00007[0-9a-f]+ abwaerts, [0-9]+ Seiten' \
        'nachgeprueft: Einsprungseite und Stapelseite abgebildet' \
        'kaputt.elf aus dem Archiv +-> Segment ausserhalb des unprivilegierten Bereichs \(ok\)' \
        'liesmich.txt als Programm +-> falsche Kennung am Dateianfang \(ok\)' \
        'Abbild zu gross +-> Abbild belegt zu viele Seiten, [0-9]+ Datenseiten zurueckgegeben' \
        'ELF-Ladetest: ([0-9]+)/\1 Faelle wie erwartet' \
        ; do
        if grep -qE "$muster" "$log"; then
            echo "  [ ok ] $muster"
        else
            echo "  [FEHL] Muster fehlt im Boot-Log: $muster"
            rc=1
        fi
    done
    # Das Programm muss ein echtes ELF aus dem Archiv sein, kein im Kernel
    # eingebettetes Byte-Array: die Datei muss im gebauten Abbild liegen und
    # als ELF64 EXEC mit zwei PT_LOAD-Segmenten dastehen.
    if [[ ! -s build/userland/hello ]]; then
        echo "  [FEHL] build/userland/hello fehlt — kein echtes Programm gebaut"
        rc=1
    elif ! readelf -hl build/userland/hello 2>/dev/null \
            | grep -q 'Type: *EXEC'; then
        echo "  [FEHL] build/userland/hello ist kein statisches ELF64-EXEC"
        rc=1
    else
        local loads
        loads=$(readelf -l build/userland/hello | grep -c '^  LOAD')
        echo "  [ ok ] echtes ELF64-EXEC im Archiv, $loads PT_LOAD-Segmente"
        if [[ "$loads" -lt 2 ]]; then
            echo "  [FEHL] weniger als zwei PT_LOAD-Segmente — Rechte nicht getrennt"
            rc=1
        fi
    fi
    # Das Archiv muss wirklich im ISO-Abbild landen, sonst findet der Kernel
    # beim naechsten Lauf nichts.
    if [[ -s build/isoroot/boot/initramfs.img ]]; then
        echo "  [ ok ] Startdateisystem im Abbild ($(stat -c%s build/isoroot/boot/initramfs.img) B)"
    else
        echo "  [FEHL] build/isoroot/boot/initramfs.img fehlt"
        rc=1
    fi
    return $rc
}
run elf_check
