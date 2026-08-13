# tests/step-21-doku-verweise.sh — wird von test.sh gesourct, nicht direkt
# gestartet.
#
# Doku, die auf Zeilennummern verweist, verrottet leise: der Code verschiebt
# sich, der Verweis bleibt stehen und zeigt irgendwann auf etwas voellig
# anderes. Genau das wurde in Runde 2 zu Recht bemaengelt (LANGUAGE.md nannte
# preempt.rs:158 fuer etwas, das in Zeile 217 stand).
#
# Deshalb gilt ein Format mit Anker — `datei.rs:ZEILE` (`Anker`) — und
# tests/verweise.py prueft jeden einzelnen Verweis gegen den echten Quelltext.
step "Doku-Verweise zeigen wirklich auf den genannten Code"
doku_verweise() {
    command -v python3 >/dev/null || { echo "  python3 fehlt"; return 1; }
    python3 tests/verweise.py || return 1
    # Gegenprobe: die Pruefung muss auch WIRKLICH anschlagen. Dafuer wird eine
    # Kopie der Doku mit einer absichtlich falschen Zeilennummer geprueft.
    local tmp
    tmp=$(mktemp -d) || return 1
    mkdir -p "$tmp/tests"
    cp tests/verweise.py "$tmp/tests/"
    cp -r kernel "$tmp/" 2>/dev/null
    printf '%s\n' 'Probe: `kernel/src/main.rs:99999` (`#![no_std]`)' > "$tmp/PROBE.md"
    if (cd "$tmp" && python3 tests/verweise.py >/dev/null 2>&1); then
        echo "  [FEHL] die Verweispruefung schlaegt bei einem falschen Verweis NICHT an"
        rm -rf "$tmp"
        return 1
    fi
    echo "  [ ok ] Gegenprobe: ein falscher Verweis laesst die Pruefung fallen"
    rm -rf "$tmp"
    # LANGUAGE.md muss mit jeder Runde wachsen, nicht nur bestehen.
    local eintraege
    eintraege=$(grep -cE '^## L-[0-9]+' LANGUAGE.md)
    if [[ "$eintraege" -lt 15 ]]; then
        echo "  [FEHL] LANGUAGE.md hat nur $eintraege Eintraege — die Runde hat nichts protokolliert"
        return 1
    fi
    echo "  LANGUAGE.md: $eintraege protokollierte Reibungspunkte mit Rust"
    return 0
}
run doku_verweise
