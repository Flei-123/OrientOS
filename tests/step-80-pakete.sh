# tests/step-80-pakete.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# DIE PAKETVERWALTUNG (Roadmap 6.1 und 6.2, Format in PAKETE.md).
#
# Was hier gemessen wird, und was ausdruecklich NICHT als Nachweis zaehlt.
#
# Der Entwurf in PACKAGING.md macht vier Zusagen, und drei davon sind
# genau dann etwas wert, wenn die Gegenprobe dazu WIRKLICH umfaellt:
#
#   * „Deinstallieren heisst: zwei Verzeichnisse loeschen. Reste sind per
#     Konstruktion unmoeglich."  -> Der Baum nach Installieren und
#     Entfernen muss Oktett fuer Oktett der Baum von vorher sein. Ein
#     Vergleich, der das behauptet, muss ZEIGEN koennen, dass er einen
#     Unterschied auch faende: deshalb steht neben jedem Vergleich einer,
#     der fehlschlagen MUSS.
#   * „Gleicher Hash = bitgleiche Software."  -> Eine Pruefsumme, die
#     alles durchlaesst, ist keine. Geprueft wird an DREI Arten von
#     Beschaedigung -- in den Daten, in den Metadaten und am genannten
#     Hash selbst -- und dazu daran, dass ein UNVERSEHRTES Paket
#     angenommen wird. Ohne die letzte Zusage waere ein Werkzeug, das
#     grundsaetzlich nichts annimmt, hier gruen.
#   * „Rollback in Sekunden."  -> Eine Generation zurueck, und der alte
#     Stand ist wirklich wieder da. Nicht „kein Fehler gemeldet",
#     sondern: dieselbe kanonische Beschreibung von `apps/` wie vorher.
#
# DIE WARNUNG AUS DEN FRUEHEREN RUNDEN steht hier als eigene Zusage:
# ein Vergleich zweier LEERER Baeume geht immer durch. Jeder Vergleich
# in diesem Schritt sagt deshalb auch, WIE VIEL er verglichen hat, und
# eine Zeilenzahl unter einer Schranke laesst die Zusage fallen.
step "Pakete: Format, Pruefsumme, Generationen, Quelle -- jede Zusage mit ihrer Gegenprobe"
pakete_check() {
    RC=0
    local O="python3 pkg/opkg.py"
    local T; T=$(mktemp -d "${TMPDIR:-/tmp}/orientos-pakete-XXXXXX")

    test -d build/pakete || ./pkg/bauen.sh >/dev/null 2>&1
    if [[ ! -f build/pakete/explorer.opkg ]]; then
        nok "build/pakete/explorer.opkg fehlt — pkg/bauen.sh ist nicht durchgelaufen"
        rm -rf "$T"; return 1
    fi

    # ------------------------------------------------------ 1. das Format
    #
    # Der Hash ist die IDENTITAET eines Pakets. Waere der Bau nicht
    # wiederholbar, waere „gleicher Hash = gleicher Inhalt" nur die halbe
    # Wahrheit: derselbe Inhalt haette dann verschiedene Hashes, und der
    # Store liefe voll mit Eintraegen, die dasselbe sind.
    local h1 h2
    $O bauen build/pakete/explorer.rezept -o "$T/a.opkg" >/dev/null 2>&1
    $O bauen build/pakete/explorer.rezept -o "$T/b.opkg" >/dev/null 2>&1
    h1=$(sha256sum "$T/a.opkg" | cut -d' ' -f1)
    h2=$(sha256sum "$T/b.opkg" | cut -d' ' -f1)
    if [[ "$h1" == "$h2" && -s "$T/a.opkg" ]]; then
        ok "zweimal gebaut, Oktett fuer Oktett dasselbe Paket ($(stat -c%s "$T/a.opkg") Oktette)"
    else
        nok "derselbe Bau ergibt zwei verschiedene Pakete — der Hash taugt nicht als Identitaet"
    fi
    # Der Hash IM Paket ist der Hash UEBER das Paket, nicht irgendeiner.
    local imkopf gerechnet
    imkopf=$($O zeigen "$T/a.opkg" | awk '/^hash/{print $2}')
    gerechnet=$(python3 -c "
import hashlib,sys,struct
roh=open(sys.argv[1],'rb').read()
ml,dl=struct.unpack_from('<QQ',roh,8)
print(hashlib.sha256(roh[64:64+ml+dl]).hexdigest())" "$T/a.opkg")
    [[ "$imkopf" == "$gerechnet" ]] \
        && ok "der Hash im Kopf ist die SHA-256 ueber Metadaten und Daten (${imkopf:0:16})" \
        || nok "der Hash im Kopf passt nicht zum Inhalt"

    # ------------------------------------------- 2. die falsche Pruefsumme
    #
    # Drei Beschaedigungen, jede an einer anderen Stelle. Die dritte ist
    # die interessante: dort wird der GENANNTE Hash veraendert und nicht
    # der Inhalt. Ein Werkzeug, das den Hash nur weiterreicht, statt ihn
    # nachzurechnen, faellt genau hier durch.
    python3 - "$T" <<'PY'
import sys
T = sys.argv[1]
roh = open('build/pakete/explorer.opkg', 'rb').read()
for pos, name in ((70000, 'daten'), (70, 'meta'), (24, 'hash')):
    b = bytearray(roh)
    b[pos] ^= 0x01
    open('%s/kaputt-%s.opkg' % (T, name), 'wb').write(bytes(b))
b = bytearray(roh)
open('%s/heil.opkg' % T, 'wb').write(bytes(b))
PY
    local art abgelehnt=0
    for art in daten meta hash; do
        if $O installieren --wurzel "$T/w1" "$T/kaputt-$art.opkg" >"$T/aus" 2>&1; then
            nok "ein Paket mit beschaedigtem Teil '$art' wurde ANGENOMMEN"
        else
            abgelehnt=$((abgelehnt + 1))
            ok "beschaedigt in '$art' -> abgelehnt: $(grep -m1 Pruefsumme "$T/aus" | cut -c1-70)"
        fi
    done
    # ...UND die Gegenprobe zur Ablehnung: das unversehrte Paket MUSS
    # durchgehen. Sonst waere „lehnt alles ab" hier eine bestandene Probe.
    if $O installieren --wurzel "$T/w1" "$T/heil.opkg" >"$T/aus" 2>&1; then
        ok "dasselbe Paket UNVERSEHRT wird angenommen — die Pruefung laesst nicht alles fallen"
    else
        nok "auch das unversehrte Paket wird abgelehnt: $(head -1 "$T/aus")"
    fi

    # --------------------------- 3. installieren, entfernen, wie vorher
    #
    # Verglichen wird die kanonische Beschreibung des Baums: Art, Pfad,
    # Modus, Groesse und die SHA-256 jedes Inhalts.
    local w="$T/w2"
    $O installieren --wurzel "$w" build/pakete/terminal.opkg >/dev/null 2>&1
    $O baum --wurzel "$w" --ohne system > "$T/vorher"
    local zeilen; zeilen=$(wc -l < "$T/vorher")
    if [[ "$zeilen" -ge 15 ]]; then
        ok "der Baum vorher hat $zeilen Eintraege — es wird etwas verglichen und nicht nichts"
    else
        nok "der Baum vorher hat nur $zeilen Eintraege — dieser Vergleich waere leer"
    fi
    $O installieren --wurzel "$w" build/pakete/explorer.opkg >/dev/null 2>&1
    $O baum --wurzel "$w" --ohne system > "$T/mittendrin"
    # DIE GEGENPROBE ZUM VERGLEICH SELBST: mittendrin MUSS anders sein.
    if cmp -s "$T/vorher" "$T/mittendrin"; then
        nok "der Baum aendert sich durch eine Installation NICHT — der Vergleich misst nichts"
    else
        ok "eine Installation veraendert den Baum um $(diff "$T/vorher" "$T/mittendrin" | grep -c '^[<>]') Eintraege"
    fi
    $O entfernen --wurzel "$w" explorer >/dev/null 2>&1
    $O aufraeumen --wurzel "$w" --behalte 1 >/dev/null 2>&1
    $O baum --wurzel "$w" --ohne system > "$T/nachher"
    if cmp -s "$T/vorher" "$T/nachher"; then
        ok "nach Installieren und Entfernen ist der Baum Oktett fuer Oktett der von vorher ($zeilen Eintraege)"
    else
        nok "es sind Reste geblieben:"
        diff "$T/vorher" "$T/nachher" | head -10 | sed 's/^/         /'
    fi
    # Und die Generationen? Die BLEIBEN, das ist Absicht. Was danach noch
    # da ist, wird benannt statt verschwiegen.
    local geng
    geng=$(ls "$w/system/generations" | wc -l)
    if [[ "$geng" -eq 1 ]]; then
        ok "uebrig bleibt genau EINE Generation (Nr. $(cat "$w/system/AKTUELL")) — die Nummern zaehlen weiter, das ist ein Protokoll"
    else
        nok "nach dem Aufraeumen stehen $geng Generationen da, erwartet 1"
    fi

    # -------------------------------------- 4. harte Verweise statt Kopien
    local geteilt gespart
    geteilt=$($O verweise --wurzel "$T/w1" | tail -1)
    gespart=$(echo "$geteilt" | grep -o '[0-9]* Oktette' | head -1 | cut -d' ' -f1)
    if [[ "${gespart:-0}" -gt 200000 ]]; then
        ok "$geteilt"
    else
        nok "die Dateien unter apps/ sind KOPIEN, keine zweiten Namen: $geteilt"
    fi

    # --------------------------------------- 5. Generationen und zurueck
    local g="$T/w3"
    $O installieren --wurzel "$g" build/pakete/terminal.opkg >/dev/null 2>&1
    $O installieren --wurzel "$g" build/pakete/explorer.opkg >/dev/null 2>&1
    $O baum --wurzel "$g" --ohne system --ohne store --ohne users > "$T/gen2"
    $O installieren --wurzel "$g" build/pakete/widgets.opkg >/dev/null 2>&1
    $O entfernen --wurzel "$g" explorer >/dev/null 2>&1
    $O baum --wurzel "$g" --ohne system --ohne store --ohne users > "$T/gen4"
    if cmp -s "$T/gen2" "$T/gen4"; then
        nok "die Generationen 2 und 4 sehen gleich aus — der Rueckrollnachweis misst nichts"
    else
        ok "Generation 4 unterscheidet sich von 2 (explorer weg, widgets da)"
    fi
    $O zurueck --wurzel "$g" 2 >/dev/null 2>&1
    $O baum --wurzel "$g" --ohne system --ohne store --ohne users > "$T/gen2b"
    local gz; gz=$(wc -l < "$T/gen2")
    if cmp -s "$T/gen2" "$T/gen2b" && [[ "$gz" -ge 10 ]]; then
        ok "zurueck auf Generation 2: apps/ ist wieder Eintrag fuer Eintrag der alte Stand ($gz Eintraege)"
    else
        nok "das Zurueckrollen hat den alten Stand nicht wiederhergestellt ($gz Eintraege verglichen)"
        diff "$T/gen2" "$T/gen2b" | head -6 | sed 's/^/         /'
    fi
    # Der Store haelt weiter jeden Eintrag, den IRGENDEINE Generation
    # nennt -- ohne das waere ein Rueckrollen nicht moeglich.
    local se; se=$(ls "$g/store" | wc -l)
    [[ "$se" -eq 3 ]] \
        && ok "der Store haelt alle 3 Eintraege, auch den gerade nicht benutzten" \
        || nok "der Store hat $se Eintraege, erwartet 3"

    # ------------------------------------------------ 6. Abhaengigkeiten
    #
    # In diesem Userland gibt es noch KEINE echte Abhaengigkeit: jedes
    # Programm aus Osum ist statisch gebunden und braucht kein zweites.
    # Gemessen wird deshalb an zwei eigens gebauten Paketen -- und das
    # steht hier, damit niemand die Zusage fuer mehr haelt, als sie ist.
    # ABSOLUTE Pfade: ein `datei=`-Eintrag ist relativ zum Rezept, und
    # diese Rezepte liegen nicht im Repo, sondern in einem Wegwerfordner.
    mkdir -p "$T/rez"
    local repo; repo=$(pwd)
    printf 'name=unten\nfassung=1.0.0\ntitel=Unten\ndatei=start %s/vendor/osum/bin/true\n' "$repo" \
        > "$T/rez/unten.rezept"
    printf 'name=oben\nfassung=1.0.0\ntitel=Oben\nbraucht=unten\ndatei=start %s/vendor/osum/bin/false\n' "$repo" \
        > "$T/rez/oben.rezept"
    $O bauen "$T/rez/unten.rezept" -o "$T/unten.opkg" >"$T/aus" 2>&1 \
        && ok "das Hilfspaket 'unten' laesst sich bauen ($(stat -c%s "$T/unten.opkg" 2>/dev/null || echo 0) Oktette)" \
        || nok "das Hilfspaket 'unten' laesst sich nicht bauen: $(head -1 "$T/aus")"
    $O bauen "$T/rez/oben.rezept" -o "$T/oben.opkg" >/dev/null 2>&1
    local d="$T/w4"
    if $O installieren --wurzel "$d" "$T/oben.opkg" >"$T/aus" 2>&1; then
        nok "'oben' liess sich ohne 'unten' installieren — die Abhaengigkeit wird nicht geprueft"
    else
        ok "ohne die Abhaengigkeit abgelehnt: $(grep -m1 braucht "$T/aus" | cut -c1-64)"
    fi
    $O installieren --wurzel "$d" "$T/unten.opkg" >/dev/null 2>&1
    if $O installieren --wurzel "$d" "$T/oben.opkg" >"$T/aus" 2>&1; then
        ok "mit der Abhaengigkeit geht es — die Pruefung lehnt nicht grundsaetzlich ab"
    else
        nok "'oben' geht auch MIT 'unten' nicht: $(head -1 "$T/aus")"
    fi
    if $O entfernen --wurzel "$d" unten >"$T/aus" 2>&1; then
        nok "'unten' liess sich entfernen, obwohl 'oben' es braucht"
    else
        ok "'unten' laesst sich nicht entfernen, solange 'oben' es braucht"
    fi

    # ----------------------------------------- 7. die Quelle und ihre Signatur
    if [[ -f build/quelle/INDEX && -f build/quelle/INDEX.sig ]]; then
        local eigen fremd
        read -r eigen fremd < <(python3 - <<'PY'
import sys
sys.path.insert(0, 'pkg')
import opkg
idx = open('build/quelle/INDEX', 'rb').read()
sig = open('build/quelle/INDEX.sig', 'rb').read()
pk = open('build/quelle/oeffentlich.key', 'rb').read()
print(opkg.ed25519_verify(pk, idx, sig),
      opkg.ed25519_verify_bibliothek(pk, idx, sig))
PY
)
        [[ "$eigen" == "True" ]] \
            && ok "die Signatur des Index geht durch die EIGENE Ed25519-Umsetzung" \
            || nok "die eigene Ed25519-Umsetzung erkennt die eigene Signatur nicht"
        if [[ "$fremd" == "True" ]]; then
            ok "und durch die Bibliothek des Wirts (cryptography) — zwei Umsetzungen, ein Ergebnis"
        elif [[ "$fremd" == "None" ]]; then
            nok "auf diesem Wirt gibt es keine zweite Ed25519-Umsetzung — die Signatur ist nur von EINEM Programm bestaetigt"
        else
            nok "die Bibliothek des Wirts erkennt die Signatur NICHT an (eigen=$eigen) — eine der beiden Umsetzungen irrt"
        fi
        # GEGENPROBE: ein veraenderter Index darf NICHT durchgehen.
        rm -rf "$T/q"; cp -r build/quelle "$T/q"
        printf 'schadsoftware\t9.9.9\t%064d\t1\tboese.opkg\n' 0 >> "$T/q/INDEX"
        if $O installieren --wurzel "$T/w5" explorer --quelle "$T/q" >"$T/aus" 2>&1; then
            nok "eine Quelle mit veraendertem Index wurde benutzt"
        else
            ok "veraenderter Index -> abgelehnt: $(grep -m1 Signatur "$T/aus" | cut -c1-60)"
        fi
        # ...und die unveraenderte Quelle MUSS gehen.
        if $O installieren --wurzel "$T/w6" explorer --quelle build/quelle >"$T/aus" 2>&1; then
            ok "aus der unveraenderten Quelle laesst sich installieren"
        else
            nok "auch die unveraenderte Quelle geht nicht: $(head -2 "$T/aus" | tr '\n' ' ')"
        fi
        # GEGENPROBE ZUM INDEX: ein Paket, dessen Oktette nicht zu dem
        # passen, was der signierte Index nennt.
        rm -rf "$T/q2"; cp -r build/quelle "$T/q2"
        python3 -c "
import sys
p = sys.argv[1] + '/explorer.opkg'
b = bytearray(open(p, 'rb').read())
b[70000] ^= 1
open(p, 'wb').write(bytes(b))" "$T/q2"
        if $O installieren --wurzel "$T/w7" explorer --quelle "$T/q2" >"$T/aus" 2>&1; then
            nok "ein Paket, das nicht zum signierten Index passt, wurde installiert"
        else
            ok "Paket passt nicht zum signierten Index -> abgelehnt"
        fi
    else
        nok "build/quelle/INDEX oder INDEX.sig fehlt — die Quelle ist nicht gebaut"
    fi

    # ------------------------------- 8. der Nachrechner auf dem Store
    #
    # Als root halten Modusbits niemanden auf; der Hash schon. Deshalb
    # ist DAS die Pruefung auf Unversehrtheit und nicht `chmod`.
    $O pruefen --wurzel "$T/w1" >"$T/aus" 2>&1 \
        && ok "pruefen: $(tail -1 "$T/aus")" \
        || nok "pruefen meldet einen Fehler an einem unberuehrten Store: $(tail -1 "$T/aus")"
    local ziel
    ziel=$(find "$T/w1/store" -name INFO | head -1)
    if [[ -n "$ziel" ]]; then
        printf 'x' >> "$ziel"
        if $O pruefen --wurzel "$T/w1" >"$T/aus" 2>&1; then
            nok "ein veraenderter Store-Eintrag faellt NICHT auf — die Pruefung ist wirkungslos"
        else
            ok "GEGENPROBE: ein Oktett mehr im Store -> $(grep -m1 KAPUTT "$T/aus" | cut -c1-64)"
        fi
    else
        nok "im Store liegt keine INFO — die Gegenprobe zur Unversehrtheit misst nichts"
    fi

    # --------------------------------- 9. und das alles im PRODUKT-Abbild
    local img="build/${SLUG}-userland.img"
    if [[ -f "$img" ]]; then
        local liste="$T/img.txt"
        python3 vendor/osum/mkfs.py list "$img" > "$liste"
        local na ns
        na=$(grep -c '^/apps/[^/]*\.prog/$' "$liste")
        ns=$(grep -c '^/store/[^/]*/$' "$liste")
        [[ "$na" -ge 3 && "$ns" -ge 3 ]] \
            && ok "im Produkt-Abbild liegen $na Buendel unter /apps und $ns Eintraege unter /store" \
            || nok "im Produkt-Abbild sind es $na Buendel und $ns Store-Eintraege, erwartet je mindestens 3"
        # Die Oktette des Explorers im Abbild gegen die im Paket.
        local imabbild impaket
        imabbild=$(awk '$1=="/apps/explorer.prog/start"{print $2}' "$liste")
        impaket=$(stat -c%s vendor/osum/bin/explorer)
        [[ "$imabbild" == "$impaket" ]] \
            && ok "/apps/explorer.prog/start ist $imabbild Oktette gross — dieselbe Zahl wie vendor/osum/bin/explorer" \
            || nok "/apps/explorer.prog/start ist $imabbild statt $impaket Oktette"
        # UND ER LIEGT NUR EINMAL DA. Waeren Store und Buendel zwei
        # Exemplare, kostete allein der Explorer 458 Bloecke doppelt.
        local frei
        frei=$(awk -F'[= ]' '/^blocks=/{print $4}' "$liste")
        local noetig=$(( (imabbild + 511) / 512 ))
        if [[ "$frei" -gt "$noetig" ]]; then
            ok "$frei Bloecke frei — mehr als die $noetig, die ein zweites Exemplar des Explorers kosten wuerde"
        else
            nok "nur $frei Bloecke frei; ein zweites Exemplar des Explorers ($noetig Bloecke) waere nicht aufgefallen"
        fi
        # Die Inodezahl, aus dem SUPERBLOCK gelesen und nicht aus einer
        # Konstanten -- das ist die Stelle, die 0003-mkfs-load-inodezahl
        # berichtigt.
        local ino
        ino=$(python3 -c "
import struct,sys
print(struct.unpack_from('<Q', open(sys.argv[1],'rb').read(), 24)[0])" "$img")
        [[ "$ino" -gt 128 ]] \
            && ok "die Inodetabelle des Abbilds hat $ino Plaetze (128 haetten fuer einen Paketbaum nicht gereicht)" \
            || nok "die Inodetabelle hat nur $ino Plaetze"
    else
        nok "$img fehlt — ./build.sh ist nicht durchgelaufen"
    fi

    rm -rf "$T"
    return $RC
}
run pakete_check

# ---------------------------------------------------------------------------
step "Pakete im laufenden System: /apps, /store und ein Paket, das WIRKLICH laeuft"
pakete_boot() {
    RC=0
    local log; log=$(mktemp "${TMPDIR:-/tmp}/orientos-pkgboot-XXXXXX.log")
    # DER EIGENTLICHE NACHWEIS DIESER RUNDE. Alles davor laeuft auf dem
    # Wirt; hier startet das Produkt, und die Frage ist, ob aus dem
    # installierten Paket auf der gebauten Platte ein PROZESS wird.
    #
    # `hallo` ist dafuer da: der Explorer und die Widget-Anwendung
    # brauchen einen Bildschirm, `hallo` schreibt eine Zeile und beendet
    # sich. Ein Paket, dessen Oktette man nur zaehlen kann, ist installiert;
    # eines, das laeuft, ist es nachweislich.
    if ! ./run-osum.sh --script 'ls /apps;ls /store;cat /system/AKTUELL;/apps/hallo.prog/start;wc -c /apps/explorer.prog/start;exit' \
            --log "$log" --timeout 180 >/dev/null 2>&1; then
        nok "der Lauf mit dem Paketbaum ist fehlgeschlagen"
        rm -f "$log"; return 1
    fi
    ok "der Kernel hat sich sauber beendet (21)"
    grep -aq 'explorer.prog/' "$log" \
        && ok "/apps enthaelt explorer.prog" \
        || nok "/apps enthaelt kein explorer.prog"
    grep -aq 'hallo.prog/' "$log" \
        && ok "/apps enthaelt hallo.prog" \
        || nok "/apps enthaelt kein hallo.prog"
    local ns
    # Zwischen dem Befehl und seiner Ausgabe stehen die Zeilen des
    # ELF-Laders -- deshalb mehr als eine Folgezeile.
    ns=$(grep -a -A8 'osum\$ ls /store' "$log" | grep -ao '[0-9a-f]\{20\}/' | wc -l)
    [[ "$ns" -ge 3 ]] \
        && ok "/store enthaelt $ns Eintraege mit hexadezimalem Namen" \
        || nok "/store enthaelt $ns Eintraege, erwartet mindestens 3"
    # DAS PAKET LAEUFT.
    if grep -aq '^hello: argc' "$log"; then
        ok "/apps/hallo.prog/start ist gestartet und hat geschrieben: $(grep -a -m1 '^hello: arg 0' "$log")"
    else
        nok "/apps/hallo.prog/start hat nichts geschrieben — das Paket ist da, laeuft aber nicht"
    fi
    # ...und die Oktette stimmen, im Gastsystem gemessen.
    local n
    n=$(grep -a -m1 '^[0-9]* /apps/explorer.prog/start' "$log" | cut -d' ' -f1)
    if [[ "$n" == "$(stat -c%s vendor/osum/bin/explorer)" ]]; then
        ok "das Gastsystem zaehlt $n Oktette in /apps/explorer.prog/start — dieselbe Zahl wie auf dem Wirt"
    else
        nok "das Gastsystem zaehlt '$n' Oktette, der Wirt $(stat -c%s vendor/osum/bin/explorer)"
    fi
    rm -f "$log"
    return $RC
}
run pakete_boot
