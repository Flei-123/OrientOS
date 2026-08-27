# tests/step-91-look.sh -- wird von test.sh gesourct, nicht direkt gestartet.
#
# "WENN ICH AUF EINEM NEUEN GERAET NEU LADE -- SCHAUT ES DANN GLEICH AUS?"
#
# Das ist die Frage, die diesen Schritt begruendet, und bis zum Nachtrag
# zur Runde PLAN2 war die Antwort NEIN. Osums Einstellungen schreiben
# /etc/theme, /etc/schemas/, /etc/hintergrund, /etc/schirm.conf,
# /etc/zeit.conf und /etc/netz.conf -- und nichts davon stand in einem
# PLAN. Ein nachgebautes Geraet hatte die Programme des alten und das
# Aussehen einer frischen Installation.
#
# ZWEI DINGE WERDEN HIER GEMESSEN, und das zweite ist das unbequemere.
#
#   1. DASS ES GLEICH AUSSIEHT. Ein System wird eingerichtet -- anderes
#      Farbschema, anderes Hintergrundbild, Taskleiste an einem anderen
#      Rand, andere Zeitzone, andere Aufloesung --, der PLAN gezogen und
#      auf einer LEEREN Wurzel nachgebaut. Verglichen wird mit der
#      SHA-256 jedes Inhalts, und die Zahl der verglichenen Eintraege
#      steht in der Zusage: ein Vergleich zweier leerer Baeume geht immer
#      durch.
#
#   2. DASS DIE EBENE STIMMT. Farbschema, Hintergrundbild und Taskleiste
#      sind BENUTZER-Einstellungen. `/etc/theme` ist deshalb nicht ein
#      kleiner Schoenheitsfehler, sondern eine Stelle, an der ein ZWEITER
#      BENUTZER NICHT EXISTIEREN KANN. Gemessen wird genau das: zwei
#      Konten, zwei Farbschemata, zwei Taskleisten -- und die
#      Vertraeglichkeitssicht unter /etc verschwindet, weil es auf die
#      Frage "wessen Schema ist /etc/theme" keine ehrliche Antwort gibt.
step "Aussehen: Farbschema, Bild und Taskleiste im PLAN -- und ein Baum, der wieder gleich aussieht"
look_check() {
    RC=0
    local O="python3 pkg/opk.py"
    local T; T=$(mktemp -d "${TMPDIR:-/tmp}/orientos-look-XXXXXX")
    local repo; repo=$(pwd)

    if [[ ! -f build/pakete/dusk.opk || ! -f build/pakete/deep.opk ]]; then
        ./pkg/bauen.sh >/dev/null 2>&1
    fi
    if [[ ! -f build/pakete/dusk.opk ]]; then
        nok "build/pakete/dusk.opk fehlt -- die Farbschemata sind nicht paketiert"
        rm -rf "$T"; return 1
    fi

    # ------------------------------------ 0. das Bild entsteht aus TEXT
    #
    # Ein Klumpen Oktette im Baum ist nichts, was jemand in einem
    # Unterschied lesen kann. Vier Zeilen Text sind es. Damit der Hash
    # im PLAN etwas wert ist, muss aus denselben vier Zeilen IMMER
    # dasselbe Bild werden.
    python3 pkg/osym.py userland/wallpapers/deep.wallpaper "$T/a.osym" >/dev/null 2>&1
    python3 pkg/osym.py userland/wallpapers/deep.wallpaper "$T/b.osym" >/dev/null 2>&1
    local o1 o2
    o1=$(sha256sum "$T/a.osym" | cut -d' ' -f1)
    o2=$(sha256sum "$T/b.osym" | cut -d' ' -f1)
    if [[ "$o1" == "$o2" && -s "$T/a.osym" ]]; then
        ok "aus vier Zeilen Text wird zweimal dasselbe Bild ($(stat -c%s "$T/a.osym") Oktette, OSYM)"
    else
        nok "derselbe Text ergibt zwei verschiedene Bilder -- der Hash im PLAN taugt nichts"
    fi
    head -c4 "$T/a.osym" | grep -q OSYM \
        && ok "es faengt mit OSYM an -- das Format, das kernel/user/schreibtisch.fi liest" \
        || nok "das erzeugte Bild ist kein OSYM"
    # GEGENPROBE: die Grenze des Schreibtischs (240x180) wird nicht
    # ueberschritten, sondern abgelehnt. Ein Bild, das nicht geladen
    # werden kann, ist schlimmer als eine Fehlermeldung.
    printf 'width=800\nheight=600\ntop=000000\nbottom=ffffff\n' > "$T/zugross.wallpaper"
    if python3 pkg/osym.py "$T/zugross.wallpaper" "$T/c.osym" >"$T/aus" 2>&1; then
        nok "ein Bild von 800x600 wurde erzeugt -- der Schreibtisch kann es nicht laden"
    else
        ok "GEGENPROBE: 800x600 wird abgelehnt: $(grep -m1 -o 'loads at most.*' "$T/aus" | cut -c1-42)"
    fi

    # ------------------------- 1. die Farbschemata haben Osums Schluessel
    #
    # `wlibc.theme_load` kennt achtzehn Namen (`theme_names()` in
    # kernel/user/wlibc.fi) und zaehlt jeden anderen als FEHLERHAFT. Ein
    # Schema mit einem erfundenen Schluessel waere also kein Schema.
    local erwartet="accent bg btn btndn btnhi dim dlg entry fg focus head line menu panel scroll sel selfg thumb"
    local th ist fehlt=0
    for th in userland/themes/*.theme; do
        ist=$(sed 's/#.*//' "$th" | grep -o '^[a-z]*=' | tr -d '=' | sort | tr '\n' ' ' | sed 's/ $//')
        [[ "$ist" == "$erwartet" ]] || fehlt=$((fehlt + 1))
    done
    local nth; nth=$(ls userland/themes/*.theme | wc -l)
    if [[ "$fehlt" -eq 0 && "$nth" -ge 2 ]]; then
        ok "$nth Farbschemata, jedes mit genau den 18 Schluesseln aus wlibc.theme_names()"
    else
        nok "$fehlt von $nth Schemata haben nicht die 18 Schluessel, die wlibc kennt"
    fi
    # GEGENPROBE: der Vergleich oben faende einen Fehler auch wirklich.
    printf 'bg=112233\nlieblingsfarbe=445566\n' > "$T/kaputt.theme"
    ist=$(sed 's/#.*//' "$T/kaputt.theme" | grep -o '^[a-z]*=' | tr -d '=' | sort | tr '\n' ' ' | sed 's/ $//')
    [[ "$ist" != "$erwartet" ]] \
        && ok "GEGENPROBE: ein Schema mit erfundenem Schluessel faellt bei derselben Pruefung durch" \
        || nok "die Pruefung der Schluessel merkt nichts"

    # ================================================================
    # 2. EIN SYSTEM MIT EINEM GESICHT
    # ================================================================
    mkdir -p "$T/src"
    cp build/pakete/hallo.opk build/pakete/ls.opk build/pakete/osum.opk \
       build/pakete/dusk.opk build/pakete/dawn.opk \
       build/pakete/deep.opk build/pakete/sand.opk "$T/src/" 2>/dev/null
    $O schluessel "$T/key" >/dev/null 2>&1
    $O quelle "$T/src" --schluessel "$T/key/geheim.key" >/dev/null 2>&1
    local PUB; PUB=$(python3 -c "print(open('$T/key/oeffentlich.key','rb').read().hex())")
    local A="$T/A"
    $O source-add --root "$A" "file://$T/src" "$PUB" >/dev/null 2>&1
    local p
    for p in hallo ls; do
        $O installieren --root "$A" --quelle "$T/src" "$p" >>"$T/aus" 2>&1
    done
    $O kernel --root "$A" build/pakete/osum.opk >/dev/null 2>&1
    $O set --root "$A" display.mode 1024x768 >/dev/null 2>&1
    $O set --root "$A" time.offset 7200 >/dev/null 2>&1
    $O set --root "$A" timezone Europe/Vienna >/dev/null 2>&1
    $O account-add --root "$A" justin >/dev/null 2>&1
    $O pref --root "$A" justin theme build/pakete/dusk.opk >/dev/null 2>&1
    $O pref --root "$A" justin wallpaper build/pakete/deep.opk >/dev/null 2>&1
    $O pref --root "$A" justin taskbar.edge left >/dev/null 2>&1
    $O pref --root "$A" justin taskbar.height 40 >/dev/null 2>&1
    $O pref --root "$A" justin taskbar.autohide yes >/dev/null 2>&1

    # DAS BILD LIEGT EINMAL DA. Es hat drei Namen -- im Store, in der
    # Konfiguration des Benutzers und in der Vertraeglichkeitssicht --
    # und ist ein einziger Inode. Bei 172 812 Oktetten auf einer Platte
    # von zwei Megaoktett ist das kein Feinschliff.
    local i1 i2 i3 wh
    wh=$(awk -F'\t' '$1=="pref" && $3=="wallpaper"{print $4}' \
         "$A/system/generations/$(cat "$A/system/AKTUELL")/PLAN")
    i1=$(stat -c%i "$A/store/${wh:0:20}/blob" 2>/dev/null || echo x)
    i2=$(stat -c%i "$A/users/justin/config/desktop/wallpaper" 2>/dev/null || echo y)
    i3=$(stat -c%i "$A/etc/hintergrund" 2>/dev/null || echo z)
    if [[ "$i1" == "$i2" && "$i2" == "$i3" ]]; then
        ok "das Hintergrundbild hat drei Namen und einen Inode ($i1, $(stat -c%s "$A/etc/hintergrund") Oktette)"
    else
        nok "das Hintergrundbild liegt mehrfach da (store=$i1 config=$i2 etc=$i3)"
    fi
    # DAS BILD STEHT NICHT IM PLAN -- nur sein Hash. Ein PLAN bleibt Text.
    $O export --root "$A" -o "$T/look.txt" >/dev/null 2>&1
    if LC_ALL=C grep -q 'OSYM' "$T/look.txt"; then
        nok "im PLAN stehen Bildoktette -- er ist kein Text mehr"
    else
        ok "im PLAN steht kein Bild, nur sein Hash ($(wc -c < "$T/look.txt") Oktette, $(wc -l < "$T/look.txt") Zeilen, alles druckbar)"
    fi
    grep -q "$wh" "$T/look.txt" \
        && ok "GEGENPROBE: die SHA-256 des Bildes steht sehr wohl darin (${wh:0:16})" \
        || nok "auch der Hash des Bildes fehlt -- das Bild waere nicht nachbaubar"
    LC_ALL=C sort -c "$T/look.txt" 2>/dev/null \
        && ok "der PLAN mit pref-Zeilen ist weiterhin sortiert (von aussen mit 'sort -c' geprueft)" \
        || nok "die neuen pref-Zeilen brechen die kanonische Reihenfolge"

    # ------------------------------- 3. ein unmoeglicher Wert faellt um
    if $O pref --root "$A" justin taskbar.edge schraeg >"$T/aus" 2>&1; then
        nok "taskbar.edge=schraeg wurde angenommen"
    else
        ok "ein Rand, den es nicht gibt, wird abgelehnt: $(grep -m1 -o 'must be one of.*' "$T/aus" | cut -c1-40)"
    fi
    $O pref --root "$A" justin taskbar.edge right >/dev/null 2>&1 \
        && ok "GEGENPROBE: 'right' geht -- es sind genau die vier Namen aus wlibc.fi und nicht alles verboten" \
        || nok "auch ein gueltiger Rand wird abgelehnt"
    $O pref --root "$A" justin taskbar.edge left >/dev/null 2>&1
    # Ein Hintergrundbild ist kein Farbschema, auch wenn beides ein
    # Asset ist. Die Klasse steht im Paket und wird geprueft.
    if $O pref --root "$A" justin theme build/pakete/deep.opk >"$T/aus" 2>&1; then
        nok "ein Hintergrundbild liess sich als Farbschema setzen"
    else
        ok "ein 'wallpaper'-Asset wird als 'theme' abgelehnt: $(grep -m1 -o 'is not a colour scheme' "$T/aus")"
    fi

    # ================================================================
    # 4. DIE MESSUNG: sieht es nach dem Nachbau gleich aus?
    # ================================================================
    local B="$T/B"
    $O rebuild --root "$B" --plan "$T/look.txt" > "$T/reb" 2>&1 \
        && ok "aus dem PLAN entsteht ein Baum von null: $(grep -m1 'package(s) fetched' "$T/reb" | xargs)" \
        || nok "der Nachbau ist fehlgeschlagen: $(tail -2 "$T/reb" | tr '\n' ' ')"
    $O snapshot --root "$A" > "$T/sA"
    $O snapshot --root "$B" > "$T/sB"
    local eintraege dateien oktette
    eintraege=$(awk '/^count /{print $2}' "$T/sA" | cut -d= -f2)
    dateien=$(awk '/^count /{print $3}' "$T/sA" | cut -d= -f2)
    oktette=$(awk '/^count /{print $4}' "$T/sA" | cut -d= -f2)
    if [[ "${eintraege:-0}" -ge 40 && "${oktette:-0}" -ge 1000000 ]]; then
        ok "verglichen werden $eintraege Eintraege, $dateien Dateien, $oktette Oktette -- nicht zwei leere Baeume"
    else
        nok "es waeren nur $eintraege Eintraege / $oktette Oktette verglichen worden"
    fi
    if cmp -s "$T/sA" "$T/sB"; then
        ok "DAS NACHGEBAUTE SYSTEM SIEHT GLEICH AUS: $eintraege Eintraege Eintrag fuer Eintrag identisch (Schema, Bild, Taskleiste, Zeitzone, Aufloesung)"
    else
        nok "der Nachbau sieht anders aus:"
        diff "$T/sA" "$T/sB" | head -10 | sed 's/^/         /'
    fi
    # Und die Dateien, um die es geht, sind wirklich da und wirklich gleich.
    local f gleich=0 anzahl=0
    for f in etc/theme etc/hintergrund etc/taskbar.conf etc/schirm.conf \
             etc/zeit.conf users/justin/config/desktop/theme \
             users/justin/config/desktop/wallpaper \
             users/justin/config/desktop/taskbar.conf; do
        anzahl=$((anzahl + 1))
        cmp -s "$A/$f" "$B/$f" && gleich=$((gleich + 1))
    done
    [[ "$gleich" -eq "$anzahl" ]] \
        && ok "die $anzahl Dateien, die das Aussehen ausmachen, sind einzeln Oktett fuer Oktett gleich" \
        || nok "nur $gleich von $anzahl Aussehens-Dateien sind gleich"

    # GEGENPROBE ZUM VERGLEICH: EINE geaenderte Einstellung MUSS auffallen.
    $O pref --root "$B" justin taskbar.edge top >/dev/null 2>&1
    $O snapshot --root "$B" > "$T/sB2"
    diff "$T/sA" "$T/sB2" > "$T/dAB2" || true
    if [[ "$(grep -c '^[<>]' "$T/dAB2")" -ge 2 ]]; then
        ok "GEGENPROBE: EIN anderer Taskleistenrand, und der Vergleich faellt um ($(grep -c '^[<>]' "$T/dAB2") Zeilen)"
    else
        nok "ein anderer Taskleistenrand faellt beim Vergleich nicht auf -- er misst nichts"
    fi

    # ================================================================
    # 5. DIE EBENE: zwei Benutzer, zwei Gesichter
    # ================================================================
    local C="$T/C"
    cp -a "$A" "$C"
    $O account-add --root "$C" anna --uid 1001 --gid 1001 >/dev/null 2>&1
    $O pref --root "$C" anna theme build/pakete/dawn.opk >/dev/null 2>&1
    $O pref --root "$C" anna wallpaper build/pakete/sand.opk >/dev/null 2>&1
    $O pref --root "$C" anna taskbar.edge top >/dev/null 2>&1
    if cmp -s "$C/users/justin/config/desktop/theme" \
              "$C/users/anna/config/desktop/theme"; then
        nok "beide Benutzer haben dasselbe Farbschema -- die Trennung wirkt nicht"
    else
        ok "zwei Benutzer, zwei verschiedene Farbschemata ($(sha256sum "$C/users/justin/config/desktop/theme" | cut -c1-8) / $(sha256sum "$C/users/anna/config/desktop/theme" | cut -c1-8))"
    fi
    if cmp -s "$C/users/justin/config/desktop/taskbar.conf" \
              "$C/users/anna/config/desktop/taskbar.conf"; then
        nok "beide Benutzer haben dieselbe Taskleiste"
    else
        ok "und zwei verschiedene Taskleisten ($(tr '\n' ' ' < "$C/users/justin/config/desktop/taskbar.conf")| $(tr '\n' ' ' < "$C/users/anna/config/desktop/taskbar.conf"))"
    fi
    # DAS IST DER PUNKT: mit zwei Konten gibt es KEIN /etc/theme mehr,
    # weil es keine ehrliche Antwort auf "wessen" gaebe. Der Fehler wird
    # sichtbar, statt willkuerlich entschieden zu werden.
    if [[ -e "$C/etc/theme" || -e "$C/etc/hintergrund" || -e "$C/etc/taskbar.conf" ]]; then
        nok "bei zwei Konten steht immer noch ein /etc/theme da -- wessen?"
    else
        ok "bei ZWEI Konten verschwindet die Vertraeglichkeitssicht unter /etc -- es gibt kein 'wessen Schema' mehr"
    fi
    # ...und die Gegenprobe: bei EINEM Konto ist sie da. Sonst waere
    # „loescht immer alles" hier gruen.
    [[ -f "$A/etc/theme" && -f "$A/etc/hintergrund" && -f "$A/etc/taskbar.conf" ]] \
        && ok "GEGENPROBE: bei EINEM Konto liegen etc/theme, etc/hintergrund und etc/taskbar.conf da -- das lesen Osums Taskleiste und Schreibtisch heute" \
        || nok "auch bei einem Konto fehlt die Vertraeglichkeitssicht"
    # /etc/schemas ist die Liste, die Osums Einstellungen anzeigen.
    local nsch; nsch=$(ls "$C/etc/schemas" 2>/dev/null | wc -l)
    [[ "$nsch" -eq 2 ]] \
        && ok "etc/schemas nennt beide benutzten Schemata ($(ls "$C/etc/schemas" | tr '\n' ' ')) -- das ist die Liste, die einstellungen.fi anzeigt" \
        || nok "etc/schemas hat $nsch Eintraege, erwartet 2"

    # -------------------------------- 6. eine Einstellung wieder wegnehmen
    $O pref-unset --root "$C" anna wallpaper >/dev/null 2>&1
    if [[ -e "$C/users/anna/config/desktop/wallpaper" ]]; then
        nok "das Hintergrundbild bleibt liegen, nachdem die Einstellung weg ist"
    else
        ok "'pref-unset' nimmt users/anna/config/desktop/wallpaper wieder aus dem Baum"
    fi
    [[ -f "$C/users/justin/config/desktop/wallpaper" ]] \
        && ok "GEGENPROBE: das Bild des ANDEREN Benutzers bleibt unberuehrt" \
        || nok "das Bild des anderen Benutzers ist mitgeloescht worden"
    # Und die Nutzerdokumente daneben ruehrt niemand an -- `config/` ist
    # der Topf des Menschen, wir besitzen darin nur eine benannte Liste.
    mkdir -p "$C/users/justin/config/eigenes"
    echo "gehoert dem menschen" > "$C/users/justin/config/eigenes/notiz"
    $O pref --root "$C" justin taskbar.height 32 >/dev/null 2>&1
    [[ -f "$C/users/justin/config/eigenes/notiz" ]] \
        && ok "eine eigene Datei unter config/ ueberlebt das Aktivieren -- erzeugt wird nur eine benannte Liste" \
        || nok "das Aktivieren hat eine fremde Datei unter config/ geloescht"

    # ------------------------------------------------------- 7. verify
    $O verify --root "$C" > "$T/ver" 2>&1 \
        && ok "verify auf dem Zwei-Benutzer-Baum: $(tail -1 "$T/ver")" \
        || nok "verify meldet einen Fehler: $(grep -m1 FAILED "$T/ver")"
    # GEGENPROBE: ein von Hand veraendertes Farbschema faellt auf, weil
    # der harte Verweis dann keiner mehr ist.
    rm -f "$C/users/anna/config/desktop/theme"
    printf 'bg=000000\n' > "$C/users/anna/config/desktop/theme"
    if $O verify --root "$C" >"$T/ver2" 2>&1; then
        nok "ein ausgetauschtes Farbschema faellt bei verify nicht auf"
    else
        ok "GEGENPROBE: ein ausgetauschtes Farbschema -> $(grep -m1 'FAILED.*SAME INODE' "$T/ver2" | cut -c1-58)"
    fi

    rm -rf "$T"
    return $RC
}
run look_check
