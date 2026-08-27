# SPDX-License-Identifier: GPL-2.0-only
# tests/step-50-schutz.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# ZWEI EIGENSCHAFTEN, DIE DEN KERNELWECHSEL UEBERLEBT HABEN, im Produkt
# gemessen — nicht im Kernel-Repo, sondern hier, aus dem ISO heraus.
#
#   1. DIE CAPABILITY-SCHICHT. Sie war die inhaltlich wertvollste
#      Eigenschaft des Rust-Kernels: kein globaler Namensraum, keine
#      geerbten Deskriptoren, jeder Zugriff ueber ein Handle mit Rechten.
#      Beim Wechsel auf Osum waere sie verloren gegangen — Osums
#      POSIX-Schicht arbeitet mit Umgebungsautoritaet. Der Kern des
#      Handle-Modells ist deshalb nach Firn portiert worden (Osum,
#      `kernel/cap.fi`). Gemessen wird, was ein unprivilegiertes Programm
#      in RING 3 ueber seine eigenen Rechte meldet.
#
#      Der entscheidende Unterschied zu POSIX steht in einer einzigen
#      Zusage: `read-denied` — ein GUELTIGES Handle ohne das noetige
#      Recht ist RightsDenied, ein GEFAELSCHTES ist BadHandle. POSIX hat
#      fuer beides nur -EBADF.
#
#   2. SMEP UND SMAP. Der letzte offene Punkt aus KERNELWECHSEL.md § 4.1,
#      portiert am 26.08.2026 nach Osum (`kernel/guard.fi`). Hier wird
#      gemessen, dass das PRODUKT sie wirklich anschaltet — auf einem
#      Prozessor, der sie hat. Gegenprobe: auf QEMUs Vorgabeprozessor
#      meldet derselbe Kernel ehrlich 0 und laeuft trotzdem.
step "Capabilities aus Ring 3: 18 Zusagen im Produkt"
osum_caps() {
    RC=0
    local log
    log=$(mktemp)
    if ! ./run-osum.sh --cmdline "osum nokbd nosched nofs noring3 caps" \
            --log "$log" > build/caps.log 2>&1; then
        nok "der Kernel ist mit der Capability-Runde nicht sauber beendet"
        tail -20 build/caps.log | sed 's/^/         /'
        rm -f "$log"; return 1
    fi
    ok "Boot mit der Capability-Runde: Beendigungscode 21"

    # Jede Zusage EINZELN. Eine Sammelzahl allein waere zu leicht gruen zu
    # bekommen, und ein umbenannter Schluessel faellt so sofort auf.
    local schluessel klartext
    while read -r schluessel klartext; do
        [[ -z "$schluessel" ]] && continue
        if grep -qaF "[ ok ] $schluessel" "$log"; then
            ok "$klartext"
        else
            nok "$klartext ($schluessel)"
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
        nok "Ring 3 meldet eine gefallene Zusage"
        grep -aF '[FAIL]' "$log" | sed 's/^/         /'
    else
        ok "keine gefallene Zusage in Ring 3"
    fi

    local n
    n=$(grep -acF 'caps: 18/18 proofs' "$log")
    [[ "$n" -eq 2 ]] && ok "beide unprivilegierten Laeufe melden 18 von 18 Zusagen" \
                     || nok "$n Laeufe mit 18/18, erwartet 2"
    n=$(grep -acF 'cap: ring 3 exit=0' "$log")
    [[ "$n" -eq 2 ]] && ok "beide Laeufe verlassen Ring 3 ohne gefallene Zusage" \
                     || nok "$n Laeufe mit exit=0, erwartet 2"

    # NICHTS WIRD GEERBT. Der erste Lauf macht seine Tabelle absichtlich
    # voll (16 von 16). Der zweite bekommt DENSELBEN Platz der
    # Aufgabentabelle und zaehlt trotzdem genau EIN Handle.
    local zeile t1 t2 left
    zeile=$(grep -a 'cap: slot t1=' "$log" | tail -1)
    t1=$(sed -n 's/.*t1=\([0-9]*\).*/\1/p' <<<"$zeile")
    t2=$(sed -n 's/.*t2=\([0-9]*\).*/\1/p' <<<"$zeile")
    left=$(sed -n 's/.*left=\([0-9]*\).*/\1/p' <<<"$zeile")
    if [[ "${t1:-x}" == "${t2:-y}" && "${left:-0}" -eq 16 ]]; then
        ok "Platz $t1 wiederverwendet, Vorgaenger liess $left Handles stehen — der Nachfolger zaehlte trotzdem eines"
    else
        nok "die Zusage 'nichts wird geerbt' misst nichts: t1=$t1 t2=$t2 left=$left"
    fi
    grep -qa 'cap: first .*differ=1' "$log" \
        && ok "zwei Prozesse, derselbe Platz, verschiedene Handle-Werte" \
        || nok "die Handles zweier Prozesse sind gleich"

    # GEGENPROBE: ohne das Wort `caps` gibt es die Schicht nicht. Ein Test,
    # der auch ohne den Gegenstand gruen waere, misst nichts.
    local ohne
    ohne=$(mktemp)
    if ./run-osum.sh --log "$ohne" > build/ohne-caps.log 2>&1 \
       && grep -qa 'cap: skipped' "$ohne" \
       && ! grep -qa 'caps: ' "$ohne"; then
        ok "Gegenprobe: ohne das Wort caps meldet niemand Zusagen"
    else
        nok "Gegenprobe fehlgeschlagen — der Nachweis haengt nicht am Gegenstand"
    fi
    rm -f "$ohne" "$log"
    return $RC
}
run osum_caps

