# SPDX-License-Identifier: GPL-2.0-only
# tests/step-40-userland.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# DAS PRODUKT HAT EIN USERLAND, UND ZWAR IM ISO.
#
# Das ist der Punkt, an dem sich vor dem 26.08.2026 nichts befand. Der
# alte Rust-Kernel hatte EIN unprivilegiertes Programm — `hello.asm`, 64
# Zeilen Assembler, in einem eigenen Archivformat als Bootloader-Modul.
# Osum hatte eine Shell und fuenfundzwanzig Werkzeuge, aber sie lagen auf
# einer PLATTE, die QEMU mit `-drive` hereinreicht; ein ISO hat keine.
#
# Was die beiden zusammenbringt, ist Osums Runde K10: der Kernel nimmt
# ein Boot-Modul entgegen, prueft dessen CRC32 und mountet es als Wurzel
# (dort `kernel/bootmod.fi`). OrientOS stellt dieses Dateisystem
# zusammen (`userland/PROGRAMME`) und legt es neben den Kern.
#
# Gemessen wird deshalb nicht "es gibt ein Modul", sondern: ein
# unprivilegiertes Programm, das der Kernel nie gesehen hat, laeuft aus
# diesem Modul in Ring 3 und sagt etwas, das nur es sagen kann.
step "Userland: eine Shell aus dem Boot-Modul, in Ring 3"
userland_check() {
    RC=0
    local log
    log=$(mktemp)
    if ./run-osum.sh --script 'ls /bin;uname;cat /etc/ausgabe.txt;wc -l < /etc/ausgabe.txt;exit' \
            --log "$log" > build/userland.log 2>&1; then
        ok "der Lauf endet regulaer (21)"
    else
        nok "der Lauf mit dem Userland ist fehlgeschlagen"
        tail -20 build/userland.log | sed 's/^/         /'
        rm -f "$log"; return 1
    fi

    local crc crc_k
    crc=$(python3 -c 'import zlib,sys;print("%08x"%zlib.crc32(open(sys.argv[1],"rb").read()))' \
          "build/${SLUG}-userland.img")
    # OHNE FUEHRENDE NULLEN, denn so schreibt der Kernel sie: `serial.hex`
    # gibt die Ziffern aus, die die Zahl hat, und nicht acht Stueck. Das
    # ist einmal aufgefallen, als die Summe zufaellig mit einer Null
    # anfing (0x04ed509d) und der Vergleich gegen "04ed509d" fiel.
    crc_k=$(printf '%x' "0x$crc")
    if grep -qaF "crc=0x$crc_k  want=0x$crc_k  ok=1" "$log"; then
        ok "der Kernel rechnet dieselbe Pruefsumme wie der Wirt (0x$crc)"
    else
        nok "die Pruefsumme des Moduls stimmt im Kernel nicht"
        grep -a '^mod:' "$log" | sed 's/^/         /'
    fi
    grep -qa 'osum: from module' "$log" \
        && ok "die Wurzelplatte ist das Boot-Modul, nicht eine Platte" \
        || nok "der Kernel hat das Modul nicht als Wurzel genommen"
    grep -qa 'osum: mount=1' "$log" \
        && ok "das Dateisystem darin ist gemountet" \
        || nok "das Dateisystem im Modul liess sich nicht mounten"
    grep -qa 'sh: ready' "$log" \
        && ok "/bin/sh laeuft — eine ELF-Datei, die der Kernel nie gesehen hat" \
        || nok "/bin/sh ist nicht gestartet"

    # Was die SHELL gesagt hat, und nichts anderes.
    local prog n fehlt="" zeile
    n=0
    # GENAU die Zeile, die `ls /bin` geschrieben hat, und keine andere:
    # ein Programmname, der irgendwo sonst im Protokoll auftaucht, ist
    # kein Beleg dafuer, dass die Datei im Abbild liegt.
    zeile=$(grep -a '^\./ \.\./ ' "$log" | tail -1)
    for prog in sh ls cat cp mv rm wc grep sort uname date df ping wget; do
        if grep -qE "(^| )$prog( |$)" <<<"$zeile"; then
            n=$((n+1))
        else
            fehlt="$fehlt $prog"
        fi
    done
    [[ -z "$fehlt" ]] && ok "'ls /bin' zeigt alle $n geprueften Werkzeuge des Produkts" \
                      || nok "'ls /bin' zeigt$fehlt nicht (von 14 geprueften)"
    grep -qa '^osum$' "$log" \
        && ok "'uname' antwortet aus Ring 3" \
        || nok "'uname' hat nicht geantwortet"

    # DIE DATEI, DIE AUS DIESEM REPO KOMMT. Sie ist der Nachweis, dass
    # OrientOS dem Produkt wirklich etwas hinzufuegt und nicht nur
    # weiterreicht, was Osum baut.
    if grep -qa 'Betriebssystem von Grund auf' "$log"; then
        ok "'cat /etc/ausgabe.txt' liest die Datei, die OrientOS beisteuert"
    else
        nok "/etc/ausgabe.txt ist nicht angekommen"
    fi
    local zeilen soll
    soll=$(wc -l < userland/dateien/ausgabe.txt)
    zeilen=$(grep -aoE '^[0-9]+$' "$log" | tail -1)
    if [[ "${zeilen:-x}" == "$soll" ]]; then
        ok "'wc -l' zaehlt $soll Zeilen darin — dieselbe Zahl wie auf dem Wirt"
    else
        nok "'wc -l' zaehlt ${zeilen:-?}, der Wirt zaehlt $soll"
    fi

    grep -qa 'osum: sh exit=0' "$log" \
        && ok "die Shell endet mit 0" \
        || nok "die Shell endet nicht mit 0"
    # Kein Rahmen bleibt liegen: was das Userland genommen hat, kam zurueck.
    local f a b
    f=$(grep -aoE 'frames_free=[0-9]+ of [0-9]+' "$log" | tail -1)
    a=$(echo "$f" | grep -oE '[0-9]+' | head -1)
    b=$(echo "$f" | grep -oE '[0-9]+' | tail -1)
    if [[ -n "$a" && "$a" == "$b" ]]; then
        ok "jeder Rahmen, den das Userland nahm, kam zurueck ($f)"
    else
        nok "Rahmen sind liegengeblieben: $f"
    fi
    # Und das Modul ist am Ende des Laufs noch dasselbe: der Bereich war
    # im Rahmenverwalter wirklich reserviert.
    if grep -qaF "mod: recheck crc=0x$crc_k  same=1" "$log"; then
        ok "nach dem ganzen Lauf ist das Modul unveraendert — sein Speicher gehoerte ihm"
    else
        nok "das Modul hat sich waehrend des Laufs veraendert"
        grep -a 'recheck' "$log" | sed 's/^/         /'
    fi
    rm -f "$log"
    return $RC
}
run userland_check

