# tests/step-90-plan.sh -- sourced by test.sh, not started directly.
#
# THE SELF-BEARING PLAN (round PLAN2, format in docs/PLAN-FORMAT.md).
#
# Until this round a PLAN was `name<TAB>hash` and described the installed
# APPLICATIONS -- nothing else. A machine cannot be rebuilt from that: it
# does not say which kernel the applications run on, it does not say where
# the hashes may be fetched from, and it does not say what the machine is
# configured to be. This step measures whether that is fixed, and the
# measurement it lives or dies by is the last one:
#
#   A tree is built. Its PLAN is exported as ONE text file. On an empty
#   directory, that file plus a package source is turned back into a tree.
#   The two trees are then compared with the SHA-256 of every content --
#   and the comparison says HOW MANY entries and HOW MANY octets it
#   compared, because a comparison of two empty trees always passes.
#
# Every assertion here has its counter-check: the same run with the
# property switched off, where the measurement must collapse. Three of
# them are counter-checks to counter-checks -- that an INTACT plan is
# accepted, that a KNOWN setting is taken, that a non-reserved name
# builds. Without those, a tool that refuses everything would be green.
step "Plan v2: typed lines, kernel in the generation, and a tree rebuilt from ONE text file"
plan2_check() {
    RC=0
    local O="python3 pkg/opk.py"
    local T; T=$(mktemp -d "${TMPDIR:-/tmp}/orientos-plan2-XXXXXX")
    local repo; repo=$(pwd)

    if [[ ! -f build/pakete/hallo.opk || ! -f build/pakete/osum.opk ]]; then
        ./pkg/bauen.sh >/dev/null 2>&1
    fi
    if [[ ! -f build/pakete/osum.opk ]]; then
        nok "build/pakete/osum.opk fehlt -- der Kernel ist nicht paketiert"
        rm -rf "$T"; return 1
    fi

    # ------------------------------------------------------ 0. eine Quelle
    #
    # Eigene Quelle mit eigenem Schluessel, damit dieser Schritt nicht
    # davon abhaengt, wann `pkg/bauen.sh` zuletzt gelaufen ist: der
    # Schluessel dort entsteht bei jedem Bau neu.
    mkdir -p "$T/src"
    cp build/pakete/hallo.opk build/pakete/ls.opk build/pakete/wc.opk \
       build/pakete/osum.opk "$T/src/" 2>/dev/null
    $O schluessel "$T/key" >/dev/null 2>&1
    $O quelle "$T/src" --schluessel "$T/key/geheim.key" >/dev/null 2>&1
    local PUB; PUB=$(python3 -c "print(open('$T/key/oeffentlich.key','rb').read().hex())")
    local A="$T/A"

    # ------------------------------------------- 1. das erweiterte Format
    #
    # Eine alte PLAN-Datei ist reines `name<TAB>hash`. Sie MUSS weiter
    # gelesen werden -- sonst waere jede bestehende Generation eines jeden
    # Baumes mit diesem Werkzeug unlesbar geworden.
    mkdir -p "$T/alt/system/generations/0"
    printf 'hallo\t%064d\n' 7 > "$T/alt/system/generations/0/PLAN"
    echo "alt" > "$T/alt/system/generations/0/GRUND"
    echo 0 > "$T/alt/system/AKTUELL"
    if $O plan --root "$T/alt" > "$T/aus" 2>&1 \
       && grep -q 'OLD two-field shape' "$T/aus"; then
        ok "eine alte PLAN-Datei (name<TAB>hash) wird gelesen und als Anwendung gedeutet"
    else
        nok "eine alte PLAN-Datei laesst sich nicht mehr lesen: $(head -2 "$T/aus" | tr '\n' ' ')"
    fi
    # GEGENPROBE: eine Zeile, die WEDER getypt noch die alte Form ist,
    # darf nicht stillschweigend durchgehen.
    printf 'hallo\t%064d\tzuviel\n' 7 > "$T/alt/system/generations/0/PLAN"
    if $O plan --root "$T/alt" >"$T/aus" 2>&1; then
        nok "eine unlesbare PLAN-Zeile wurde klaglos angenommen"
    else
        ok "GEGENPROBE: eine Zeile, die weder getypt noch zweispaltig ist -> $(grep -m1 -o 'is not a known type.*' "$T/aus" | cut -c1-52)"
    fi

    # -------------------------------------- 2. der Baum, der nachgebaut wird
    $O source-add --root "$A" "file://$T/src" "$PUB" >/dev/null 2>&1
    local p
    for p in hallo ls wc; do
        $O installieren --root "$A" --quelle "$T/src" "$p" >>"$T/aus" 2>&1
    done
    $O kernel --root "$A" build/pakete/osum.opk >"$T/kern" 2>&1 \
        && ok "der Kernel ist ein Paket und wird zu einer Generation: $(grep -m1 origin "$T/kern" | xargs)" \
        || nok "der Kernel liess sich nicht setzen: $(head -1 "$T/kern")"
    # DER KERNEL LIEGT NUR EINMAL DA. `system/kernel/image` ist ein
    # ZWEITER NAME auf den Store-Eintrag, kein zweites Exemplar -- bei
    # 1,6 MiB Kernel auf einer Platte von 2 MiB ist das kein Feinschliff.
    local kh ki si
    kh=$(awk -F'\t' '$1=="kernel"{print $2}' "$A/system/generations/$(cat "$A/system/AKTUELL")/PLAN")
    ki=$(stat -c%i "$A/system/kernel/image" 2>/dev/null || echo x)
    si=$(stat -c%i "$A/store/${kh:0:20}/image" 2>/dev/null || echo y)
    [[ "$ki" == "$si" ]] \
        && ok "system/kernel/image und store/${kh:0:12}/image sind derselbe Inode ($ki, $(stat -c%s "$A/system/kernel/image") Oktette)" \
        || nok "system/kernel/image ist eine KOPIE des Store-Eintrags ($ki vs $si)"
    # GEGENPROBE: ein Kernelpaket ist KEINE Anwendung.
    if $O installieren --root "$T/w0" build/pakete/osum.opk >"$T/aus" 2>&1; then
        nok "ein Kernelpaket liess sich als Anwendung nach /apps installieren"
    else
        ok "GEGENPROBE: ein Kernelpaket wird von 'installieren' abgelehnt -- es gehoert der Generation"
    fi
    # ...und die Gegenprobe dazu: eine normale Anwendung geht sehr wohl.
    $O installieren --root "$T/w0" build/pakete/hallo.opk >/dev/null 2>&1 \
        && ok "dieselbe Wurzel nimmt eine gewoehnliche Anwendung an -- 'installieren' lehnt nicht grundsaetzlich ab" \
        || nok "auch eine gewoehnliche Anwendung wird abgelehnt"

    # ------------------------------------------------ 3. die Einstellungen
    $O set --root "$A" timezone Europe/Vienna >/dev/null 2>&1
    $O set --root "$A" time.offset 7200 >/dev/null 2>&1
    $O set --root "$A" hostname orient1 >/dev/null 2>&1
    $O set --root "$A" net.mode static >/dev/null 2>&1
    $O set --root "$A" net.address 192.168.1.50 >/dev/null 2>&1
    $O set --root "$A" net.netmask 255.255.255.0 >/dev/null 2>&1
    $O set --root "$A" net.gateway 192.168.1.1 >/dev/null 2>&1
    # `/etc/netz.conf` ist NICHT erfunden: `kernel/user/dhcp.fi` schreibt
    # und `kernel/user/einstellungen.fi` liest genau diese Datei, in
    # genau dieser Form. Eine Einstellung, die eine Datei erzeugt, die
    # niemand liest, waere Buchhaltung.
    if grep -q '^modus=fest$' "$A/etc/netz.conf" 2>/dev/null \
       && grep -q '^ip=192.168.1.50$' "$A/etc/netz.conf"; then
        ok "aus vier Einstellungen wird /etc/netz.conf in Osums Form ($(wc -c < "$A/etc/netz.conf") Oktette)"
    else
        nok "/etc/netz.conf ist nicht die Datei, die kernel/user/dhcp.fi schreibt"
    fi
    # GEGENPROBE: eine geloeschte Einstellung nimmt ihre Datei mit. Ohne
    # das waere der Baum keine FUNKTION des Plans, sondern seiner
    # Geschichte.
    $O set --root "$A" screen.mode 800x600 >/dev/null 2>&1
    if [[ -f "$A/etc/schirm.conf" ]]; then
        $O unset --root "$A" screen.mode >/dev/null 2>&1
        [[ ! -f "$A/etc/schirm.conf" ]] \
            && ok "GEGENPROBE: 'unset' nimmt /etc/schirm.conf wieder aus dem Baum" \
            || nok "/etc/schirm.conf bleibt stehen, nachdem die Einstellung weg ist"
    else
        nok "/etc/schirm.conf wurde gar nicht erst erzeugt -- die Gegenprobe misst nichts"
    fi
    # Der Schluesselraum ist GESCHLOSSEN. Ein offener waere die
    # Registrierdatenbank, die es hier nicht geben soll.
    if $O set --root "$A" lieblingsfarbe blau >"$T/aus" 2>&1; then
        nok "ein beliebiger Einstellungsschluessel wurde angenommen"
    else
        ok "GEGENPROBE: ein unbekannter Schluessel wird abgelehnt -- der Satz ist geschlossen"
    fi

    # ------------------------------------------------------ 4. die Konten
    $O account-add --root "$A" justin >/dev/null 2>&1
    printf '$osum1$2048$0011223344556677$%s' \
        "0011223344556677deadbeefcafebabe0011223344556677deadbeefcafebabe" > "$T/cred"
    $O secret-set --root "$A" justin "$T/cred" >"$T/aus" 2>&1 \
        && ok "das Kennwort ist eine Generation: $(grep -m1 -o 'sha256 [0-9a-f]*' "$T/aus")" \
        || nok "secret-set ist fehlgeschlagen: $(head -1 "$T/aus")"
    # DER PUNKT, um den es bei den Konten geht: der PLAN darf weitergegeben
    # werden, also darf das Kennwort nicht darin stehen.
    $O export --root "$A" -o "$T/plan.txt" >/dev/null 2>&1
    if grep -q 'deadbeefcafebabe' "$T/plan.txt"; then
        nok "die Zugangsdaten stehen im PLAN -- er duerfte nicht weitergegeben werden"
    else
        ok "im PLAN steht kein Kennwort ($(wc -c < "$T/plan.txt") Oktette, $(wc -l < "$T/plan.txt") Zeilen)"
    fi
    # ...und die Gegenprobe: die SHA-256 des Zugangsdatums steht sehr wohl
    # darin, sonst waere ein Kennwortwechsel keine Generation.
    local sh256; sh256=$(sha256sum "$T/cred" | cut -d' ' -f1)
    grep -q "$sh256" "$T/plan.txt" \
        && ok "GEGENPROBE: die SHA-256 des Zugangsdatums steht darin (${sh256:0:16}) -- ein Kennwortwechsel ist eine Generation" \
        || nok "auch die Pruefsumme des Zugangsdatums fehlt -- ein Kennwortwechsel waere keine Generation"

    # ------------------------------------ 5. der PLAN ist das, was `sort` macht
    #
    # Die kanonische Reihenfolge laesst sich damit VON AUSSEN pruefen,
    # ohne dieses Programm zu kennen.
    local G; G=$(cat "$A/system/AKTUELL")
    if LC_ALL=C sort -c "$A/system/generations/$G/PLAN" 2>/dev/null; then
        ok "der PLAN ist nach den Oktetten der ganzen Zeile sortiert ($(wc -l < "$A/system/generations/$G/PLAN") Zeilen, von aussen mit 'sort -c' geprueft)"
    else
        nok "der PLAN ist nicht sortiert -- zwei gleiche Systeme haetten verschiedene Dateien"
    fi
    # GEGENPROBE: eine verwuerfelte Datei MUSS bei `sort -c` umfallen,
    # sonst misst die Zusage oben nichts.
    tac "$A/system/generations/$G/PLAN" > "$T/verdreht"
    if LC_ALL=C sort -c "$T/verdreht" 2>/dev/null; then
        nok "auch eine umgedrehte Datei gilt als sortiert -- die Pruefung misst nichts"
    else
        ok "GEGENPROBE: dieselbe Datei umgedreht faellt bei 'sort -c' um"
    fi

    # ------------------------------------------ 6. reservierte Namen
    #
    # Fuenf Namen wuerden eine alte PLAN-Zeile mehrdeutig machen. Sie
    # werden beim BAUEN abgelehnt -- dem letzten Zeitpunkt, an dem es
    # nichts kostet.
    printf 'name=kernel\nfassung=1.0.0\ntitel=X\ndatei=start %s/vendor/osum/bin/true\n' "$repo" > "$T/res.rezept"
    if $O bauen "$T/res.rezept" -o "$T/res.opk" >"$T/aus" 2>&1; then
        nok "ein Paket namens 'kernel' liess sich bauen -- alte PLAN-Zeilen waeren mehrdeutig"
    else
        ok "ein Paket namens 'kernel' wird abgelehnt: $(grep -m1 -o 'cannot be a package name' "$T/aus")"
    fi
    printf 'name=kernelchen\nfassung=1.0.0\ntitel=X\ndatei=start %s/vendor/osum/bin/true\n' "$repo" > "$T/ok.rezept"
    $O bauen "$T/ok.rezept" -o "$T/ok.opk" >/dev/null 2>&1 \
        && ok "GEGENPROBE: 'kernelchen' laesst sich bauen -- es sind genau die fuenf Typnamen und nicht alles, was so aehnlich klingt" \
        || nok "auch ein unverfaenglicher Name wird abgelehnt"

    # ================================================================
    # 7. DIE KERNMESSUNG: aus EINER Textdatei einen Baum von NULL
    # ================================================================
    local B="$T/B"
    if $O rebuild --root "$B" --plan "$T/plan.txt" \
                  --secrets "$A/system/secrets" > "$T/reb" 2>&1; then
        ok "aus PLAN + Quelle entsteht ein Baum von null: $(grep -m1 'package(s) fetched' "$T/reb" | xargs)"
    else
        nok "der Nachbau ist fehlgeschlagen: $(tail -2 "$T/reb" | tr '\n' ' ')"
    fi
    $O snapshot --root "$A" > "$T/snapA"
    $O snapshot --root "$B" > "$T/snapB"
    local eintraege oktette
    eintraege=$(awk '/^count /{print $2}' "$T/snapA" | cut -d= -f2)
    oktette=$(awk '/^count /{print $4}' "$T/snapA" | cut -d= -f2)
    # ZUERST: dass ueberhaupt etwas verglichen wird. Zwei leere Baeume
    # sind immer gleich, und das ist der Fehler, den diese Zusage
    # ausschliesst.
    if [[ "${eintraege:-0}" -ge 25 && "${oktette:-0}" -ge 1000000 ]]; then
        ok "verglichen werden $eintraege Eintraege und $oktette Oktette -- nicht zwei leere Baeume"
    else
        nok "es waeren nur $eintraege Eintraege / $oktette Oktette verglichen worden"
    fi
    if cmp -s "$T/snapA" "$T/snapB"; then
        ok "der nachgebaute Baum ist OKTETT FUER OKTETT der urspruengliche ($eintraege Eintraege, $oktette Oktette, SHA-256 je Inhalt)"
    else
        nok "der Nachbau weicht ab:"
        diff "$T/snapA" "$T/snapB" | head -8 | sed 's/^/         /'
    fi
    # GEGENPROBE ZUM VERGLEICH SELBST: ein Baum mit einem Paket weniger
    # MUSS auffallen. Ohne das koennte `snapshot` konstant sein.
    $O entfernen --root "$B" wc >/dev/null 2>&1
    $O snapshot --root "$B" > "$T/snapB2"
    if cmp -s "$T/snapA" "$T/snapB2"; then
        nok "auch nach dem Entfernen eines Pakets ist der Vergleich gleich -- er misst nichts"
    else
        ok "GEGENPROBE: ein Paket weniger, und der Vergleich faellt um ($(diff "$T/snapA" "$T/snapB2" | grep -c '^[<>]') Zeilen Unterschied)"
    fi

    # 7b. DERSELBE NACHBAU OHNE DIE ZUGANGSDATEN. Er muss sich in GENAU
    # einer Zeile unterscheiden, und die muss /etc/shadow sein -- das ist
    # die ehrliche Grenze eines Plans, der keine Kennwoerter traegt.
    local C="$T/C"
    $O rebuild --root "$C" --plan "$T/plan.txt" > "$T/reb2" 2>&1
    $O snapshot --root "$C" > "$T/snapC"
    local nzeilen
    diff "$T/snapA" "$T/snapC" > "$T/dAC" || true
    nzeilen=$(grep -c '^<' "$T/dAC" || true)
    if [[ "$nzeilen" -eq 2 ]] && grep -q 'etc/shadow' "$T/dAC"; then
        ok "ohne die Zugangsdaten unterscheidet sich genau /etc/shadow (und die Zaehlzeile) -- die Konten sind gesperrt, nicht offen"
    else
        nok "ohne die Zugangsdaten weicht mehr ab als /etc/shadow ($nzeilen Zeile(n))"
    fi
    grep -q ':!:' "$C/etc/shadow" \
        && ok "GEGENPROBE: in /etc/shadow des Nachbaus steht ':!:' -- das Konto ist gesperrt und nicht kennwortlos" \
        || nok "/etc/shadow des Nachbaus sperrt das Konto nicht"

    # 7c. DER PLAN TRAEGT SEIN VERTRAUEN. Eine Quelle mit denselben
    # Paketen, aber einem anderen Schluessel, wird abgelehnt.
    mkdir -p "$T/fremd"; cp "$T/src"/*.opk "$T/fremd/"
    $O schluessel "$T/fkey" >/dev/null 2>&1
    $O quelle "$T/fremd" --schluessel "$T/fkey/geheim.key" >/dev/null 2>&1
    $O rebuild --root "$T/D" --plan "$T/plan.txt" --source "$T/fremd" > "$T/reb3" 2>&1
    grep -q 'REFUSED' "$T/reb3" \
        && ok "eine Quelle mit fremdem Schluessel wird abgelehnt: $(grep -m1 -o 'not signed by any of.*' "$T/reb3" | cut -c1-46)" \
        || nok "eine Quelle mit fremdem Schluessel wurde benutzt"
    # ...und die Gegenprobe: mit dem RICHTIGEN Schluessel geht dieselbe
    # Quelle durch. Sonst waere „lehnt jede Quelle ab" hier gruen.
    cp "$T/key/oeffentlich.key" "$T/fremd/"
    $O quelle "$T/fremd" --schluessel "$T/key/geheim.key" >/dev/null 2>&1
    $O rebuild --root "$T/E" --plan "$T/plan.txt" --source "$T/fremd" > "$T/reb4" 2>&1
    grep -q 'REFUSED' "$T/reb4" \
        && nok "auch mit dem richtigen Schluessel wird die Quelle abgelehnt" \
        || ok "GEGENPROBE: dieselbe Quelle, mit dem Schluessel des Plans signiert, wird angenommen"
    # Ein Plan ohne Quelle ist nicht selbsttragend, und das sagt er.
    grep -v '^source' "$T/plan.txt" > "$T/ohnequelle.txt"
    if $O rebuild --root "$T/F" --plan "$T/ohnequelle.txt" >"$T/aus" 2>&1; then
        nok "ein Plan ohne Quelle liess sich nachbauen -- aus dem Nichts"
    else
        ok "ein Plan ohne Quelle wird abgelehnt: $(grep -m1 -o 'names no source.*' "$T/aus" | cut -c1-40)"
    fi

    # ------------------------------------- 8. Kernelwechsel und zurueck
    printf 'name=osum\nfassung=2.0.0\ntitel=Osum\ninfo=zweites Abbild, fuer die Messung\nkeys=osum\nkind=kernel\norigin=osum:zweites-abbild\ndatei=image %s/vendor/osum/osum.mb.elf\n' "$repo" > "$T/k2.rezept"
    $O bauen "$T/k2.rezept" -o "$T/k2.opk" >/dev/null 2>&1
    local vor nach zurueck_h
    vor=$(sha256sum "$A/system/kernel/image" | cut -d' ' -f1)
    local vorgen; vorgen=$(cat "$A/system/AKTUELL")
    $O kernel --root "$A" "$T/k2.opk" >/dev/null 2>&1
    nach=$(sha256sum "$A/system/kernel/image" | cut -d' ' -f1)
    [[ "$vor" != "$nach" ]] \
        && ok "ein Kernelwechsel taucht die Oktette unter system/kernel/image aus (${vor:0:12} -> ${nach:0:12})" \
        || nok "nach dem Kernelwechsel liegen dieselben Oktette da -- der Wechsel misst nichts"
    $O zurueck --root "$A" "$vorgen" > "$T/zur" 2>&1
    zurueck_h=$(sha256sum "$A/system/kernel/image" | cut -d' ' -f1)
    if [[ "$zurueck_h" == "$vor" ]]; then
        ok "'zurueck' stellt den alten KERNEL wieder her, nicht nur die Anwendungen ($(grep -m1 -o 'KERNEL zurueckgestellt.*' "$T/zur" | cut -c1-46))"
    else
        nok "nach dem Zurueckrollen liegt der falsche Kernel da"
    fi
    # Und der Store haelt den gerade nicht benutzten Kernel weiter --
    # ohne das waere ein zweites Zurueckrollen unmoeglich.
    local k2kurz; k2kurz=$(python3 -c "
import sys; sys.path.insert(0, 'pkg'); import opk
print(opk.kurz(opk.paket_lesen(open('$T/k2.opk','rb').read())[2]))")
    [[ -d "$A/store/$k2kurz" ]] \
        && ok "der Store haelt auch den gerade NICHT benutzten Kernel (store/${k2kurz:0:12})" \
        || nok "der abgewaehlte Kernel ist aus dem Store verschwunden -- ein Vorwaertsrollen waere unmoeglich"

    # --------------------------------------------------------- 9. verify
    $O verify --root "$A" > "$T/ver" 2>&1 \
        && ok "verify auf dem unberuehrten Baum: $(tail -1 "$T/ver")" \
        || nok "verify meldet einen Fehler an einem unberuehrten Baum: $(grep -m1 FAILED "$T/ver")"
    # GEGENPROBE: eine von Hand verdrehte PLAN-Datei MUSS auffallen.
    G=$(cat "$A/system/AKTUELL")
    cp "$A/system/generations/$G/PLAN" "$T/plan.sicher"
    tac "$T/plan.sicher" > "$A/system/generations/$G/PLAN"
    if $O verify --root "$A" >"$T/ver2" 2>&1; then
        nok "eine unsortierte PLAN-Datei faellt bei verify NICHT auf"
    else
        ok "GEGENPROBE: eine unsortierte PLAN-Datei -> $(grep -m1 FAILED "$T/ver2" | cut -c1-56)"
    fi
    cp "$T/plan.sicher" "$A/system/generations/$G/PLAN"
    # GEGENPROBE 2: ein veraendertes Zugangsdatum passt nicht mehr zu
    # seiner Pruefsumme im Plan.
    printf 'x' >> "$A/system/secrets/justin"
    if $O verify --root "$A" >"$T/ver3" 2>&1; then
        nok "ein veraendertes Zugangsdatum faellt nicht auf"
    else
        ok "GEGENPROBE: ein veraendertes Zugangsdatum -> $(grep -m1 'FAILED.*credential' "$T/ver3" | cut -c1-56)"
    fi

    # ------------------------------------------- 10. wieviel paketiert ist
    #
    # Vor dieser Runde waren es drei von neunundsechzig gebauten
    # Programmen. Die Zahl steht hier, damit sie beim naechsten Mal
    # nachgezaehlt und nicht abgeschrieben wird.
    local gebaut geindext programme
    gebaut=$(ls build/pakete/*.opk 2>/dev/null | wc -l)
    geindext=$(grep -c . build/quelle/INDEX 2>/dev/null || echo 0)
    programme=$(ls vendor/osum/bin 2>/dev/null | wc -l)
    if [[ "$gebaut" -ge 60 && "$gebaut" -eq "$geindext" ]]; then
        ok "$gebaut Pakete gebaut aus $programme Programmen in vendor/osum/bin, und der signierte INDEX nennt genau dieselbe Zahl"
    else
        nok "$gebaut Pakete gebaut, $geindext im INDEX, $programme Programme -- die Zahlen passen nicht"
    fi
    # GEGENPROBE: die Rezepte sind ERZEUGT und nicht eingecheckt. Ein
    # eingechecktes Rezept waere die zweite Stelle, an der ein Name steht.
    local imrepo; imrepo=$(ls pkg/rezepte/*.rezept 2>/dev/null | wc -l)
    [[ "$imrepo" -le 2 ]] \
        && ok "GEGENPROBE: im Repo liegen nur $imrepo handgeschriebene(s) Rezept(e) -- die uebrigen entstehen bei jedem Bau" \
        || nok "es liegen $imrepo Rezepte im Repo -- das sind zu viele zweite Stellen"

    rm -rf "$T"
    return $RC
}
run plan2_check
