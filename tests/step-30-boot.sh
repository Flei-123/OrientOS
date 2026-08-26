# tests/step-30-boot.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# DAS PRODUKT STARTET WIRKLICH — UEBER BIOS UND UEBER UEFI.
#
# Der UEFI-Weg ist der Grund, warum Osums Multiboot-Kopf ueberhaupt
# geaendert werden musste (KERNELWECHSEL.md § 3.1): ohne Flag-Bit 2 nimmt
# ein Lader den Textmodus an, und unter UEFI gibt es keinen. Beide Wege
# werden hier wirklich gefahren, nicht behauptet.
#
# Dass es ZWEI Starts waren und nicht zweimal derselbe, haengt an einer
# Zahl, die keiner von beiden erfinden kann: die Speicherkarte kommt von
# der Firmware, und SeaBIOS und OVMF liefern verschieden viele Eintraege.
step "Boot: dasselbe Abbild ueber SeaBIOS und ueber OVMF"
boot_check() {
    RC=0
    local log muster
    log=$(mktemp)
    if ./run-osum.sh --log "$log" > build/boot-bios.log 2>&1; then
        ok "Boot ueber SeaBIOS: der Kernel beendet sich selbst (21)"
    else
        nok "Boot ueber SeaBIOS fehlgeschlagen"
        tail -15 build/boot-bios.log | sed 's/^/         /'
        rm -f "$log"; return 1
    fi
    for muster in \
        '^firn kernel r62, profile kernel' \
        '^mb: flags=' \
        '^mmap: [0-9]+ entries' \
        '^pci: devices=[1-9]' \
        '^smp: online=[1-9]' \
        '^guard: cr4=' \
        '^mod: base=' \
        '^kernel: done'
    do
        if grep -qaE "$muster" "$log"; then
            ok "BIOS: /$muster/"
        else
            nok "BIOS: /$muster/ fehlt"
        fi
    done
    # Die Speicherkarte kommt vom Lader. Sie MUSS Eintraege haben, sonst
    # haette der Kernel keinen Speicher gefunden und liefe auf Glueck.
    local eintraege
    eintraege=$(grep -aoE '^mmap: [0-9]+' "$log" | head -1 | grep -oE '[0-9]+')
    if [[ "${eintraege:-0}" -ge 5 ]]; then
        ok "BIOS: Speicherkarte mit $eintraege Eintraegen vom Lader"
    else
        nok "BIOS: Speicherkarte mit ${eintraege:-0} Eintraegen"
    fi

    if ls /usr/share/OVMF/OVMF_CODE*.fd >/dev/null 2>&1 \
       || ls /usr/share/ovmf/OVMF.fd >/dev/null 2>&1; then
        local ulog
        ulog=$(mktemp)
        if ./run-osum.sh --uefi --log "$ulog" > build/boot-uefi.log 2>&1; then
            ok "Boot ueber UEFI (OVMF): der Kernel beendet sich selbst (21)"
        else
            nok "Boot ueber UEFI fehlgeschlagen — genau das ging vor dem Kernelwechsel nicht"
            tail -15 build/boot-uefi.log | sed 's/^/         /'
        fi
        # Ohne Zeilenanfang: die UEFI-Firmware schreibt Steuerzeichen auf
        # dieselbe serielle Leitung, und die stehen dann vor der Zeile.
        for muster in 'firn kernel r62' 'mmap: [0-9]+ entries' 'mod: base=' 'kernel: done'; do
            grep -qaE "$muster" "$ulog" \
                && ok "UEFI: /$muster/" \
                || nok "UEFI: /$muster/ fehlt"
        done
        local ueintraege
        ueintraege=$(grep -aoE '^mmap: [0-9]+' "$ulog" | head -1 | grep -oE '[0-9]+')
        if [[ -n "${ueintraege:-}" && "${ueintraege:-0}" -ne "${eintraege:-0}" ]]; then
            ok "UEFI liefert eine andere Speicherkarte als BIOS ($ueintraege statt $eintraege) — es waren zwei echte Starts"
        else
            nok "BIOS und UEFI melden dieselbe Speicherkarte (${eintraege:-?}) — der zweite Start ist nicht belegt"
        fi
        # Und das Modul kommt ueber BEIDE Wege heil an.
        if grep -qa 'ok=1' "$ulog"; then
            ok "UEFI: die Pruefsumme des Boot-Moduls stimmt auch dort"
        else
            nok "UEFI: das Boot-Modul kam nicht heil an"
        fi
        rm -f "$ulog"
    else
        echo "  (OVMF nicht installiert — der UEFI-Start wird uebersprungen)"
    fi
    rm -f "$log"
    return $RC
}
run boot_check
