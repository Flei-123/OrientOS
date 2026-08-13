# tests/step-19-handles.sh — wird von test.sh gesourct, nicht direkt gestartet.
# Nachweis der capability-basierten ABI: Handles sind unfaelschbar, Rechte
# gelten je Handle, ein Prozess kommt ohne explizite Uebergabe an nichts.
step "Capabilities: Handle-Negativtests im Boot-Log"
handles_check() {
    ./run-qemu.sh --test-handles || return 1
    local log=build/boot.log rc=0 muster
    # Jede Zusage einzeln pruefen — eine Sammelzahl allein waere zu leicht
    # gruen zu bekommen.
    for muster in \
        'Handle-Negativtest: 3/3 abgewiesen' \
        'ungueltiger Index ok' \
        'veraltete Generation ok' \
        'fehlendes Recht ok' \
        'Capability-Negativtest: ([0-9]+)/\1 abgewiesen' \
        'fremdes Handle ok' \
        'ohne Uebergabe kein Zugriff ok' \
        'keine Rechteausweitung ok' \
        'Duplizieren ohne Recht ok' \
        'Uebergabe ohne Recht ok' \
        'unbekannte Aufrufnummer ok' \
        'Generation geraten \(0 Treffer bei 4096 Versuchen\)' \
        'Benutzung nach Schliessen ok' \
        'Handle nach Prozessende ok' \
        'Prozessprobe: spawn ohne fork' \
        'explizit uebergebenen Handle' \
        'Schreiben ohne Recht -> RightsDenied' \
        'Aufruf [a-z_]+ aus unprivilegiertem Prozess [0-9]+ ".*" abgewiesen: BadHandle' \
        'abgewiesen \(davon [1-9][0-9]* aus unprivilegierten Prozessen\)' \
        ; do
        if grep -qE "$muster" "$log"; then
            echo "  [ ok ] $muster"
        else
            echo "  [FEHL] Muster fehlt im Boot-Log: $muster"
            rc=1
        fi
    done
    # Kein fork, kein errno, kein globaler Pfad-Namensraum im Kern: die
    # native ABI darf solche Aufrufe gar nicht kennen.
    if grep -rqE '\bfn (sys_)?fork\b' kernel/src libs/karst-abi-native/src; then
        echo "  [FEHL] fork in Core oder nativer ABI gefunden"
        rc=1
    else
        echo "  [ ok ] kein fork in Core und nativer ABI"
    fi
    # Erklaerende Kommentare und die Verbotsliste im Test der ABI zaehlen nicht —
    # gesucht wird echter Code, der errno-Semantik einfuehrt.
    if grep -rniE '\berrno\b' kernel/src/kcore libs/karst-abi-native/src \
         | grep -vE ':[0-9]+:[[:space:]]*(//|///|//!|\*)' \
         | grep -vq '"errno"'; then
        echo "  [FEHL] errno-Semantik in kcore/ oder der nativen ABI"
        rc=1
    else
        echo "  [ ok ] kein errno in kcore/ und in der nativen ABI"
    fi
    return $rc
}
run handles_check