# ---------------------------------------------------------------------------
#
# DIE GEGENPROBEN. Ein Nachweis, der auch ohne seinen Gegenstand gruen
# waere, misst nichts. Also: dasselbe Produkt einmal OHNE das Modul und
# einmal mit einer Pruefsumme, die nicht passt.
step "Gegenproben: ohne Modul und mit falscher Pruefsumme laeuft nichts"
userland_gegenproben() {
    RC=0
    local log
    log=$(mktemp)
    # --- 1. ohne Modul
    if ./run-osum.sh --ohne-userland --log "$log" > build/ohne-userland.log 2>&1; then
        ok "ohne Userland startet der Kernel weiterhin (21)"
    else
        nok "ohne Userland startet der Kernel nicht mehr"
    fi
    grep -qa '^mod: none' "$log" \
        && ok "und sagt 'mod: none', statt etwas zu raten" \
        || nok "der Kernel meldet kein 'mod: none'"
    grep -qa 'sh: ready' "$log" \
        && nok "es laeuft trotzdem eine Shell — der Nachweis haengt nicht am Modul" \
        || ok "es laeuft keine Shell"
    grep -qaE '^ *module_path' build/isoroot/boot/limine/limine.conf \
        && nok "limine.conf nennt trotzdem ein Modul" \
        || ok "limine.conf nennt kein Modul"

    # --- 2. mit falscher Pruefsumme
    if ./run-osum.sh --kaputte-summe --log "$log" > build/kaputte-summe.log 2>&1; then
        ok "mit falscher Pruefsumme startet der Kernel weiterhin (21)"
    else
        nok "mit falscher Pruefsumme bleibt der Kernel stehen — er soll das Modul nur liegen lassen"
    fi
    grep -qa 'want=0xdeadbeef  ok=0' "$log" \
        && ok "er sagt, dass die Summe nicht passt (ok=0)" \
        || { nok "der Kernel meldet die falsche Summe nicht"; grep -a '^mod:' "$log" | sed 's/^/         /'; }
    grep -qa 'osum: from module' "$log" \
        && nok "er benutzt das Modul trotzdem — die Pruefsumme ist wirkungslos" \
        || ok "und benutzt das Modul NICHT"
    grep -qa 'sh: ready' "$log" \
        && nok "es laeuft trotzdem eine Shell daraus" \
        || ok "es laeuft keine Shell daraus"
    rm -f "$log"
    # Zum Schluss das richtige Produkt wieder herstellen.
    ./build.sh >/dev/null 2>&1 || nok "das Produkt laesst sich nicht wieder bauen"
    return $RC
}
run userland_gegenproben
