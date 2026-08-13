# tests/step-17-ring3.sh — wird von test.sh gesourct, nicht direkt gestartet.
# Nachweis der unprivilegierten Ebene: ein Programm laeuft wirklich in Ring 3
# (Selektor mit RPL 3 und CPL 3, an einem echten Ausnahmerahmen gemessen),
# gibt ueber ein explizit uebergebenes Handle etwas aus, beendet sich per
# Systemaufruf — und jeder Uebergriff aus Ring 3 wird sauber abgewiesen, ohne
# den Kernel anzuhalten.
step "Ring 3: unprivilegiertes Programm, Systemaufruf, Schutzwall"
ring3_check() {
    ./run-qemu.sh --test-ring3 || return 1
    local log=build/boot.log rc=0 muster
    # Jede Zusage einzeln pruefen — eine Sammelzahl allein waere zu leicht
    # gruen zu bekommen.
    for muster in \
        'CPU-Schutz  : Schnellaufruf ja,.*Per-CPU-Basis ja' \
        'Systemaufrufpfad: eingerichtet' \
        'Abbildung   : Programm 0x[0-9a-f]+ \([0-9]+ B, RX\), Stapel 0x0000[0-9a-f]+\.\.0x0000[0-9a-f]+ \(RW, NX\)' \
        'Prozess     : pid [0-9]+ unprivilegiert, 1 Handle uebergeben' \
        '#BP aus Ring 3: CS=0x[0-9a-f]+ \(CPL=3\), RSP=0x0000[0-9a-f]+' \
        'Ring 3      : CS=0x[0-9a-f]+ \(RPL=3\), CPL=3, Stapel=0x0000[0-9a-f]+, [1-9][0-9]* Systemaufruf\(e\), Ende 0' \
        'Zeiger 0x[0-9a-f]+ \(\+[0-9]+ B\) aus Ring 3 abgewiesen: nicht im eigenen Bereich' \
        'Ring-3-Zugriff auf Kerneladresse 0x[0-9a-f]+: sauber abgewiesen \(Schutzverletzung, Userspace\)' \
        '#PF aus Ring 3: CS=0x[0-9a-f]+ \(CPL=3\), RSP=0x0000[0-9a-f]+, RIP=0x[0-9a-f]+, Lesezugriff, Schutzverletzung' \
        'Negativtest : Programm nach [0-9]+ Systemaufruf\(en\) abgeraeumt, Kernel laeuft weiter' \
        'Schutzwall  : privilegierte Instruktion -> abgewiesen' \
        'Schutzwall  : Schreiben auf die eigene Codeseite -> abgewiesen' \
        'Schutzwall  : Sprung in den Kernelbereich -> abgewiesen' \
        'Schutzwall  : Zeiger auf eine nicht abgebildete Seite -> abgewiesen' \
        'Zeitbudget von Ring 3 erschoepft \(RIP=0x[0-9a-f]+\) — Programm wird abgeraeumt' \
        'Schutzwall  : Rechenschleife ohne Systemaufruf -> vom Zeitgeber unterbrochen und abgeraeumt \(ok\)' \
        'Schutzwall  : ([0-9]+)/\1 Uebergriffe aus Ring 3 abgewehrt' \
        'Aus dem Archiv: Einsprung 0x[0-9a-f]+, CS=0x[0-9a-f]+ \(CPL=3\), Stapel=0x0000[0-9a-f]+' \
        'Abmeldung   : pid [0-9]+ beendet mit Code 0, danach 0 Handle\(s\) in seiner Tafel' \
        'Abgeraeumt  : [0-9]+ Seiten zurueckgegeben, [0-9]+ B aus Ring 3 ausgegeben, [1-9][0-9]* Zugriff\(e\) abgewiesen' \
        'Startvorgang abgeschlossen' \
        ; do
        if grep -qE "$muster" "$log"; then
            echo "  [ ok ] $muster"
        else
            echo "  [FEHL] Muster fehlt im Boot-Log: $muster"
            rc=1
        fi
    done
    # Kein Uebergriff darf durchgekommen sein.
    if grep -qE 'Schutzwall  : .* wurde NICHT abgewiesen' "$log"; then
        echo "  [FEHL] ein Uebergriff aus Ring 3 wurde nicht abgewiesen"
        rc=1
    else
        echo "  [ ok ] kein Uebergriff aus Ring 3 kam durch"
    fi
    # Die Schutzbits der CPU (Ausfuehr-/Zugriffssperre) meldet das
    # voreingestellte QEMU-Rechnermodell NICHT; der Kernel ueberspringt sie
    # dann und vermerkt das. Damit dieser Pfad nicht nur behauptet ist, wird
    # dasselbe Abbild ein zweites Mal mit einem Rechnermodell gestartet, das
    # die Bits meldet — dort muessen sie tatsaechlich eingeschaltet sein UND
    # der ganze Startvorgang muss weiterhin sauber durchlaufen (das offene
    # Zugriffsfenster sitzt also an den richtigen Stellen).
    local smaplog="build/ring3-smap.$$.log"
    : > "$smaplog"
    timeout 180 qemu-system-x86_64 -machine q35 -cpu max -m 512M \
        -cdrom build/karstos.iso -boot d -no-reboot -display none \
        -serial "file:$smaplog" >/dev/null 2>&1
    for muster in \
        'CPU-Schutz  : Schnellaufruf ja, Ausfuehrsperre ja, Zugriffssperre ja, Per-CPU-Basis ja' \
        'Ring 3      : CS=0x[0-9a-f]+ \(RPL=3\), CPL=3, .*, Ende 0' \
        'Schutzwall  : ([0-9]+)/\1 Uebergriffe aus Ring 3 abgewehrt' \
        'Selbsttestbilanz: ([0-9]+)/\1 bestanden' \
        'Startvorgang abgeschlossen' \
        ; do
        if grep -qE "$muster" "$smaplog"; then
            echo "  [ ok ] mit Schutzbits: $muster"
        else
            echo "  [FEHL] mit Schutzbits fehlt: $muster"
            rc=1
        fi
    done
    if grep -qE 'FEHLER|WARNUNG' "$smaplog"; then
        echo "  [FEHL] Lauf mit eingeschalteten Schutzbits meldet FEHLER/WARNUNG"
        rc=1
    else
        echo "  [ ok ] Lauf mit eingeschalteten Schutzbits bleibt fehlerfrei"
    fi
    rm -f "$smaplog"
    # Der Privilegwechsel selbst darf nur in arch/ stehen: kein syscall-MSR,
    # kein swapgs, kein iretq ausserhalb von kernel/src/arch/.
    if grep -rnE '\b(swapgs|iretq|sysret|LSTAR|STAR|SFMASK)\b' kernel/src --include='*.rs' \
         | grep -v '^kernel/src/arch/' | grep -vE ':[[:space:]]*(//|///|//!|\*)' | grep -q .; then
        echo "  [FEHL] Privilegwechsel-Details ausserhalb kernel/src/arch/"
        rc=1
    else
        echo "  [ ok ] Privilegwechsel steht ausschliesslich in kernel/src/arch/"
    fi
    return $rc
}
run ring3_check
