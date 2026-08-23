# tests/step-23-firn-elf.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# Der ELF64-PRUEFTEIL liegt seit dem 23.08.2026 in Firn (kernel/firn/elf.fi).
#
# Vorher hatte `kernel/src/kcore/elf.rs` NULL `#[test]`. Was es gab, war ein
# Selbsttest im Kernel mit 16 Faellen — besser als nichts, aber kein Massstab,
# an dem sich eine Neufassung messen laesst: die Faelle standen als Rust-Code
# IN der Datei, die ersetzt werden sollte.
#
# Deshalb wurde der Massstab ZUERST gebaut und gegen die ALTE Fassung belegt:
# 53 Falldateien in tests/firn-elf/faelle/, erzeugt von faelle.py. Beide
# Fassungen lesen dieselben Oktette und muessen denselben Fehlercode liefern.
#
# Dieser Abschnitt faehrt BEIDE. Das ist Absicht und kein Aufwand umsonst:
# `lauf-rust.sh` schneidet den alten Pruefteil aus der Git-Historie und haelt
# ihn gegen dieselben Faelle. Solange beide dasselbe sagen, ist die Portierung
# nachweislich verhaltensgleich — und nicht nur "besteht auch Tests".
step "Firn-ELF: 53 Faelle, beide Fassungen, gleiches Ergebnis"
firn_elf() {
    local rc=0

    # 1) Die Firn-Fassung gegen den Massstab.
    ./tests/firn-elf/lauf.sh || rc=1

    # 2) Dieselben Faelle gegen die ALTE Rust-Fassung. Sie liegt als
    #    Referenzmassstab in tests/firn-elf/alter-pruefteil.rs.txt und wird
    #    NICHT mehr gebaut -- aber gefahren. Solange beide Fassungen zu jedem
    #    Fall dasselbe sagen, ist die Portierung nachweislich verhaltensgleich.
    if ./tests/firn-elf/lauf-rust.sh > build/firn-elf/alt.txt 2>&1; then
        echo "  [ ok ] die alte Rust-Fassung besteht denselben Massstab ($(grep -c '\[ ok \]' build/firn-elf/alt.txt) Faelle)"
    else
        echo "  [FEHL] die alte Rust-Fassung faellt am Massstab durch:"
        tail -6 build/firn-elf/alt.txt
        rc=1
    fi

    # 3) Beide Seiten muessen sich ueber die Segmentgrenze einig sein. Weichen
    #    sie ab, schreibt die Firn-Seite ueber das Feld der Rust-Seite hinaus —
    #    und zwar ohne dass irgendetwas abstuerzt.
    local fi_max rs_max
    fi_max=$(grep -oP 'const MAX_SEGMENTS: u64 = \K[0-9]+' kernel/firn/elf.fi)
    rs_max=$(grep -oP 'pub const MAX_SEGMENTS: usize = \K[0-9]+' kernel/src/kcore/firn_elf.rs)
    if [[ "$fi_max" != "$rs_max" ]]; then
        echo "  [FEHL] MAX_SEGMENTS weicht ab: elf.fi=$fi_max firn_elf.rs=$rs_max"
        rc=1
    else
        echo "  [ ok ] MAX_SEGMENTS stimmt ueberein ($fi_max)"
    fi

    # 4) Der Strukturaufbau, mit Gegenprobe.
    local fi_felder rs_felder
    fi_felder=$(sed -n '/^struct ElfSegment {/,/^}/p' kernel/firn/elf.fi \
                | grep -oE '^\s+[a-z_]+:' | tr -d ' :' | tr '\n' ' ')
    rs_felder=$(sed -n '/^struct FirnSegment {/,/^}/p' kernel/src/kcore/firn_elf.rs \
                | grep -oE '^\s+[a-z_]+:' | tr -d ' :' | tr '\n' ' ')
    if [[ "$fi_felder" != "$rs_felder" ]]; then
        echo "  [FEHL] Segmentaufbau weicht ab:"
        echo "         elf.fi      : $fi_felder"
        echo "         firn_elf.rs : $rs_felder"
        rc=1
    else
        echo "  [ ok ] Segmentaufbau stimmt ueberein: $fi_felder"
        local probe
        probe=$(sed -n '/^struct ElfSegment {/,/^}/p' kernel/firn/elf.fi \
                | sed 's/^    flags: u64,/    flags_FALSCH: u64,/' \
                | grep -oE '^\s+[a-z_]+:' | tr -d ' :' | tr '\n' ' ')
        if [[ "$probe" == "$rs_felder" ]]; then
            echo "  [FEHL] die Aufbaupruefung schlaegt bei einer Abweichung NICHT an"
            rc=1
        else
            echo "  [ ok ] Gegenprobe: ein umbenanntes Feld laesst die Pruefung fallen"
        fi
    fi

    # 5) Die Symbole muessen wirklich im Kernelabbild stehen.
    local n
    n=$(grep -cE ' T elf_(parse|max_segments)$' build/osum.map || true)
    if [[ "$n" -ne 2 ]]; then
        echo "  [FEHL] im Kernelabbild stehen $n von 2 Firn-ELF-Symbolen"
        rc=1
    else
        echo "  [ ok ] beide Firn-Symbole stehen im Kernelabbild"
    fi

    # 6) Und der Rust-Pruefteil muss wirklich weg sein, nicht nur ungenutzt.
    if awk '/>>> PRUEFTEIL ANFANG/{d=1} /<<< PRUEFTEIL ENDE/{d=0} d' \
            kernel/src/kcore/elf.rs | grep -q 'fn parse'; then
        echo "  [FEHL] kernel/src/kcore/elf.rs enthaelt noch einen eigenen Parser"
        rc=1
    else
        echo "  [ ok ] der Rust-Pruefteil ist ausgebaut, es gibt nur noch einen"
    fi

    return $rc
}
run firn_elf
