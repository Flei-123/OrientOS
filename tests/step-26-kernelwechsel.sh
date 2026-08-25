# tests/step-26-kernelwechsel.sh — wird von test.sh gesourct, nicht direkt
# gestartet.
#
# DAMIT DIE DOPPELUNG NICHT WIEDERKOMMT.
#
# Es gab in diesem Projekt zwei Kernel mit demselben Namen, und der Grund
# war nicht Absicht, sondern Doku, die stehengeblieben ist: LANGUAGE.md
# beschrieb eine modulweise Migration nach Firn, waehrend daneben ein
# fertiger Firn-Kernel entstand. Zwei Wahrheiten in einem Projekt sind
# eine Wahrheit zu viel.
#
# Dieser Schritt prueft deshalb nicht Code, sondern ob die Dokumente noch
# dasselbe sagen wie der Baum:
#
#   1. KERNELWECHSEL.md gibt es, und es enthaelt wirklich den Abgleich und
#      die offenen Punkte -- nicht nur eine Ueberschrift.
#   2. Der alte Migrationsstand wird nirgends mehr als GELTEND behauptet.
#   3. Die Zahl "noch in Rust" stimmt mit dem Baum ueberein. Eine Zahl in
#      der Doku, die niemand nachzaehlt, ist in drei Runden falsch.
#   4. Jeder offene Punkt aus KERNELWECHSEL.md § 4 hat den Code, auf den
#      er sich beruft -- sonst waere er erledigt und niemand haette es
#      gemerkt.
#   5. Der Rust-Kernel ist NICHT geloescht. Nichts wird geloescht, bevor
#      der Ersatz nachweislich laeuft.
step "Kernelwechsel: die Doku sagt dasselbe wie der Baum"
kernelwechsel_doku() {
    local rc=0

    # --- 1. Das Dokument selbst
    if [[ ! -f KERNELWECHSEL.md ]]; then
        echo "  [FEHL] KERNELWECHSEL.md fehlt"
        return 1
    fi
    local abschnitt
    for abschnitt in \
        '## 2. Der Abgleich, Modul für Modul' \
        '## 3. Was portiert wurde' \
        '## 4. Was NOCH IN RUST STEHT' \
        '## 6. Die Geschichte dieses Wechsels'
    do
        if grep -qF "$abschnitt" KERNELWECHSEL.md; then
            echo "  [ ok ] KERNELWECHSEL.md: $abschnitt"
        else
            echo "  [FEHL] KERNELWECHSEL.md: Abschnitt fehlt — $abschnitt"
            rc=1
        fi
    done
    # Der Abgleich muss eine TABELLE sein, keine Absichtserklaerung.
    local zeilen
    zeilen=$(grep -cE '^\| .* \| .* \| .* \|' KERNELWECHSEL.md)
    if [[ "$zeilen" -ge 20 ]]; then
        echo "  [ ok ] der Abgleich hat $zeilen Tabellenzeilen"
    else
        echo "  [FEHL] der Abgleich hat nur $zeilen Tabellenzeilen — das ist keine Modul-fuer-Modul-Pruefung"
        rc=1
    fi

    # --- 2. Alle Hauptdokumente verweisen darauf
    local d
    for d in README.md ARCHITECTURE.md ROADMAP.md LANGUAGE.md; do
        if grep -qF 'KERNELWECHSEL.md' "$d"; then
            echo "  [ ok ] $d verweist auf KERNELWECHSEL.md"
        else
            echo "  [FEHL] $d verweist nicht auf KERNELWECHSEL.md — dort steht, was gilt"
            rc=1
        fi
    done

    # --- 3. Der alte Stand wird nicht mehr als geltend behauptet
    if grep -qF 'ÜBERHOLT AM 25.08.2026' LANGUAGE.md; then
        echo "  [ ok ] LANGUAGE.md kennzeichnet den alten Migrationsstand als überholt"
    else
        echo "  [FEHL] LANGUAGE.md behauptet den alten Migrationsstand weiter als geltend"
        rc=1
    fi
    # README darf nicht mehr sagen, der Umbau dieses Baums laufe modulweise.
    if grep -qF 'Der Umbau läuft modulweise' README.md; then
        echo "  [FEHL] README.md kuendigt weiter die modulweise Migration DIESES Baums an"
        rc=1
    else
        echo "  [ ok ] README.md kuendigt keine modulweise Migration dieses Baums mehr an"
    fi

    # --- 4. Die Zahl stimmt mit dem Baum
    local ist ist_fmt
    ist=$(find kernel/src libs -name '*.rs' -exec cat {} + | wc -l)
    # In der Doku steht sie mit schmalem Trenner: 18 017
    ist_fmt=$(printf '%d' "$ist" | sed -E 's/([0-9]+)([0-9]{3})$/\1 \2/')
    if grep -qF "$ist_fmt" LANGUAGE.md && grep -qF "$ist_fmt" KERNELWECHSEL.md; then
        echo "  [ ok ] \"noch in Rust\" = $ist_fmt Zeilen, so steht es in LANGUAGE.md und KERNELWECHSEL.md"
    else
        echo "  [FEHL] der Baum hat $ist_fmt Zeilen Rust; die Doku nennt eine andere Zahl"
        grep -nE '[0-9]+ [0-9]{3} Zeilen' LANGUAGE.md KERNELWECHSEL.md | head -5 | sed 's/^/         /'
        rc=1
    fi

    # --- 5. Jeder offene Punkt beruft sich auf Code, den es gibt
    local datei
    for datei in \
        kernel/src/arch/x86_64/user.rs \
        kernel/src/kcore/arch_iface.rs \
        kernel/src/drivers/fbcon.rs \
        kernel/src/drivers/font.rs \
        kernel/src/kcore/initramfs.rs \
        kernel/src/abi/native.rs \
        libs/osum-abi-native/src/table.rs
    do
        if [[ -f "$datei" ]]; then
            echo "  [ ok ] offener Punkt belegt: $datei"
        else
            echo "  [FEHL] KERNELWECHSEL.md § 4 nennt $datei — die Datei gibt es nicht (mehr)"
            rc=1
        fi
    done
    # Gegenprobe zu Punkt 4.1: steht SMEP wirklich nur in der Rust-Fassung?
    if grep -q 'CR4_SMEP' kernel/src/arch/x86_64/user.rs; then
        echo "  [ ok ] SMEP/SMAP steht im Rust-Kernel — der offene Punkt ist echt"
    else
        echo "  [FEHL] SMEP/SMAP nicht mehr im Rust-Kernel; KERNELWECHSEL.md § 4.1 ist veraltet"
        rc=1
    fi

    # --- 6. Nichts wurde geloescht, bevor der Ersatz laeuft
    if [[ -d kernel/src && -d libs && -f Cargo.toml ]]; then
        echo "  [ ok ] der Rust-Kernel steht vollstaendig da (kernel/src, libs/)"
    else
        echo "  [FEHL] der Rust-Kernel ist verschwunden, obwohl offene Punkte an ihm haengen"
        rc=1
    fi
    # ...und er wird wirklich noch gebaut. Ein Zweig, der nur behauptet zu
    # existieren, ist in zwei Runden kaputt.
    if grep -q 'kernel rust' test.sh; then
        echo "  [ ok ] der Testlauf baut ihn weiterhin (--kernel rust)"
    else
        echo "  [FEHL] der Testlauf baut den Rust-Kernel nicht mehr — die Vorlage verrottet unbemerkt"
        rc=1
    fi
    return $rc
}
run kernelwechsel_doku
