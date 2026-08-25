# tests/step-25-osum-caps.sh — wird von test.sh gesourct, nicht direkt
# gestartet.
#
# DIE CAPABILITY-SCHICHT HAT DEN KERNELWECHSEL UEBERLEBT.
#
# Sie war die inhaltlich wertvollste Eigenschaft des Rust-Kernels: kein
# globaler Namensraum, keine geerbten Deskriptoren, jeder Zugriff ueber ein
# Handle mit Rechten. Beim Wechsel auf Osum waere sie verloren gegangen —
# Osums POSIX-Schicht arbeitet mit Umgebungsautoritaet. Der Kern des
# Handle-Modells ist deshalb nach Firn portiert worden (Osum,
# `kernel/cap.fi`, `kernel/sys.fi`, `kernel/uprog.fi`).
#
# Was hier gemessen wird, ist NICHT dasselbe wie Schritt 19: Schritt 19
# prueft die Handles des Rust-Kernels. Dieser Schritt startet das
# PRODUKT-ISO mit dem Osum-Kernel und liest, was ein unprivilegiertes
# Programm in RING 3 ueber seine eigenen Rechte meldet.
#
# Der entscheidende Unterschied zu POSIX steht in einer einzigen Zusage:
# `read-denied` — ein GUELTIGES Handle ohne das noetige Recht ist
# RightsDenied, ein GEFAELSCHTES ist BadHandle. POSIX hat fuer beides nur
# -EBADF.
step "Capabilities im Osum-Kernel: 18 Zusagen aus Ring 3"
osum_caps() {
    local rc=0 log
    log=$(mktemp)
    if ! ./run-osum.sh --cmdline "osum nokbd nosched nofs noring3 caps" \
            --log "$log" > build/osum-caps.log 2>&1; then
        echo "  [FEHL] der Osum-Kernel ist mit der Capability-Runde nicht sauber beendet"
        tail -20 build/osum-caps.log | sed 's/^/         /'
        rm -f "$log"
        return 1
    fi
    echo "  [ ok ] Boot mit der Capability-Runde: Beendigungscode 21"

    # Jede Zusage EINZELN. Eine Sammelzahl allein waere zu leicht gruen zu
    # bekommen, und ein umbenannter Schluessel faellt so sofort auf.
    local schluessel klartext
    while read -r schluessel klartext; do
        [[ -z "$schluessel" ]] && continue
        if grep -qaF "[ ok ] $schluessel" "$log"; then
            echo "  [ ok ] $klartext"
        else
            echo "  [FEHL] $klartext ($schluessel)"
            rc=1
        fi
    done <<'ZUSAGEN'
version die ABI meldet ihre Fassung
count-is-one eine frische Tabelle haelt genau EIN Handle
write-ok schreiben durch das Handle gibt die Laenge zurueck
inspect Auskunft ueber das eigene Handle
read-denied lesen ohne Leserecht: RightsDenied, NICHT BadHandle
handle-zero das Handle 0 ist kein stdin
forged-gen eine erfundene Generation trifft nichts
slot-range ein Platz ausserhalb der Tabelle trifft nichts
unknown-nr eine Nummer, die es in dieser ABI nicht gibt
transfer-nopid uebertragen an eine pid, die es nicht gibt
dup-less vervielfaeltigen mit kleinerer Rechtemenge
dup-write die Kopie darf schreiben
dup-nodup die Kopie darf sich NICHT selbst vervielfaeltigen
close schliessen
after-close nach dem Schliessen trifft das Handle nichts mehr
close-twice zweimal schliessen ist ein Fehler, kein Absturz
exhausted eine volle Tabelle sagt Exhausted
alive und der Kernel lebt danach
ZUSAGEN

    if grep -qaF '[FAIL]' "$log"; then
        echo "  [FEHL] Ring 3 meldet eine gefallene Zusage"
        grep -aF '[FAIL]' "$log" | sed 's/^/         /'
        rc=1
    else
        echo "  [ ok ] keine gefallene Zusage in Ring 3"
    fi

    # Zwei Laeufe, beide vollstaendig — der zweite ist der eigentliche
    # Beweis, dass NICHTS GEERBT WIRD (siehe unten).
    local n
    n=$(grep -acF 'caps: 18/18 proofs' "$log")
    if [[ "$n" -eq 2 ]]; then
        echo "  [ ok ] beide unprivilegierten Laeufe melden 18 von 18 Zusagen"
    else
        echo "  [FEHL] $n Laeufe mit 18/18, erwartet 2"
        rc=1
    fi
    n=$(grep -acF 'cap: ring 3 exit=0' "$log")
    if [[ "$n" -eq 2 ]]; then
        echo "  [ ok ] beide Laeufe verlassen Ring 3 ohne gefallene Zusage"
    else
        echo "  [FEHL] $n Laeufe mit exit=0, erwartet 2"
        rc=1
    fi

    # NICHTS WIRD GEERBT. Der erste Lauf macht seine Tabelle absichtlich
    # voll (16 von 16). Der zweite bekommt DENSELBEN Platz der
    # Aufgabentabelle und zaehlt trotzdem genau EIN Handle.
    local zeile t1 t2 left
    zeile=$(grep -a 'cap: slot t1=' "$log" | tail -1)
    t1=$(sed -n 's/.*t1=\([0-9]*\).*/\1/p' <<<"$zeile")
    t2=$(sed -n 's/.*t2=\([0-9]*\).*/\1/p' <<<"$zeile")
    left=$(sed -n 's/.*left=\([0-9]*\).*/\1/p' <<<"$zeile")
    if [[ "${t1:-x}" == "${t2:-y}" && "${left:-0}" -eq 16 ]]; then
        echo "  [ ok ] Platz $t1 wiederverwendet, Vorgaenger liess $left Handles stehen — der Nachfolger zaehlte trotzdem eines"
    else
        echo "  [FEHL] die Zusage 'nichts wird geerbt' misst nichts: t1=$t1 t2=$t2 left=$left"
        rc=1
    fi

    # Der Wuerfelwert: derselbe Platz in zwei Prozessen, verschiedene
    # Handle-Werte. Ein abgehoertes Handle aus einem fremden Prozess
    # trifft damit auch dann nichts, wenn dort derselbe Platz belegt ist.
    if grep -qa 'cap: first .*differ=1' "$log"; then
        echo "  [ ok ] zwei Prozesse, derselbe Platz, verschiedene Handle-Werte"
    else
        echo "  [FEHL] die Handles zweier Prozesse sind gleich"
        rc=1
    fi

    # GEGENPROBE: ohne das Wort `caps` gibt es die Schicht nicht. Ein Test,
    # der auch ohne den Gegenstand gruen waere, misst nichts.
    local ohne
    ohne=$(mktemp)
    if ./run-osum.sh --log "$ohne" > build/osum-ohne-caps.log 2>&1 \
       && grep -qa 'cap: skipped' "$ohne" \
       && ! grep -qa 'caps: ' "$ohne"; then
        echo "  [ ok ] Gegenprobe: ohne das Wort caps meldet niemand Zusagen"
    else
        echo "  [FEHL] Gegenprobe fehlgeschlagen — der Nachweis haengt nicht am Gegenstand"
        rc=1
    fi
    rm -f "$ohne" "$log"
    return $rc
}
run osum_caps
