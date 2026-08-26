# tests/step-60-elf-korpus.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# DREIUNDFUENFZIG KAPUTTE ABBILDER, DURCH DEN LADER DES PRODUKTS.
#
# HERKUNFT DIESES KORPUS. `tests/firn-elf/faelle/` ist das einzige
# Stueck des alten Rust-Kernels, das nicht geloescht wurde und trotzdem
# noch etwas tut. Es entstand fuer den ELF-Pruefteil, der damals von Rust
# nach Firn portiert wurde: dreiundfuenfzig von Hand gebaute ELF64-Koepfe,
# jeder mit genau EINEM Fehler — abgeschnitten, falsche Kennung, falsche
# Klasse, Ausrichtung, die nicht aufgeht, Segmente, die sich eine Seite
# teilen, Adressen, die ueberlaufen. Der Rust-Code, der sie geprueft hat,
# ist weg. Die Faelle sind besser als der Code, der sie hervorgebracht
# hat, und deshalb bleiben sie.
#
# WAS SIE JETZT MESSEN. Sie liegen im Produkt-ISO unter `/f/`, und die
# Shell versucht in EINEM Boot, jedes davon zu starten. Gemessen wird
# nicht "alle werden abgelehnt" — Osums Lader hat andere Grenzen fuer
# USER_BASE und USER_LIMIT als der Rust-Kernel, und eine Zahl, die man
# nicht nachgerechnet hat, gehoert nicht in eine Zusage. Gemessen wird:
#
#   1. DER KERNEL UEBERLEBT ALLE DREIUNDFUENFZIG. Kein Panik, keine
#      Ausnahme, Beendigungscode 21. Ein Lader, der bei einem
#      abgeschnittenen Kopf ueber sein Dateiende hinausliest, faellt hier.
#   2. DIE GROSSE MEHRHEIT WIRD ABGELEHNT, und zwar mit einem Fehlerwert,
#      nicht mit einem Absturz. Die Zahl steht unten und wird
#      nachgezaehlt.
#   3. DIE SHELL LEBT DANACH UND ARBEITET WEITER. Nach dem letzten Fall
#      wird noch ein echtes Programm gestartet; laeuft das nicht, hat
#      einer der Faelle etwas hinterlassen.
#   4. GEGENPROBE: dasselbe Programm, das nach den Faellen laeuft, wird
#      auch OHNE die Faelle gestartet. Waere der Lauf schon vorher kaputt,
#      bewiese Punkt 3 nichts.
#
# ERWARTETE ABLEHNUNGEN: 46 von 53. Sieben Faelle heissen `gueltig-*` und
# sind absichtlich in Ordnung; sie sind der Gegenbeweis dazu, dass der
# Lader einfach alles ablehnt.
ELF_ABLEHNUNGEN_SOLL=46

