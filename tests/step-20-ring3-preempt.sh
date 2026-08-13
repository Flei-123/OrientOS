# tests/step-20-ring3-preempt.sh — wird von test.sh gesourct, nicht direkt
# gestartet.
#
# Nachweis der SCHWIERIGEN Haelfte der Verdraengung: nicht nur privilegierter
# Kernelcode, sondern auch ein Programm in Ring 3 wird vom Zeitgeber
# unterbrochen und DANACH FORTGESETZT. Der Unterschied ist wesentlich —
# ein Programm abzuraeumen, weil es zu lange rechnet (Wachhund, Schritt 17),
# ist keine Verdraengung.
#
# Aufbau des Nachweises im Kernel (kcore/user.rs, arch/x86_64/preempt.rs):
# zwei gleichrangige, verdraengbare Threads. Einer startet ein Rechenprogramm
# in Ring 3, das 30 Mio. Schleifendurchlaeufe lang KEINEN Systemaufruf
# absetzt; der andere zaehlt. Keiner der beiden gibt freiwillig ab. Kommt das
# Programm trotzdem mit Ende 0 zurueck, waehrend der Zaehlthread vorangekommen
# ist, kann das nur am Zeitgeber liegen.
step "Ring 3 wird verdraengt und FORTGESETZT (nicht abgeraeumt)"
ring3_preempt_check() {
    ./run-qemu.sh --check || return 1
    local log=build/boot.log rc=0 muster
    for muster in \
        'Ring 3 verdraengt: [1-9][0-9]* Unterbrechung\(en\) durch den Zeitgeber, danach fortgesetzt, CS=0x[0-9a-f]+ \(RPL=3\), Ende 0' \
        'Ring 3 verdraengt: Zaehlthread kam auf [1-9][0-9]* Durchlaeufe, [1-9][0-9]* erzwungene und [0-9]+ freiwillige Wechsel, Ticks [1-9][0-9]*/[1-9][0-9]*, ([0-9]+)/\1 Zusagen' \
        ; do
        if grep -qE "$muster" "$log"; then
            echo "  [ ok ] $muster"
        else
            echo "  [FEHL] Muster fehlt im Boot-Log: $muster"
            rc=1
        fi
    done
    # Abgrenzung zum Wachhund: im Boot gibt es GENAU EINEN Abbruch wegen
    # Zeitbudget — den der Wachhund-Probe des Schutzwalls (Schritt 17). Die
    # Verdraengungsprobe darf keinen zweiten ausloesen; sie wird unterbrochen,
    # nicht abgeraeumt, und meldet deshalb "Ende 0".
    local abbrueche
    abbrueche=$(grep -c 'Zeitbudget von Ring 3 erschoepft' "$log" || true)
    if [[ "$abbrueche" -ne 1 ]]; then
        echo "  [FEHL] $abbrueche Wachhund-Abbrueche im Boot (erwartet genau 1, der des Schutzwalls)"
        rc=1
    else
        echo "  [ ok ] genau 1 Wachhund-Abbruch (Schutzwall), die Verdraengungsprobe lief durch"
    fi
    # Und die Reihenfolge stimmt: erst der Wachhund-Nachweis, danach der
    # Fortsetzungs-Nachweis mit Ende 0 — zwei verschiedene Programme, zwei
    # verschiedene Ausgaenge.
    local z_wach z_vor
    z_wach=$(grep -n 'Rechenschleife ohne Systemaufruf' "$log" | head -1 | cut -d: -f1)
    z_vor=$(grep -n 'Ring 3 verdraengt:' "$log" | head -1 | cut -d: -f1)
    if [[ -n "$z_wach" && -n "$z_vor" && "$z_vor" -gt "$z_wach" ]]; then
        echo "  [ ok ] Wachhund (Zeile $z_wach) und Fortsetzung (Zeile $z_vor) sind getrennte Nachweise"
    else
        echo "  [FEHL] Wachhund- und Fortsetzungsnachweis nicht beide vorhanden"
        rc=1
    fi
    # Gegenprobe im Quelltext: die beiden Threads duerfen kein `yield` rufen,
    # sonst waere der Wechsel freiwillig und der Nachweis wertlos.
    if awk '/fn r3_program_thread|fn r3_counter_thread/,/^}/' kernel/src/kcore/user.rs \
         | grep -q 'yield_now'; then
        echo "  [FEHL] ein Nachweis-Thread gibt freiwillig ab (yield_now)"
        rc=1
    else
        echo "  [ ok ] kein yield in den beiden Nachweis-Threads"
    fi
    # Der Wechsel aus einem Ring-3-Rahmen heraus gehoert in die Architektur.
    if ! grep -q 'USER_PREEMPTIONS' kernel/src/arch/x86_64/preempt.rs; then
        echo "  [FEHL] arch/x86_64/preempt.rs zaehlt keine Ring-3-Verdraengungen"
        rc=1
    else
        echo "  [ ok ] Ring-3-Verdraengung sitzt in arch/x86_64/preempt.rs"
    fi
    # Und der Kernelstapel je Thread, ohne den der Rahmen einen Wechsel nicht
    # ueberlebt, ebenso.
    if ! grep -q 'TRAP_GAP' kernel/src/arch/x86_64/user.rs; then
        echo "  [FEHL] kein eigener Rahmenplatz je Thread (TRAP_GAP fehlt)"
        rc=1
    else
        echo "  [ ok ] Rahmen des Privilegwechsels liegt auf dem Thread-Kernelstapel"
    fi
    return $rc
}
run ring3_preempt_check
