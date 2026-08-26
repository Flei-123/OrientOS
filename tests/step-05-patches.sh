# tests/step-05-patches.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# BERICHTIGUNGEN AM FESTGENAGELTEN KERNEL — und warum sie gemessen werden.
#
# Ein festgenagelter Commit ist ein BEKANNTER Stand, kein fehlerfreier.
# Beim Nachziehen auf Osum c5fe12f stellte sich heraus, dass dieser
# Merge-Commit ueberhaupt nicht uebersetzt: dem Merge sind in
# `kernel/kmain.fi` eine schliessende Klammer abhanden gekommen und in
# `kernel/sys.fi` ist die K15-Naht in der falschen Funktion gelandet.
# Beides ist in `vendor/osum/patches/` berichtigt, und `hole-osum.sh`
# wendet die Dateien beim Auspacken an.
#
# Ein Patchstapel ist ein Ort, an dem still etwas verrottet: Osum
# berichtigt die Stelle irgendwann selbst, und dann traegt OrientOS jahrelang
# eine Datei mit sich herum, die niemand mehr anfassen mag, weil keiner
# weiss, ob sie noch etwas tut. Dagegen steht dieser Schritt.
#
# Vier Fragen, und die letzte ist die einzige, die wirklich etwas beweist:
#
#   1. Sagt jeder Patch, WAS er berichtigt, WARUM es ein Fehler ist und auf
#      welchen Stand er sich beruft? Ein Patch ohne Begruendung ist ein
#      Fork mit anderem Namen.
#   2. Passt er noch auf den festgenagelten Commit — geprueft gegen den
#      Baum, den `git archive <COMMIT>` liefert, nicht gegen das
#      Arbeitsverzeichnis des Kernelrepos, in dem parallel gearbeitet wird?
#   3. Uebersetzt der Kernel MIT dem ganzen Stapel?
#   4. GEGENPROBE, JE PATCH EINZELN: nimmt man GENAU EINEN heraus und
#      laesst die anderen stehen, muss firnc den Kernel ABLEHNEN. Damit ist
#      fuer jeden einzelnen Patch gezeigt, dass er noch gebraucht wird —
#      und nicht nur fuer den Stapel als Ganzes. Faellt einer weg, weil
#      Osum die Stelle selbst berichtigt hat, wird der Lauf rot und sagt,
#      welche Datei zu loeschen ist.
#
# Uebersetzt wird `kernel/kmain.fi` — die Uebersetzungseinheit, mit der
# auch `tools/build-kernel.sh` anfaengt. Alle anderen Kerneldateien haengen
# per `import` daran, ein Fehler in `sys.fi` faellt also hier auf. Ein Lauf
# kostet rund zwei Sekunden.
step "Berichtigungen am festgenagelten Kernel: begruendet, passend, und einzeln noetig"
patches_check() {
    RC=0
    local c p n=0 k
    c=$(cat vendor/osum/COMMIT 2>/dev/null || echo -)

    shopt -s nullglob
    local patches=(vendor/osum/patches/*.patch)
    shopt -u nullglob

    if [[ ${#patches[@]} -eq 0 ]]; then
        # Kein Patch ist der ERWUENSCHTE Endzustand. Dann bleibt eine
        # Zusage: dass hole-osum.sh die Vorrichtung noch kennt, damit sie
        # beim naechsten kaputten Merge wieder da ist.
        grep -q 'patches/\*\.patch' vendor/osum/hole-osum.sh \
            && ok "kein Patch noetig — und hole-osum.sh kennt die Vorrichtung weiterhin" \
            || nok "kein Patch da, und hole-osum.sh hat die Vorrichtung verloren"
        return $RC
    fi

    # --- 1. Begruendung
    for p in "${patches[@]}"; do
        n=$((n+1))
        local kopf
        kopf=$(sed -n '1,/^--- /p' "$p")
        if grep -q '^Betrifft:' <<<"$kopf" \
           && grep -q '^Befund:'   <<<"$kopf" \
           && grep -q '^Wirkung:'  <<<"$kopf"; then
            ok "$(basename "$p") sagt Betrifft/Befund/Wirkung"
        else
            nok "$(basename "$p") hat keinen begruendenden Kopf"
        fi
        if grep -q "${c:0:7}" <<<"$kopf"; then
            ok "$(basename "$p") beruft sich auf den festgenagelten Stand ${c:0:8}"
        else
            nok "$(basename "$p") beruft sich nicht auf ${c:0:8} — auf welchen dann?"
        fi
    done
    ok "$n Berichtigung(en) im Stapel"

    # --- Der Baum des Commits. Ausgepackt wie in hole-osum.sh: `git
    # archive`, kein `git worktree` — im Kernelrepo wird parallel gearbeitet.
    local osum=""
    for k in ${OSUM_REPO:-} ../osum ../../osum "$HOME/osum"; do
        [[ -d ${k:-}/.git ]] && git -C "$k" cat-file -e "$c^{commit}" 2>/dev/null \
            && { osum=$(cd "$k" && pwd); break; }
    done
    if [[ -z "$osum" ]]; then
        nok "das Osum-Repo mit ${c:0:8} wurde nicht gefunden — die Zusagen 2 bis 4 messen nichts"
        return $RC
    fi

    local roh; roh=$(mktemp -d "${TMPDIR:-/tmp}/orientos-patchprobe-XXXXXX")
    git -C "$osum" archive "$c" | tar -x -C "$roh"
    # Der unberuehrte Stand, aus dem einzelne Dateien zurueckgeholt werden.
    cp -r "$roh/kernel" "$roh/.kernel-roh"

    local hier; hier=$(pwd)
    local alle_passen=1
    for p in "${patches[@]}"; do
        if git -C "$roh" apply --check -p1 "$hier/$p" 2>/dev/null; then
            git -C "$roh" apply -p1 "$hier/$p"
        else
            nok "$(basename "$p") passt NICHT mehr auf ${c:0:8} — Kopf lesen und die Datei wegwerfen, nicht anpassen"
            alle_passen=0
        fi
    done
    [[ $alle_passen -eq 1 ]] && ok "alle Berichtigungen passen auf den ausgepackten Baum von ${c:0:8}"

    # --- Der Uebersetzer, den OSUM festnagelt (nicht der von OrientOS).
    local firnc=""
    if [[ -x "$roh/vendor/firn/bin/firnc" ]]; then
        firnc="$roh/vendor/firn/bin/firnc"
    else
        local firnrepo=""
        for k in ${FIRN_REPO:-} ../firn ../../firn "$HOME/firn"; do
            [[ -d ${k:-}/.git ]] && { firnrepo=$(cd "$k" && pwd); break; }
        done
        if [[ -n "$firnrepo" ]]; then
            ( cd "$roh" && FIRN_REPO="$firnrepo" bash vendor/firn/hole-firnc.sh ) >/dev/null 2>&1
            [[ -x "$roh/vendor/firn/bin/firnc" ]] && firnc="$roh/vendor/firn/bin/firnc"
        fi
    fi
    if [[ -z "$firnc" ]]; then
        nok "kein firnc greifbar — die Wirkung der Berichtigungen ist NICHT gemessen"
        rm -rf "$roh"; return $RC
    fi

    # `firnc` schreibt neben der Ausgabedatei auch <ausgabe>.s — deshalb ein
    # echter Pfad und nicht /dev/null.
    uebersetzt() {
        ( cd "$roh" && FIRNLIB="$roh/lib" "$firnc" kernel/kmain.fi -o "$roh/probe.o" ) \
            >"$roh/probe.log" 2>&1
    }

    # --- 3. mit dem ganzen Stapel
    if uebersetzt; then
        ok "mit allen $n Berichtigung(en) uebersetzt der Kernel ($(stat -c%s "$roh/probe.o") Oktette)"
    else
        nok "der Kernel uebersetzt TROTZ der Berichtigungen nicht: $(head -1 "$roh/probe.log")"
    fi

    # --- 4. je Patch einzeln zuruecknehmen
    for p in "${patches[@]}"; do
        local dateien=()
        while read -r d; do dateien+=("$d"); done < <(
            grep '^+++ b/' "$p" | sed -e 's|^+++ b/||' -e 's/[[:space:]].*$//' | sort -u)
        local f
        for f in "${dateien[@]}"; do
            [[ -f "$roh/.${f/kernel\//kernel-roh/}" ]] \
                && cp "$roh/.${f/kernel\//kernel-roh/}" "$roh/$f"
        done
        if uebersetzt; then
            nok "OHNE $(basename "$p") uebersetzt der Kernel trotzdem — die Berichtigung ist ueberfluessig geworden und gehoert GELOESCHT"
        else
            ok "GEGENPROBE: ohne $(basename "$p") bricht firnc ab ($(head -1 "$roh/probe.log" | cut -c1-60))"
        fi
        # wieder anwenden, damit die naechste Runde nur EINEN Patch vermisst
        git -C "$roh" apply -p1 "$hier/$p" 2>/dev/null || true
    done

    rm -rf "$roh"
    return $RC
}
run patches_check
