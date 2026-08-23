# tests/step-22-firn-bitmap.sh — wird von test.sh gesourct, nicht direkt
# gestartet.
#
# Der Bitmap-Rahmenverwalter liegt seit dem 23.08.2026 in Firn
# (kernel/firn/bitmap.fi). Damit ist er dem `cargo test` entzogen: die
# Rust-Fassung mit ihren 23 Testfaellen ist ausgebaut.
#
# Ersatzlos waere das ein Rueckschritt — ein Verwalter physischen Speichers
# ohne Tests ist nicht vertrauenswuerdig. Deshalb stellt tests/firn-bitmap/
# dem Firn-Objekt GENAU DIESELBEN 23 Fragen, und zwar gegen dasselbe Objekt,
# das im Kernelabbild landet (Kernelprofil, freistehend), nicht gegen eine
# host-taugliche Zweitfassung.
#
# Zusaetzlich wird hier geprueft, dass der Verwaltungsblock auf beiden Seiten
# denselben Aufbau hat. Weicht er ab, liest Rust Muell aus der Struktur — und
# zwar ohne dass irgendetwas abstuerzt.
step "Firn-Bitmap: 23 Faelle und gleicher Strukturaufbau"
firn_bitmap() {
    ./tests/firn-bitmap/lauf.sh || return 1

    # Der Verwaltungsblock: vier u64 in fester Reihenfolge. Die Rust-Seite
    # (kernel/src/mm/firn_bitmap.rs) und die Firn-Seite (kernel/firn/bitmap.fi)
    # muessen sich einig sein.
    local fi_felder rs_felder
    fi_felder=$(sed -n '/^struct Bitmap {/,/^}/p' kernel/firn/bitmap.fi \
                | grep -oE '^\s+[a-z_]+:' | tr -d ' :' | tr '\n' ' ')
    rs_felder=$(sed -n '/^struct FirnBitmap {/,/^}/p' kernel/src/mm/firn_bitmap.rs \
                | grep -oE '^\s+[a-z_]+:' | tr -d ' :' | tr '\n' ' ')
    if [[ "$fi_felder" != "$rs_felder" ]]; then
        echo "  [FEHL] Strukturaufbau weicht ab:"
        echo "         bitmap.fi      : $fi_felder"
        echo "         firn_bitmap.rs : $rs_felder"
        return 1
    fi
    echo "  [ ok ] Verwaltungsblock stimmt ueberein: $fi_felder"

    # Gegenprobe: die Pruefung muss auch wirklich anschlagen.
    local tmp
    tmp=$(mktemp) || return 1
    sed 's/^    cursor: u64,/    cursor_FALSCH: u64,/' kernel/firn/bitmap.fi > "$tmp"
    local probe
    probe=$(sed -n '/^struct Bitmap {/,/^}/p' "$tmp" \
            | grep -oE '^\s+[a-z_]+:' | tr -d ' :' | tr '\n' ' ')
    rm -f "$tmp"
    if [[ "$probe" == "$rs_felder" ]]; then
        echo "  [FEHL] die Strukturpruefung schlaegt bei einer Abweichung NICHT an"
        return 1
    fi
    echo "  [ ok ] Gegenprobe: ein umbenanntes Feld laesst die Pruefung fallen"

    # Das Objekt im Kernelabbild muss die Firn-Symbole wirklich enthalten --
    # sonst haette der Linker still eine andere Fassung genommen.
    local n
    n=$(grep -cE ' T bm_(words_needed|init|frames|used_frames|free_frames|is_used|free_range|reserve_range|alloc|alloc_contiguous|free)$' build/osum.map || true)
    if [[ "$n" -ne 11 ]]; then
        echo "  [FEHL] im Kernelabbild stehen $n von 11 Firn-Bitmap-Symbolen"
        return 1
    fi
    echo "  [ ok ] alle 11 Firn-Symbole stehen im Kernelabbild"

    # Und die Rust-Fassung muss wirklich weg sein, nicht nur ungenutzt.
    if [[ -e libs/osum-mem/src/bitmap.rs ]]; then
        echo "  [FEHL] libs/osum-mem/src/bitmap.rs existiert noch — zwei Fassungen"
        return 1
    fi
    echo "  [ ok ] die Rust-Fassung ist ausgebaut, es gibt nur noch eine"
    return 0
}
run firn_bitmap
