# SPDX-License-Identifier: GPL-2.0-only
# tests/step-10-marken.sh - sourced by test.sh, never started directly.

PROBE_NAME="Probe-Distro"
PROBE_SLUG="zzprobe"
PROBE_FILE="brands/${PROBE_SLUG}.toml"

brand_names_in_scripts() {
    local f name
    for f in brands/*.toml; do
        name=$(brand_field "$f" os-name)
        [[ -n "$name" ]] || continue
        grep -n "$name" build.sh run-osum.sh brand.sh vendor/osum/hole-osum.sh \
            | grep -vE '^\S+:[0-9]+:[[:space:]]*#' || true
    done
}

scan_build_output() {
    local pattern="$1" root="${2:-build}" hits
    hits=$(grep -rail -e "$pattern" "$root/isoroot" "$root/${PROBE_SLUG}-userland.img" 2>/dev/null || true)
    printf '%s' "$hits"
}

step "Brands: one source tree, several products"
brands_check() {
    RC=0
    local n=0 slugs="" f on sl
    shopt -s nullglob
    for f in brands/*.toml; do
        [[ "$f" == "$PROBE_FILE" ]] && continue
        n=$((n+1))
        on=$(brand_field "$f" os-name); sl=$(brand_field "$f" slug)
        if [[ -z "$on" || -z "$sl" ]]; then
            nok "$f: os-name or slug missing"; continue
        fi
        if [[ ! "$sl" =~ ^[a-z][a-z0-9-]*$ ]]; then
            nok "$f: slug \"$sl\" is not usable as a file name"; continue
        fi
        case " $slugs " in *" $sl "*) nok "duplicate slug: $sl"; continue ;; esac
        slugs="$slugs $sl"
        ok "$f -> $on ($sl)"
    done
    [[ $n -ge 2 ]] && ok "$n brands - more than one, otherwise this measures nothing" \
                   || nok "only $n brand file(s)"

    local std=""
    if [[ -f brands/STANDARD ]]; then
        std=$(tr -d '[:space:]' < brands/STANDARD)
        if [[ -f "brands/$std.toml" ]]; then
            ok "brands/STANDARD names \"$std\" and the file exists"
        else
            nok "brands/STANDARD names \"$std\" but brands/$std.toml is missing"
        fi
        if [[ "$std" == "xoffi" && "$(brand_field brands/xoffi.toml os-name)" == "XoffiOS" ]]; then
            ok "the default brand is XoffiOS (xoffi)"
        else
            nok "the default brand is not XoffiOS (xoffi)"
        fi
    else
        nok "brands/STANDARD is missing"
    fi

    if [[ -n "$(brand_names_in_scripts)" ]]; then
        brand_names_in_scripts
        nok "a brand name is hardcoded in a build script (it belongs in brands/)"
    else
        ok "build.sh, run-osum.sh, brand.sh and hole-osum.sh contain no brand name"
    fi

    local mapped
    mapped=$(BRAND=xoffi bash -c 'source ./brand.sh; printf "%s|%s|%s|%s|%s" "$OSUM_MARKE_PRODUKT" "$OSUM_MARKE_KURZ" "$OSUM_MARKE_HERSTELLER" "$OSUM_MARKE_WEB" "$OSUM_MARKE_FEED"')
    if [[ "$mapped" == "XoffiOS|xoffi|FleiTec|https://xoffi.fleitec.com|https://xoffi.fleitec.com/pakete" ]]; then
        ok "brand.sh maps brands/xoffi.toml onto OSUM_MARKE_* (one value store)"
    else
        nok "OSUM_MARKE_* mapping wrong: $mapped"
    fi
    mapped=$(BRAND=orientos OSUM_MARKE_PRODUKT=Stale bash -c 'source ./brand.sh; printf "%s" "$OSUM_MARKE_PRODUKT"')
    [[ "$mapped" == "OrientOS" ]] && ok "the brand file beats a stale OSUM_MARKE_PRODUKT in the environment" \
                                  || nok "a stale OSUM_MARKE_PRODUKT leaked through: $mapped"

    local second zslug zname
    second=$(ls brands/*.toml | xargs -n1 basename | sed 's/\.toml$//' \
             | grep -v -e "^$BRAND$" -e "^$PROBE_SLUG$" | head -1)
    if [[ -n "$second" ]]; then
        zslug=$(brand_field "brands/$second.toml" slug)
        zname=$(brand_field "brands/$second.toml" os-name)
        if ./build.sh --brand "$second" >/dev/null 2>&1; then
            ok "the second brand ($second) builds"
        else
            nok "./build.sh --brand $second failed"
        fi
        if [[ -s "build/$zslug.iso" ]]; then
            ok "build/$zslug.iso exists ($(( $(stat -c%s "build/$zslug.iso") / 1024 )) KiB)"
        else
            nok "build/$zslug.iso is missing"
        fi
        if grep -qF "/$zname" build/isoroot/boot/limine/limine.conf; then
            ok "its boot menu carries the other product name ($zname)"
        else
            nok "limine.conf of the second brand does not name $zname"
        fi
        if [[ -f build/isoroot/boot/osum ]]; then
            ok "the kernel is called osum in both brands"
        else
            nok "the image of the second brand has no boot/osum"
        fi
    else
        nok "there is no second brand to cross-check"
    fi

    if ./build.sh --brand doesnotexist >/dev/null 2>&1; then
        nok "an invented brand builds - a typo falls back silently to the default brand"
    else
        ok "counter-proof: an invented brand aborts instead of using the default brand"
    fi

    return $RC
}
run brands_check

step "Brands: counter-proof, no known brand name in the built output"
brand_probe_check() {
    cat > "$PROBE_FILE" <<EOF
os-name   = "$PROBE_NAME"
slug      = "$PROBE_SLUG"
publisher = "Probe Corp"
web       = "https://probe.example"
feed      = "https://probe.example/packages"
EOF
    brand_probe_body
    local rc=$?
    rm -f "$PROBE_FILE"
    return $rc
}

brand_probe_body() {
    RC=0
    if ./build.sh --brand "$PROBE_SLUG" >/dev/null 2>&1; then
        ok "the probe brand builds"
    else
        nok "./build.sh --brand $PROBE_SLUG failed"
        return $RC
    fi
    if grep -qF "/$PROBE_NAME" build/isoroot/boot/limine/limine.conf; then
        ok "the probe boot menu carries $PROBE_NAME"
    else
        nok "the probe boot menu does not carry $PROBE_NAME"
    fi
    if grep -qa "$PROBE_NAME" build/isoroot/boot/osum; then
        ok "the probe kernel image carries $PROBE_NAME (the brand reaches the kernel)"
    else
        nok "the probe kernel image does not carry $PROBE_NAME"
    fi
    local f name slug hits
    for f in brands/*.toml; do
        [[ "$f" == "$PROBE_FILE" ]] && continue
        name=$(brand_field "$f" os-name)
        slug=$(brand_field "$f" slug)
        hits=$(scan_build_output "$name")
        if [[ -z "$hits" ]]; then
            ok "no \"$name\" anywhere in the probe build output"
        else
            echo "$hits" | sed 's/^/        /'
            nok "\"$name\" found in the probe build output"
        fi
        hits=$(scan_build_output "$slug")
        if [[ -z "$hits" ]]; then
            ok "no \"$slug\" anywhere in the probe build output"
        else
            echo "$hits" | sed 's/^/        /'
            nok "\"$slug\" found in the probe build output"
        fi
    done
    return $RC
}
run brand_probe_check

step "Brands: the default brand builds again"
brand_restore_check() {
    RC=0
    if ./build.sh >/dev/null 2>&1; then
        ok "the default brand builds again"
    else
        nok "the default brand does not build again"
    fi
    return $RC
}
run brand_restore_check
