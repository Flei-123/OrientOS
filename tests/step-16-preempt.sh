# tests/step-16-preempt.sh — wird von test.sh gesourct, nicht direkt gestartet.
# Nachweis der Verdraengung: der Zeitgeber nimmt Zaehlschleifen die CPU weg,
# ohne dass diese je `yield` rufen; Prioritaeten verteilen die Ticks messbar;
# der Leerlauf ist ein eigener Thread; und der Zeitgebereinsprung rettet
# wirklich alle 15 Allzweckregister.
step "Verdraengung: Wechsel ohne freiwilliges yield, Prioritaeten wirken"
preempt_check() {
    ./run-qemu.sh --test-preempt || return 1
    local log=build/boot.log rc=0 muster
    # Jede Zusage einzeln pruefen — eine Sammelzahl allein waere zu leicht
    # gruen zu bekommen.
    for muster in \
        'Praeemption : aktiv, Zeitscheibe [0-9]+ Tick\(s\) je Prioritaetsstufe' \
        'praeemptive Wechsel: [1-9][0-9]*, kooperative Wechsel: [0-9]+' \
        'ohne yield: [1-9][0-9]* Wechsel zwischen [2-9] Threads' \
        'Thread 1: Prio [0-9]+, [1-9][0-9]* Ticks, [1-9][0-9]* Schleifendurchlaeufe' \
        'Thread 2: Prio [0-9]+, [1-9][0-9]* Ticks, [1-9][0-9]* Schleifendurchlaeufe' \
        'Thread 3: Prio [0-9]+, [1-9][0-9]* Ticks, [1-9][0-9]* Schleifendurchlaeufe' \
        'Prioritaet wirkt: Thread 1 \(Prio [0-9]+\) [0-9]+ Ticks > Thread 2 .* > Thread 3 ' \
        'Verdraengung: [1-9][0-9]* erzwungene Wechsel in [0-9]+ Ticks, ([0-9]+)/\1 Zusagen' \
        'Leerlauf-Thread: [1-9][0-9]* Einlastung\(en\), [1-9][0-9]* Tick\(s\) im Leerlauf, eigener Stapel [0-9]+ B, Wachzone unversehrt' \
        'Leerlauf unter Verdraengung: [1-9][0-9]* Einlastung\(en\), [1-9][0-9]* Tick\(s\) auf den Leerlauf-Thread gebucht' \
        'Schlafen/Wecken: .* ([0-9]+)/\1 Zusagen' \
        'Verdraengungssperre: [1-9][0-9]* Tick\(s\) im geschuetzten Abschnitt, 0 Wechsel darin, [1-9][0-9]* nachgeholt nach dem Ende' \
        'Zeitbudget von Ring 3 erschoepft \(RIP=0x[0-9a-f]{16}\) — Programm wird abgeraeumt' \
        'Startvorgang abgeschlossen' \
        ; do
        if grep -qE "$muster" "$log"; then
            echo "  [ ok ] $muster"
        else
            echo "  [FEHL] Muster fehlt im Boot-Log: $muster"
            rc=1
        fi
    done

    # Die Ticks muessen der Prioritaet folgen: hohe Stufe -> mehr Ticks.
    local t1 t2 t3
    t1=$(grep -oE 'Thread 1: Prio [0-9]+, [0-9]+ Ticks' "$log" | grep -oE '[0-9]+ Ticks' | grep -oE '[0-9]+')
    t2=$(grep -oE 'Thread 2: Prio [0-9]+, [0-9]+ Ticks' "$log" | grep -oE '[0-9]+ Ticks' | grep -oE '[0-9]+')
    t3=$(grep -oE 'Thread 3: Prio [0-9]+, [0-9]+ Ticks' "$log" | grep -oE '[0-9]+ Ticks' | grep -oE '[0-9]+')
    if [[ -n "$t1" && -n "$t2" && -n "$t3" ]] && (( t1 > t2 && t2 > t3 )); then
        echo "  [ ok ] Tickverteilung $t1 : $t2 : $t3 folgt der Prioritaet"
    else
        echo "  [FEHL] Tickverteilung folgt nicht der Prioritaet ($t1 : $t2 : $t3)"
        rc=1
    fi

    # Erzwungene Wechsel muessen deutlich ueberwiegen: die Zaehlschleifen
    # geben nur zum Abmelden freiwillig ab.
    local erzw koop
    erzw=$(grep -oE 'praeemptive Wechsel: [0-9]+' "$log" | grep -oE '[0-9]+$')
    koop=$(grep -oE 'kooperative Wechsel: [0-9]+' "$log" | grep -oE '[0-9]+$')
    if [[ -n "$erzw" && -n "$koop" ]] && (( erzw > koop )); then
        echo "  [ ok ] $erzw erzwungene gegen $koop freiwillige Wechsel"
    else
        echo "  [FEHL] erzwungene Wechsel ueberwiegen nicht ($erzw gegen $koop)"
        rc=1
    fi

    # Der Zeitgebereinsprung muss ALLE 15 Allzweckregister retten und in
    # umgekehrter Reihenfolge zurueckholen — sonst ist "voller Registersatz"
    # nur behauptet. Geprueft wird die Quelle, nicht die Absichtserklaerung.
    local einsprung=kernel/src/arch/x86_64/preempt.rs reg fehlt=""
    for reg in rax rcx rdx rbx rbp rsi rdi r8 r9 r10 r11 r12 r13 r14 r15; do
        grep -q "\"push $reg\"" "$einsprung" || fehlt="$fehlt push:$reg"
        grep -q "\"pop $reg\"" "$einsprung"  || fehlt="$fehlt pop:$reg"
    done
    if [[ -n "$fehlt" ]]; then
        echo "  [FEHL] Zeitgebereinsprung rettet nicht alle Register:$fehlt"
        rc=1
    else
        echo "  [ ok ] Zeitgebereinsprung rettet und holt alle 15 Allzweckregister"
    fi

    # Der Registerrettungscode gehoert nach arch/, die Auswahllogik nach kcore:
    # in kcore darf kein Registername und kein Assembler stehen.
    if grep -nE '(naked_asm|asm!|iretq|\br1[0-5]\b)' \
         kernel/src/kcore/preempt.rs kernel/src/kcore/sched.rs | grep -q .; then
        echo "  [FEHL] Registerdetails in kcore/preempt.rs oder kcore/sched.rs"
        rc=1
    else
        echo "  [ ok ] kcore kennt keine Register — Rettung nur in arch/"
    fi

    # Der verdraengende Zeitgeberpfad muss auch den Fall behandeln, dass der
    # Tick unprivilegierten Code trifft — sonst haelt eine Rechenschleife in
    # Ring 3 den Kernel an, sobald die Verdraengung eingeschaltet ist.
    if grep -q 'note_user_tick' "$einsprung" && grep -q 'abort_user' "$einsprung"; then
        echo "  [ ok ] Wachhund fuer Ring 3 haengt auch am verdraengenden Pfad"
    else
        echo "  [FEHL] verdraengender Zeitgeberpfad kennt den Wachhund fuer Ring 3 nicht"
        rc=1
    fi

    # Die Verdraengungssperre darf den Planer aus dem Interruptpfad heraus
    # wirklich heraushalten: kein Buchen, kein Wechsel, solange sie gilt.
    if grep -q 'if held()' kernel/src/kcore/preempt.rs; then
        echo "  [ ok ] Zeitgeberpfad prueft die Verdraengungssperre vor jeder Buchung"
    else
        echo "  [FEHL] Verdraengungssperre wird im Zeitgeberpfad nicht geprueft"
        rc=1
    fi

    # Es darf nur EINEN Planer geben: kein zweiter, paralleler Scheduler.
    local planer
    planer=$(grep -rlE 'impl +Scheduler +for ' kernel/src --include='*.rs' | wc -l)
    if [[ "$planer" -eq 1 ]]; then
        echo "  [ ok ] genau eine Umsetzung des Scheduler-Traits"
    else
        echo "  [FEHL] $planer Umsetzungen des Scheduler-Traits (soll: 1)"
        rc=1
    fi
    return $rc
}
run preempt_check