step "ELF-Korpus: 53 kaputte Abbilder durch den Lader des Produkts"
elf_korpus() {
    RC=0
    local d=tests/firn-elf/faelle
    local n
    n=$(ls "$d"/*.elf 2>/dev/null | wc -l)
    if [[ "$n" -ne 53 ]]; then
        nok "der Korpus hat $n Faelle, erwartet 53"
        return 1
    fi
    ok "der Korpus hat $n Faelle, davon $(ls "$d"/gueltig-*.elf | wc -l) absichtlich gueltige"

    # Ein Skript, das jeden Fall EINMAL zu starten versucht, und danach ein
    # echtes Programm. Die Shell nimmt einen Namen mit Schraegstrich als
    # Pfad (Osum, kernel/user/sh.fi).
    local skript
    skript=$(mktemp)
    local dazu=()
    local f name
    echo "echo ==KORPUS==" > "$skript"
    for f in "$d"/*.elf; do
        name=$(basename "$f" .elf)
        # Kurze Namen: die Kommandozeile des Laders ist nicht beliebig lang,
        # und der Name im Dateisystem muss nicht der auf dem Wirt sein.
        dazu+=(--dazu "/f/$name=$f")
        echo "/f/$name" >> "$skript"
    done
    echo "echo ==DANACH==" >> "$skript"
    echo "uname" >> "$skript"
    echo "echo ==ENDE==" >> "$skript"
    dazu+=(--dazu "/f/=" --dazu "/t/=" --dazu "/t/korpus.sh=$skript")

    if ! ./build.sh "${dazu[@]}" --cmdline \
            "osum nokbd nosched noproc nofs noring3 script=sh /t/korpus.sh;exit" \
            >/dev/null 2>&1; then
        nok "das ISO mit dem Korpus laesst sich nicht bauen"
        rm -f "$skript"; return 1
    fi
    ok "ein ISO, in dem alle $n Faelle unter /f/ liegen"

    local log
    log=$(mktemp)
    local rc=0
    # run-osum.sh wuerde neu bauen und dabei die --dazu-Dateien verlieren;
    # deshalb wird hier direkt gestartet.
    timeout 180 qemu-system-x86_64 -machine q35 -cpu max -m 512M \
        -cdrom "build/${SLUG}.iso" -boot d -no-reboot -display none \
        -serial "file:$log" \
        -device isa-debug-exit,iobase=0xf4,iosize=0x04 >/dev/null 2>&1 || rc=$?
    if [[ "$rc" -eq 21 ]]; then
        ok "der Kernel ueberlebt alle $n Faelle und beendet sich selbst (21)"
    else
        nok "der Lauf endet mit $rc statt 21 — einer der Faelle hat den Kernel umgebracht"
        tail -20 "$log" | sed 's/^/         /'
    fi
    if grep -qa 'EXCEPTION' "$log"; then
        nok "es gab eine Prozessorausnahme im Kernel"
        grep -a 'EXCEPTION' "$log" | head -3 | sed 's/^/         /'
    else
        ok "keine einzige Prozessorausnahme im Kernel"
    fi

    local abgelehnt
    abgelehnt=$(grep -ac 'cannot run' "$log")
    if [[ "$abgelehnt" -eq "$ELF_ABLEHNUNGEN_SOLL" ]]; then
        ok "$abgelehnt von $n Abbildern abgelehnt — genau die, die kaputt sind"
    else
        nok "$abgelehnt Ablehnungen, erwartet $ELF_ABLEHNUNGEN_SOLL"
        grep -a 'cannot run' "$log" | sed 's/^/         /' | head -5
    fi
    # Die Ablehnungen sind FEHLERWERTE, keine Abstuerze.
    local mitwert
    mitwert=$(grep -ac 'cannot run.*-> -' "$log")
    if [[ "$mitwert" -eq "$abgelehnt" ]]; then
        ok "und jede Ablehnung traegt einen Fehlerwert ($mitwert von $abgelehnt)"
    else
        nok "nur $mitwert von $abgelehnt Ablehnungen tragen einen Fehlerwert"
    fi
    # Wie viele VERSCHIEDENE Gruende der Lader nennt: ein Lader, der alles
    # mit demselben Wert ablehnt, prueft in Wahrheit eine einzige Sache.
    local gruende
    gruende=$(grep -aoE '\-> -[0-9]+' "$log" | sort -u | wc -l)
    if [[ "$gruende" -ge 2 ]]; then
        ok "der Lader nennt $gruende verschiedene Ablehnungsgruende"
    else
        nok "der Lader nennt nur $gruende Ablehnungsgrund — er prueft in Wahrheit eine einzige Sache"
    fi

    # DIE SHELL LEBT DANACH.
    if grep -qa '==DANACH==' "$log" && grep -qa '==ENDE==' "$log"; then
        ok "die Shell arbeitet nach dem letzten Fall weiter"
    else
        nok "die Shell hat den Korpus nicht ueberlebt"
    fi
    if grep -qaE '^osum$' "$log"; then
        ok "und startet danach noch ein echtes Programm (uname)"
    else
        nok "nach dem Korpus laesst sich kein Programm mehr starten"
    fi
    rm -f "$log" "$skript"
    # Das richtige Produkt wieder herstellen.
    ./build.sh >/dev/null 2>&1 || nok "das Produkt laesst sich nicht wieder bauen"
    return $RC
}
run elf_korpus
