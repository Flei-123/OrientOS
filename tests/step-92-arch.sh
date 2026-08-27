# SPDX-License-Identifier: GPL-2.0-only
# tests/step-92-arch.sh -- sourced by test.sh, not started on its own.
#
# TWO MACHINES. Firn, Osum and OrientOS are to run on x86-64 and on
# AArch64. Firn's round 80 already compiles the same source for both;
# round OSUM-ARM is putting an architecture seam through the kernel. The
# question for the package manager was: WHAT DOES A HASH MEAN NOW?
#
# The answer is that it means what it meant -- machine code for another
# machine is other content, so it is another package with another hash.
# Nothing had to be invented. What had to be built is the part that makes
# that fact REFUSABLE, because the worst outcome is not a refusal, it is
# a device that installs the wrong binaries quietly and dies at the first
# instruction with nothing to read.
#
# THREE THINGS ARE MEASURED HERE.
#
#   1. THAT ONE SOURCE REALLY BECOMES TWO PACKAGES. Not a byte flipped in
#      a header: `firnc --target=` twice, two ELF files that both RUN and
#      both print the same thing, and two different hashes. If the
#      toolchain is not on this host the step falls back to a crafted ELF
#      header and SAYS SO in the assertion text -- a measurement that
#      does not say how it was taken is worth nothing.
#
#   2. THAT WHAT HAS NO MACHINE CODE IN IT IS SHARED. A colour scheme and
#      a wallpaper get `arch=any` because there was no ELF header to
#      read, and `any` is the same octets everywhere, so the hash is the
#      same on both machines and the store holds it ONCE.
#
#   3. THAT THE WRONG ONE IS REFUSED BY NAME. Six different doors, each
#      with its counter-check: the package, the kernel, the rebuild, the
#      dependency, the fat package, and the source that offers a choice
#      nobody made.
step "Two machines: one source, two hashes -- and the wrong binary refused by name, not installed"
arch_check() {
    RC=0
    local O="python3 pkg/opk.py"
    local T; T=$(mktemp -d "${TMPDIR:-/tmp}/orientos-arch-XXXXXX")
    local FIRNC=../firn/compiler/target/release/firnc

    mkdir -p "$T/bx" "$T/ba" "$T/q" "$T/hostx" "$T/hosta"

    # ---------------------------------------------- 1. one source, two machines
    #
    # The strong form of this needs a compiler that can aim at both
    # machines. Firn has one. If it is not built on this host, an
    # AArch64 ELF is crafted from the x86-64 one by writing the machine
    # number into the header -- that still measures the DETECTION, which
    # is what the package manager does, but it does not measure that the
    # two binaries behave alike. Which of the two happened is part of the
    # assertion.
    local HOW="crafted ELF header (no cross toolchain on this host)"
    local BOTH_RUN=0
    if [[ -x $FIRNC && -f ../firn/tests/048_print_number.fi ]] \
       && $FIRNC --target=x86_64-linux  -o "$T/bx/twin" ../firn/tests/048_print_number.fi >/dev/null 2>&1 \
       && $FIRNC --target=aarch64-linux -o "$T/ba/twin" ../firn/tests/048_print_number.fi >/dev/null 2>&1; then
        HOW="firnc --target=, the same source through the same compiler"
        if command -v qemu-aarch64 >/dev/null 2>&1; then
            local ox oa
            ox=$("$T/bx/twin" 2>/dev/null); local rx=$?
            oa=$(qemu-aarch64 "$T/ba/twin" 2>/dev/null); local ra=$?
            if [[ "$ox" == "$oa" && $rx -eq 0 && $ra -eq 0 && -n "$ox" ]]; then
                BOTH_RUN=1
                ok "the same source runs on both machines and prints the same thing ('$ox', exit $rx/$ra)"
            else
                nok "the two builds do not behave alike: x86='$ox'($rx) arm='$oa'($ra)"
            fi
        fi
    else
        cp vendor/osum/bin/cat "$T/bx/twin"
        cp vendor/osum/bin/cat "$T/ba/twin"
        # e_machine, two octets at offset 18: 183 = EM_AARCH64.
        printf '\267\000' | dd of="$T/ba/twin" bs=1 seek=18 conv=notrunc status=none
    fi
    [[ $BOTH_RUN -eq 1 ]] || ok "SKIPPED: no qemu-aarch64 on this host, so behaviour was not compared ($HOW)"

    local mx ma
    mx=$(python3 -c "import sys;b=open(sys.argv[1],'rb').read(20);print(int.from_bytes(b[18:20],'little'))" "$T/bx/twin")
    ma=$(python3 -c "import sys;b=open(sys.argv[1],'rb').read(20);print(int.from_bytes(b[18:20],'little'))" "$T/ba/twin")
    [[ "$mx" == "62" && "$ma" == "183" ]] \
        && ok "two ELF files, e_machine 62 (EM_X86_64) and 183 (EM_AARCH64), from $HOW" \
        || nok "the two files are not the two machines (e_machine $mx and $ma)"

    # ------------------------------------ 2. two packages, two hashes
    #
    # AND NOBODY TYPED THE ARCHITECTURE. The two recipes differ in the
    # path of one file and in nothing else -- `bauen` reads the machine
    # out of the octets it is packing.
    rez() { printf 'name=twin\nfassung=1.0.0\ntitel=twin\ninfo=one source, two machines\nkeys=twin\nhandle=konsole\ndatei=start %s\n' "$2" > "$1"; }
    rez "$T/x.rezept" "$T/bx/twin"
    rez "$T/a.rezept" "$T/ba/twin"
    grep -v '^datei=' "$T/x.rezept" > "$T/x.head"; grep -v '^datei=' "$T/a.rezept" > "$T/a.head"
    cmp -s "$T/x.head" "$T/a.head" \
        && ok "the two recipes are identical except for the path of the binary -- nothing declares a machine" \
        || nok "the two recipes differ somewhere other than the file"

    $O bauen "$T/x.rezept" -o "$T/q/twin-x86_64.opk"  > "$T/bx.log" 2>&1
    $O bauen "$T/a.rezept" -o "$T/q/twin-aarch64.opk" > "$T/ba.log" 2>&1
    grep -q ' x86_64 '  "$T/bx.log" && ok "the x86-64 build was MEASURED as x86_64: $(tr -s ' ' < "$T/bx.log" | cut -d' ' -f1-3)" \
                                    || nok "the x86-64 build was not measured as x86_64: $(cat "$T/bx.log")"
    grep -q ' aarch64 ' "$T/ba.log" && ok "the aarch64 build was MEASURED as aarch64: $(tr -s ' ' < "$T/ba.log" | cut -d' ' -f1-3)" \
                                    || nok "the aarch64 build was not measured as aarch64: $(cat "$T/ba.log")"
    local hx ha
    hx=$($O zeigen "$T/q/twin-x86_64.opk"  | grep -m1 '^hash' | awk '{print $2}')
    ha=$($O zeigen "$T/q/twin-aarch64.opk" | grep -m1 '^hash' | awk '{print $2}')
    [[ -n "$hx" && "$hx" != "$ha" ]] \
        && ok "two packages, two hashes: ${hx:0:12} and ${ha:0:12} -- the identity of a package is still its content" \
        || nok "the two builds have the same hash ($hx) -- then a hash names two things"

    # GEGENPROBE: a recipe that LIES about the machine is refused, and it
    # is the octets that win.
    printf 'name=liar\nfassung=1.0.0\ntitel=liar\ninfo=x\nkeys=liar\narch=aarch64\nhandle=konsole\ndatei=start %s\n' "$T/bx/twin" > "$T/liar.rezept"
    if $O bauen "$T/liar.rezept" -o "$T/liar.opk" > "$T/liar.log" 2>&1; then
        nok "a recipe that says aarch64 over an x86-64 binary was built anyway"
    else
        ok "GEGENPROBE: a recipe that lies -> $(grep -m1 -o 'says arch=.*' "$T/liar.log" | cut -c1-52)"
    fi

    # GEGENPROBE: THE FAT PACKAGE. macOS puts both machines in one file;
    # this refuses to, and the reason is the hash.
    printf 'name=fat\nfassung=1.0.0\ntitel=fat\ninfo=x\nkeys=fat\nhandle=konsole\ndatei=start %s\ndatei=start2 %s\n' "$T/bx/twin" "$T/ba/twin" > "$T/fat.rezept"
    if $O bauen "$T/fat.rezept" -o "$T/fat.opk" > "$T/fat.log" 2>&1; then
        nok "a package with both machines in it was built -- its hash would name two things"
    else
        ok "GEGENPROBE: the fat package -> $(grep -m1 -o 'machine code for [0-9]* different machines ([^)]*)' "$T/fat.log")"
    fi

    # ------------------------------- 3. what has no machine code is SHARED
    #
    # Two build hosts, the same input, and the claim that has to hold for
    # the sharing to be real: the same hash.
    python3 pkg/osym.py userland/wallpapers/deep.wallpaper "$T/deep.osym" >/dev/null 2>&1
    $O asset-pack "$T/deep.osym" --class wallpaper --name shared -o "$T/hostx/shared.opk" >/dev/null 2>&1
    $O asset-pack "$T/deep.osym" --class wallpaper --name shared -o "$T/hosta/shared.opk" >/dev/null 2>&1
    if cmp -s "$T/hostx/shared.opk" "$T/hosta/shared.opk"; then
        ok "a wallpaper built on two machines is IDENTICAL, $(stat -c%s "$T/hostx/shared.opk") octets -- one store entry serves both"
    else
        nok "the same wallpaper produced two different packages"
    fi
    $O zeigen "$T/hostx/shared.opk" | grep -q '^arch     any' \
        && ok "and it says arch=any, which nobody typed either -- there was no ELF header to read" \
        || nok "the wallpaper did not come out as arch=any"
    # GEGENPROBE: an `asset` that is secretly a program is refused --
    # otherwise machine code could enter as content and skip every check
    # on this page.
    if $O asset-pack "$T/bx/twin" --class wallpaper --name sneak -o "$T/sneak.opk" > "$T/sneak.log" 2>&1; then
        nok "a program was packed as a wallpaper -- machine code would then travel unchecked"
    else
        ok "GEGENPROBE: a program packed as a wallpaper -> $(grep -m1 -o 'holds machine code.*refused' "$T/sneak.log" | cut -c1-48)"
    fi
    cp "$T/hostx/shared.opk" "$T/q/shared.opk"

    # ------------------------------------------- 4. one source, both machines
    $O schluessel "$T/keys" >/dev/null 2>&1
    $O quelle "$T/q" --schluessel "$T/keys/geheim.key" >/dev/null 2>&1
    local PK; PK=$(python3 -c "print(open('$T/keys/oeffentlich.key','rb').read().hex())")
    local n_ix; n_ix=$(awk -F'\t' 'NF==6' "$T/q/INDEX" | wc -l)
    [[ "$n_ix" == "3" ]] \
        && ok "the INDEX carries the architecture in a sixth column, 3 entries: $(awk -F'\t' '{printf "%s=%s ",$1,$6}' "$T/q/INDEX")" \
        || nok "the INDEX does not have six columns everywhere ($n_ix of 3)"
    # GEGENPROBE: an old five-column INDEX is still read. The rule for
    # that is one line long, like the one for old PLAN files.
    mkdir -p "$T/old"; cp "$T/q"/*.opk "$T/old/" 2>/dev/null
    cut -f1-5 "$T/q/INDEX" > "$T/old/INDEX"
    python3 - "$T/old" >/dev/null 2>&1 <<'PY' && ok "GEGENPROBE: an INDEX from before this round is still read, and its entries simply say nothing about machines" || nok "an old five-column INDEX can no longer be read"
import sys
sys.path.insert(0, "pkg")
from opk import quelle_lesen
e, s = quelle_lesen(sys.argv[1])
assert len(e) >= 1, e
PY

    # ------------------------------------- 5. a system that says what it is
    local X="$T/x86"
    $O arch --root "$X" x86_64 >/dev/null 2>&1
    grep -q '^arch	x86_64$' "$X/system/generations/1/PLAN" \
        && ok "the machine is ONE line in the plan: $(grep -m1 '^arch' "$X/system/generations/1/PLAN" | tr '\t' ' ')" \
        || nok "the arch line is not in the plan of the new generation"
    LC_ALL=C sort -c "$X/system/generations/1/PLAN" \
        && ok "and the file is still what plain \`sort\` produces -- the arch line did not break the canonical order" \
        || nok "the PLAN is no longer sorted after the arch line was added"
    $O source-add --root "$X" "file://$T/q" "$PK" >/dev/null 2>&1
    $O installieren --root "$X" --quelle "$T/q" --schluessel "$T/keys/oeffentlich.key" twin > "$T/ix.log" 2>&1
    local got
    got=$(python3 -c "import sys;b=open(sys.argv[1],'rb').read(20);print(int.from_bytes(b[18:20],'little'))" "$X/apps/twin.prog/start" 2>/dev/null)
    [[ "$got" == "62" ]] \
        && ok "from a source holding BOTH builds, the x86-64 machine got the x86-64 one (/apps/twin.prog/start is EM_X86_64)" \
        || nok "the x86-64 machine got e_machine $got"

    # GEGENPROBE, AND IT IS THE POINT OF THE WHOLE STEP: the other build
    # is refused BY NAME rather than installed quietly.
    if $O installieren --root "$X" "$T/q/twin-aarch64.opk" > "$T/refuse.log" 2>&1; then
        nok "the aarch64 build was installed on an x86-64 system"
    else
        ok "GEGENPROBE: the aarch64 build on x86-64 -> $(grep -m1 -o 'twin is for aarch64, this system is x86_64 -- REFUSED' "$T/refuse.log")"
    fi
    # GEGENPROBE: a package from before this round says nothing about
    # machines, and on a system that DOES know what it is that is not a
    # detail to wave through.
    python3 - "$T/q/twin-x86_64.opk" "$T/mute.opk" >/dev/null 2>&1 <<'PY'
import sys
sys.path.insert(0, "pkg")
from opk import paket_lesen, paket_packen, meta_lesen, meta_bauen
meta, daten, h = paket_lesen(open(sys.argv[1], "rb").read())
felder, braucht, handles = meta_lesen(meta)
del felder["arch"]
roh, _ = paket_packen(meta_bauen(felder, braucht, handles), daten)
open(sys.argv[2], "wb").write(roh)
PY
    if $O installieren --root "$X" "$T/mute.opk" > "$T/mute.log" 2>&1; then
        nok "an unlabelled package was installed on a system that states its machine"
    else
        ok "GEGENPROBE: an unlabelled package -> $(grep -m1 -o 'does not say which machine it is for' "$T/mute.log")"
    fi

    # GEGENPROBE: a machine that does NOT say what it is, asking a source
    # that offers a choice. Nothing is picked.
    if $O installieren --root "$T/mutesys" --quelle "$T/q" --schluessel "$T/keys/oeffentlich.key" twin > "$T/amb.log" 2>&1; then
        nok "a machine that states nothing was given one of two builds at random"
    else
        ok "GEGENPROBE: two builds, no machine stated -> $(grep -m1 -o 'nothing said which machine this is for' "$T/amb.log")"
    fi

    # GEGENPROBE: the word cannot be changed to make a wrong tree right.
    if $O arch --root "$X" aarch64 > "$T/switch.log" 2>&1; then
        nok "a system full of x86-64 binaries was allowed to call itself aarch64"
    else
        ok "GEGENPROBE: renaming the machine -> $(grep -m1 -o 'cannot be called aarch64.*not for it' "$T/switch.log" | cut -c1-56)"
    fi

    # -------------------------------------------- 6. the aarch64 system
    local A="$T/arm"
    $O arch --root "$A" aarch64 >/dev/null 2>&1
    $O source-add --root "$A" "file://$T/q" "$PK" >/dev/null 2>&1
    $O installieren --root "$A" --quelle "$T/q" --schluessel "$T/keys/oeffentlich.key" twin >/dev/null 2>&1
    $O pref --root "$A" justin wallpaper "$T/hostx/shared.opk" >/dev/null 2>&1
    $O export --root "$A" -o "$T/arm.plan" >/dev/null 2>&1
    $O export --root "$X" -o "$T/x86.plan" >/dev/null 2>&1
    # THE PLAN IS STILL TEXT AND STILL READABLE WITH `cat`.
    LC_ALL=C grep -qP '[^\x20-\x7e\t\n]' "$T/arm.plan" \
        && nok "the plan of a two-machine world is no longer printable text" \
        || ok "the aarch64 plan is $(wc -l < "$T/arm.plan") printable lines, $(stat -c%s "$T/arm.plan") octets -- \`cat\` still reads the whole state"
    local d_app d_arch
    d_app=$(diff "$T/x86.plan" "$T/arm.plan" | grep -c '^[<>] app')
    d_arch=$(diff "$T/x86.plan" "$T/arm.plan" | grep -c '^[<>] arch')
    [[ "$d_app" == "2" && "$d_arch" == "2" ]] \
        && ok "the two plans differ in exactly the lines that name machine code (2) plus the one that names the machine (2)" \
        || nok "the two plans differ in $d_app app line(s) and $d_arch arch line(s), expected 2 and 2"

    # ------------------------- 7. rebuild from zero, on the other machine
    local N="$T/new"
    $O rebuild --root "$N" --plan "$T/arm.plan" --source "$T/q" > "$T/rb.log" 2>&1
    got=$(python3 -c "import sys;b=open(sys.argv[1],'rb').read(20);print(int.from_bytes(b[18:20],'little'))" "$N/apps/twin.prog/start" 2>/dev/null)
    [[ "$got" == "183" ]] \
        && ok "an aarch64 system rebuilt from its plan on an EMPTY root gets ARM binaries ($(grep -m1 -o '[0-9]* package(s) fetched and verified, [0-9]* octet(s)' "$T/rb.log"))" \
        || nok "the rebuilt aarch64 tree has e_machine $got in /apps/twin.prog/start"
    grep -q 'checked against arch aarch64' "$T/rb.log" \
        && ok "and every fetched package was checked against the machine before it entered the store: $(grep -m1 -o '[0-9]* octet(s) of that is .any.' "$T/rb.log")" \
        || nok "the rebuild did not report checking the packages against the machine"

    # GEGENPROBE: a hand-edited plan that claims one machine and names
    # the build of the other. This is the case that must not half-build.
    sed "s/^app\ttwin\t.*/app\ttwin\t$hx/" "$T/arm.plan" > "$T/lie.plan"
    if $O rebuild --root "$T/lied" --plan "$T/lie.plan" --source "$T/q" > "$T/lied.log" 2>&1; then
        nok "a plan that says aarch64 and names the x86-64 build was rebuilt"
    else
        local left; left=$(ls "$T/lied/apps" 2>/dev/null | wc -l)
        [[ "$left" == "0" ]] \
            && ok "GEGENPROBE: a plan that claims aarch64 and names the x86-64 build is refused, and NOTHING was activated ($left in /apps)" \
            || nok "the refused rebuild left $left bundle(s) behind"
    fi

    # ---------------------------------- 8. dependencies stay on one machine
    printf 'name=lib\nfassung=1.0.0\ntitel=lib\ninfo=x\nkeys=lib\nhandle=konsole\ndatei=start %s\n' "$T/bx/twin" > "$T/lib.rezept"
    printf 'name=user\nfassung=1.0.0\ntitel=user\ninfo=x\nkeys=user\nbraucht=lib\nhandle=konsole\ndatei=start %s\n' "$T/ba/twin" > "$T/user.rezept"
    printf 'name=user\nfassung=1.0.0\ntitel=user\ninfo=x\nkeys=user\nbraucht=lib\nhandle=konsole\ndatei=start %s\n' "$T/bx/twin" > "$T/userx.rezept"
    $O bauen "$T/lib.rezept"   -o "$T/lib.opk"   >/dev/null 2>&1
    $O bauen "$T/user.rezept"  -o "$T/user.opk"  >/dev/null 2>&1
    $O bauen "$T/userx.rezept" -o "$T/userx.opk" >/dev/null 2>&1
    mkdir -p "$T/dep"
    $O installieren --root "$T/dep" "$T/lib.opk" >/dev/null 2>&1
    $O installieren --root "$T/dep" "$T/userx.opk" >/dev/null 2>&1 \
        && ok "a dependency INSIDE one machine resolves as it always did (user -> lib, both x86_64)" \
        || nok "a same-machine dependency was refused"
    # GEGENPROBE: the same name, the other machine. The plan of this root
    # states no architecture, so the machine check is out of the way on
    # purpose -- what fires here is the DEPENDENCY check.
    if $O installieren --root "$T/dep" "$T/user.opk" > "$T/dep.log" 2>&1; then
        nok "an aarch64 package resolved its dependency against an x86-64 library"
    else
        ok "GEGENPROBE: across the boundary -> $(grep -m1 -o 'is aarch64 and needs lib, but the lib installed here is x86_64' "$T/dep.log")"
    fi

    # ----------------------------------- 9. one store, both machines
    #
    # Not needed by a device -- needed by the build server. It works for
    # the same reason everything else here works: store entries are named
    # by content, and two machines never produce the same content.
    local B="$T/both"
    $O rebuild --root "$B" --plan "$T/x86.plan" --source "$T/q" >/dev/null 2>&1
    $O rebuild --root "$B" --plan "$T/arm.plan" --source "$T/q" >/dev/null 2>&1
    $O archs --root "$B" > "$T/archs.log" 2>&1
    local n_arch; n_arch=$(grep -cE '^  (x86_64|aarch64) ' "$T/archs.log")
    [[ "$n_arch" == "2" ]] \
        && ok "one store holds both machines side by side: $(grep -m1 -o '[0-9]* machine(s) side by side.*' "$T/archs.log" | cut -c1-46)" \
        || nok "the store does not hold two machines ($n_arch)"
    local n_apps; n_apps=$(ls "$B/apps" | wc -l)
    [[ "$n_apps" == "1" ]] \
        && ok "and the activated view names exactly ONE of them -- a device is one machine ($n_apps bundle in /apps)" \
        || nok "the activated view has $n_apps bundles, expected 1"
    grep -q 'octet(s) that do not exist twice' "$T/archs.log" \
        && ok "the sharing is counted, not claimed: $(grep -m1 -oE '[0-9]+ octet\(s\) that do not exist twice' "$T/archs.log")" \
        || nok "archs does not report what arch-independent packages save"

    # --------------------------------------------------- 10. verify
    $O verify --root "$A" > "$T/ver.log" 2>&1 \
        && ok "verify on the aarch64 tree: $(tail -1 "$T/ver.log")" \
        || nok "verify fails on the aarch64 tree: $(grep -m1 FAILED "$T/ver.log")"
    grep -q 'no dependency crosses the architecture boundary' "$T/ver.log" \
        && ok "and it checks the dependency graph per machine, not only the packages" \
        || nok "verify does not check dependencies across the boundary"
    # GEGENPROBE: an old plan makes no claim about machines, and verify
    # says so instead of inventing one.
    mkdir -p "$T/legacy"; $O installieren --root "$T/legacy" "$T/lib.opk" >/dev/null 2>&1
    $O verify --root "$T/legacy" 2>&1 | grep -q 'states no machine' \
        && ok "GEGENPROBE: a plan without an arch line is read, and verify names it an old plan rather than guessing" \
        || nok "a plan without an arch line is not handled as an old plan"

    # ----------------------------- 11. one list of reserved names, not two
    python3 - >/dev/null 2>&1 <<'PY' && ok "\`arch\` is a reserved package name, and pkg/recipes.py uses THE SAME list as pkg/opk.py (7 names)" || nok "the reserved-name lists of opk.py and recipes.py disagree"
import sys
sys.path.insert(0, "pkg")
import opk, recipes
assert "arch" in opk.PLAN_TYPES, opk.PLAN_TYPES
assert tuple(recipes.RESERVED) == tuple(opk.PLAN_TYPES), (recipes.RESERVED, opk.PLAN_TYPES)
assert len(opk.PLAN_TYPES) == 7, opk.PLAN_TYPES
PY
    printf 'name=arch\nfassung=1.0.0\ntitel=x\ninfo=x\nkeys=x\ndatei=start %s\n' "$T/bx/twin" > "$T/res.rezept"
    if $O bauen "$T/res.rezept" -o "$T/res.opk" > "$T/res.log" 2>&1; then
        nok "a package called 'arch' was built -- an old-shape plan line could not be told apart from it"
    else
        ok "GEGENPROBE: a package called 'arch' -> $(grep -m1 -oE "is one of the [0-9]+ PLAN types" "$T/res.log")"
    fi

    rm -rf "$T"
    return $RC
}
run arch_check
