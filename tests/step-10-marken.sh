# SPDX-License-Identifier: GPL-2.0-only
# tests/step-10-marken.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# EIN QUELLBAUM, ZWEI PRODUKTE. Das ist die Rolle, die OrientOS nach dem
# Kernelwechsel hat, und sie ist die einzige, die sich NICHT in den Kernel
# schieben laesst: der Kernel heisst in jeder Marke `osum`, so wie NT in
# jeder Windows-Ausgabe NT heisst. Was sich unterscheidet, steht in
# `brands/*.toml` und spaeter in Daten — nie in Code (BRANDING.md).
#
# Was hier gemessen wird:
#
#   1. Jede Markendatei ist vollstaendig und ihr `slug` taugt als
#      Dateiname. Ein fehlendes Feld faellt sonst erst beim Bauen auf,
#      ein doppelter `slug` ueberschreibt ein Abbild.
#   2. `brands/STANDARD` zeigt auf eine Marke, die es gibt. Seit dem
#      Kernelwechsel steht die Standardmarke dort und nicht mehr in
#      `kernel/Cargo.toml` — das gibt es nicht mehr.
#   3. Die Bauskripte kennen KEINEN Markennamen. Ein hartkodierter Name
#      faerbt jede Zweitmarke rot, obwohl alles richtig ist.
#   4. GEGENPROBE MIT WIRKUNG: die zweite Marke wird wirklich gebaut. Es
#      entsteht ein zweites ISO mit einem anderen Namen, und in seiner
#      limine.conf steht der andere Produktname.
#   5. Eine erfundene Marke bricht ab und faellt NICHT still auf die
#      Standardmarke zurueck.
step "Marken: ein Quellbaum, zwei Produkte"
marken_check() {
    RC=0
    local n=0 slugs="" f on sl
    shopt -s nullglob
    for f in brands/*.toml; do
        n=$((n+1))
        on=$(brand_feld "$f" os-name); sl=$(brand_feld "$f" slug)
        if [[ -z "$on" || -z "$sl" ]]; then
            nok "$f: os-name oder slug fehlt"; continue
        fi
        if [[ ! "$sl" =~ ^[a-z][a-z0-9-]*$ ]]; then
            nok "$f: slug \"$sl\" taugt nicht als Dateiname"; continue
        fi
        case " $slugs " in *" $sl "*) nok "slug doppelt vergeben: $sl"; continue ;; esac
        slugs="$slugs $sl"
        ok "$f -> $on ($sl)"
    done
    [[ $n -ge 2 ]] && ok "$n Marken — mehr als eine, sonst misst das hier nichts" \
                   || nok "nur $n Markendatei(en)"

    if [[ -f brands/STANDARD ]]; then
        local std
        std=$(tr -d '[:space:]' < brands/STANDARD)
        if [[ -f "brands/$std.toml" ]]; then
            ok "brands/STANDARD nennt \"$std\", und die Datei gibt es"
        else
            nok "brands/STANDARD nennt \"$std\" — brands/$std.toml fehlt"
        fi
    else
        nok "brands/STANDARD fehlt — die Standardmarke haette keine Quelle mehr"
    fi

    # Kein Markenname in den Bauskripten.
    if grep -n "$OS_NAME" build.sh run-osum.sh brand.sh | grep -vE '^\S+:[0-9]+:[[:space:]]*#'; then
        nok "Markenname in einem Bauskript hartkodiert (gehoert nach brands/)"
    else
        ok "build.sh, run-osum.sh und brand.sh kennen keinen Markennamen"
    fi

    # GEGENPROBE MIT WIRKUNG: die zweite Marke bauen.
    local zweit
    zweit=$(ls brands/*.toml | xargs -n1 basename | sed 's/\.toml$//' \
            | grep -v "^$BRAND$" | head -1)
    if [[ -n "$zweit" ]]; then
        local zslug
        zslug=$(brand_feld "brands/$zweit.toml" slug)
        if ./build.sh --brand "$zweit" >/dev/null 2>&1; then
            ok "die zweite Marke ($zweit) baut durch"
        else
            nok "./build.sh --brand $zweit ist fehlgeschlagen"
        fi
        if [[ -s "build/$zslug.iso" ]]; then
            ok "es entsteht build/$zslug.iso ($(( $(stat -c%s "build/$zslug.iso") / 1024 )) KiB)"
        else
            nok "build/$zslug.iso fehlt"
        fi
        local zname
        zname=$(brand_feld "brands/$zweit.toml" os-name)
        if grep -qF "/$zname" build/isoroot/boot/limine/limine.conf; then
            ok "und ihr Bootmenue traegt den anderen Produktnamen ($zname)"
        else
            nok "limine.conf der zweiten Marke nennt nicht $zname"
        fi
        # Und der Kernel heisst trotzdem in beiden gleich.
        if [[ -f build/isoroot/boot/osum ]]; then
            ok "der Kernel heisst auch dort osum — die Marke faerbt das Produkt, nicht den Kern"
        else
            nok "im Abbild der zweiten Marke liegt kein boot/osum"
        fi
    else
        nok "es gibt keine zweite Marke zum Gegenprobieren"
    fi

    # Eine erfundene Marke MUSS abbrechen.
    if ./build.sh --brand gibtsnicht >/dev/null 2>&1; then
        nok "eine erfundene Marke baut durch — der Tippfehler faellt still auf die Standardmarke zurueck"
    else
        ok "Gegenprobe: eine erfundene Marke bricht ab, statt still die Standardmarke zu nehmen"
    fi

    # Zum Schluss die Standardmarke wiederherstellen, damit die folgenden
    # Schritte das Produkt messen und nicht die Zweitmarke.
    ./build.sh >/dev/null 2>&1 || nok "die Standardmarke laesst sich nicht wieder bauen"
    return $RC
}
run marken_check