step "Schutzbits: das Produkt schaltet SMEP und SMAP ein"
schutzbits() {
    RC=0
    local log
    log=$(mktemp)
    ./run-osum.sh --log "$log" > build/schutz.log 2>&1 \
        && ok "Boot auf einem Prozessor mit beiden Bits (21)" \
        || nok "der Boot mit -cpu max ist fehlgeschlagen"
    if grep -qa 'guard: cr4=0x300020  smep=1  smap=1' "$log"; then
        ok "CR4 traegt Bit 20 und Bit 21 — zurueckgelesen, nicht behauptet"
    else
        nok "SMEP/SMAP sind im Produkt nicht an"
        grep -a '^guard:' "$log" | sed 's/^/         /'
    fi
    local w
    w=$(grep -aoE 'windows=[0-9]+' "$log" | tail -1 | grep -oE '[0-9]+')
    if [[ "${w:-0}" -gt 0 ]]; then
        ok "das SMAP-Fenster wurde ${w}x geoeffnet — der Kernel fasst wirklich Ring-3-Speicher an"
    else
        nok "das Fenster wurde nie geoeffnet (windows=${w:-?}) — die Zusage misst nichts"
    fi

    # GEGENPROBE 1: auf einem Prozessor ohne die Bits meldet derselbe
    # Kernel ehrlich 0 und laeuft trotzdem durch. Ein Kernel, der ein Bit
    # behauptet, das die Maschine nicht hat, waere schlimmer als einer
    # ohne das Bit.
    ./run-osum.sh --cpu qemu64 --log "$log" > build/schutz-alt.log 2>&1 \
        && ok "Gegenprobe: auf einem Prozessor ohne die Bits laeuft dasselbe ISO (21)" \
        || nok "auf qemu64 laeuft das Produkt nicht"
    grep -qa 'guard: cr4=0x20  smep=0  smap=0  cpu=0/0' "$log" \
        && ok "und meldet ehrlich smep=0 smap=0, statt etwas zu behaupten" \
        || { nok "die Meldung auf qemu64 stimmt nicht"; grep -a '^guard:' "$log" | sed 's/^/         /'; }

    # GEGENPROBE 2: abgeschaltet ist abgeschaltet.
    ./run-osum.sh --cmdline "osum nokbd nosched noproc nofs noring3 nosmap" \
        --log "$log" > build/schutz-nosmap.log 2>&1 \
        && grep -qa 'smep=1  smap=0' "$log" \
        && ok "Gegenprobe: 'nosmap' nimmt SMAP und laesst SMEP stehen" \
        || nok "'nosmap' wirkt nicht wie beschrieben"
    ./run-osum.sh --cmdline "osum nokbd nosched noproc nofs noring3 nosmep" \
        --log "$log" > build/schutz-nosmep.log 2>&1 \
        && grep -qa 'smep=0  smap=1' "$log" \
        && ok "Gegenprobe: 'nosmep' nimmt SMEP und laesst SMAP stehen" \
        || nok "'nosmep' wirkt nicht wie beschrieben"

    # GEGENPROBE 3, die schaerfste: ein Zugriff, der scheitern MUSS. Der
    # Kernel liest EINE Zelle Ring-3-Speicher ohne das Fenster.
    if ./run-osum.sh --cmdline "osum nokbd nosched noproc nofs noring3 smapraw" \
            --log "$log" > build/schutz-raw.log 2>&1; then
        nok "'smapraw' laeuft durch — SMAP setzt nichts durch"
    else
        if grep -qa 'EXCEPTION 14 #PF  err=0x1' "$log"; then
            ok "'smapraw' endet in einem #PF (err=0x1) — SMAP setzt wirklich durch"
        else
            nok "'smapraw' bleibt stehen, aber nicht an einem #PF"
            tail -6 "$log" | sed 's/^/         /'
        fi
    fi
    if ./run-osum.sh --cmdline "osum nokbd nosched noproc nofs noring3 smapraw nosmap" \
            --log "$log" > build/schutz-raw-off.log 2>&1 \
       && grep -qa 'got=0x5a' "$log"; then
        ok "derselbe Zugriff mit 'nosmap' kommt zurueck und liest 0x5a — der Unterschied ist EIN Bit in CR4"
    else
        nok "der Gegenlauf ohne SMAP verhaelt sich nicht anders"
    fi
    rm -f "$log"
    return $RC
}
run schutzbits
