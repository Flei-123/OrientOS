# tests/step-60-elf-korpus.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# DREIUNDFUENFZIG KAPUTTE ABBILDER, DURCH DEN LADER DES PRODUKTS.
#
# HERKUNFT DIESES KORPUS. `tests/firn-elf/faelle/` ist das einzige Stueck
# des alten Rust-Kernels, das nicht geloescht wurde und trotzdem noch
# etwas tut. Es entstand fuer den ELF-Pruefteil, der damals von Rust nach
# Firn portiert wurde: dreiundfuenfzig von Hand gebaute ELF64-Koepfe,
# jeder mit genau EINEM Fehler — abgeschnitten, falsche Kennung, falsche
# Klasse, Ausrichtung, die nicht aufgeht, Segmente, die sich eine Seite
# teilen, Adressen, die ueberlaufen. Der Rust-Code, der sie geprueft hat,
# ist weg. Die Faelle sind besser als der Code, der sie hervorgebracht
# hat, und deshalb bleiben sie.
#
# WARUM AUCH DIE SIEBEN `gueltig-*` ABGELEHNT WERDEN, und das ist ein
# Messergebnis und keine Panne: der Korpus wurde gegen die
# Adressaufteilung des RUST-Kernels gebaut. Seine gueltigen Faelle laden
# nach `0x401000`; Osums Bildbereich beginnt bei `0x40100000`
# (`proc.IMAGE_BASE`). Ein Abbild, das ausserhalb liegt, ist fuer Osums
# Lader nicht gueltig — er weist es mit `reason 15  outside the user
# region` ab, und das ist genau richtig. Der Korpus misst hier also
# nicht mehr "erkennt der Lader gueltig von ungueltig", sondern das,
# wofuer 46 seiner 53 Faelle ohnehin gebaut wurden:
#
#   1. DER KERNEL UEBERLEBT ALLE DREIUNDFUENFZIG. Kein Absturz, keine
#      Prozessorausnahme, Beendigungscode 21. Ein Lader, der bei einem
#      abgeschnittenen Kopf ueber sein Dateiende hinausliest, faellt hier.
#   2. ER LEHNT JEDES EINZELN AB, mit einem Grund, den er nennt. Nicht
#      "ging nicht", sondern `elf: refused, reason N  <Klartext>`.
#   3. ER NENNT VIELE VERSCHIEDENE GRUENDE. Ein Lader, der alles mit
#      demselben Wert ablehnt, prueft in Wahrheit eine einzige Sache.
#      Gemessen: 13 verschiedene Gruende auf 53 Faelle.
#   4. DIE SHELL LEBT DANACH UND ARBEITET WEITER. Nach dem letzten Fall
#      wird noch ein echtes Programm gestartet; laeuft das nicht, hat
#      einer der Faelle etwas hinterlassen.
#
# Alles in EINEM Boot: 53 Ladeversuche aus Ring 3, ueber `exec`, aus
# einem Dateisystem, das als Boot-Modul im ISO liegt.
ELF_FAELLE_SOLL=53
ELF_GRUENDE_MIN=10

step "ELF-Korpus: 53 kaputte Abbilder durch den Lader des Produkts"
elf_korpus() {
    RC=0
    local d=tests/firn-elf/faelle
    local n
    n=$(ls "$d"/*.elf 2>/dev/null | wc -l)
    if [[ "$n" -ne "$ELF_FAELLE_SOLL" ]]; then
        nok "der Korpus hat $n Faelle, erwartet $ELF_FAELLE_SOLL"
        return 1
    fi
    ok "der Korpus hat $n Faelle"

    # Ein Skript, das jeden Fall EINMAL zu starten versucht, und danach ein
    # echtes Programm. Die Shell nimmt einen Namen mit Schraegstrich als
    # Pfad (Osum, kernel/user/sh.fi).
    local skript
    skript=$(mktemp)
    local dazu=()
    local f i=0 kurz
    echo "echo ==KORPUS==" > "$skript"
    for f in "$d"/*.elf; do
        i=$((i + 1))
        # KURZE NAMEN, und das ist kein Schoenheitsfehler: OFS haelt einen
        # Verzeichniseintrag in fester Breite, und `align-keine-
        # zweierpotenz.elf` passt nicht hinein. Der Name im Dateisystem
        # muss nicht der auf dem Wirt sein — gemessen wird der INHALT.
        kurz=$(printf 'c%02d' "$i")
        dazu+=(--dazu "/f/$kurz=$f")
        echo "/f/$kurz" >> "$skript"
    done
    echo "echo ==DANACH==" >> "$skript"
    echo "uname" >> "$skript"
    echo "echo ==ENDE==" >> "$skript"
    dazu=(--dazu "/f/" --dazu "/t/" "${dazu[@]}" --dazu "/t/korpus.sh=$skript")

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

    # DER KERNEL sagt, warum. Nicht die Shell — die sieht nur -8
    # (ENOEXEC) und koennte einen abgeschnittenen Kopf nicht von einer
    # falschen Maschine unterscheiden.
    local abgelehnt
    abgelehnt=$(grep -ac '^elf: refused' "$log")
    if [[ "$abgelehnt" -eq "$n" ]]; then
        ok "der Lader lehnt alle $abgelehnt ab, jedes einzeln und mit genanntem Grund"
    else
        nok "$abgelehnt Ablehnungen, erwartet $n"
        grep -a '^elf: refused' "$log" | head -5 | sed 's/^/         /'
    fi
    # ...und die Shell bekommt fuer jede einen Fehlerwert, keinen Absturz.
    local ausring3
    ausring3=$(grep -ac 'cannot run.*-> -' "$log")
    if [[ "$ausring3" -eq "$n" ]]; then
        ok "und Ring 3 bekommt $ausring3 Fehlerwerte zurueck — keinen einzigen Absturz"
    else
        nok "Ring 3 bekommt nur $ausring3 von $n Fehlerwerten"
    fi
    # Wie viele VERSCHIEDENE Gruende: ein Lader, der alles mit demselben
    # Wert ablehnt, prueft in Wahrheit eine einzige Sache.
    local gruende
    gruende=$(grep -a '^elf: refused' "$log" | sed 's/.*reason \([0-9]*\).*/\1/' | sort -u | wc -l)
    if [[ "$gruende" -ge "$ELF_GRUENDE_MIN" ]]; then
        ok "er nennt $gruende verschiedene Gruende ($(grep -a '^elf: refused' "$log" \
            | sed 's/.*reason \([0-9]*\).*/\1/' | sort -un | tr '\n' ' '))"
    else
        nok "er nennt nur $gruende verschiedene Gruende — er prueft in Wahrheit eine einzige Sache"
    fi
    # Die Gruende im Klartext, damit sichtbar bleibt, WAS er unterscheidet.
    local klartexte
    klartexte=$(grep -a '^elf: refused' "$log" | sed 's/^elf: refused, //' | sort -u | wc -l)
    ok "die Gruende im Klartext: $klartexte verschiedene Meldungen"

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
