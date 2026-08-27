# SPDX-License-Identifier: GPL-2.0-only
# tests/step-93-orphans.sh -- sourced by test.sh, not started on its own.
#
# "WHAT ABOUT PROGRAMS THAT ARE NOT IN THE APP STORE, WHEN I SET UP A NEW
# DEVICE AND WANT MY APPLICATIONS BACK?"
#
# That was a real hole and not a detail. A PLAN names `app <name>
# <hash>`; the OCTETS have to come from somewhere; and for a package
# built by hand and installed from a file there was no somewhere.
# `rebuild` said `4 of 39 hashes are in no source` and named none of
# them -- which is the least useful thing it could have said, because
# the one question the owner has at that moment is WHICH ONES.
#
# THE ANSWER HAS THREE PARTS, AND THE FIRST IS NOT MACHINERY.
#
#   1. RUN YOUR OWN SOURCE. A plan may name any number of sources, so
#      the right answer to "I build my own packages" is a directory with
#      an INDEX and a signature -- on a NAS, a server or a memory stick.
#      This step measures that too: adding one makes the orphan
#      disappear.
#
#   2. ORPHANHOOD IS DECIDABLE. A hash is covered if some recorded
#      source has it. Nobody keeps a list, nobody remembers. That is the
#      difference between this and a design where "which of my programs
#      are irreplaceable" is answered from memory.
#
#   3. WHAT IS ORPHANED, AND ONLY THAT, GOES IN THE BACKUP.
#
# THE FOUR CLAIMS THE OWNER ASKED FOR are (a) it is reported as
# orphaned, (b) it lands in the backup, (c) a rebuild on an empty root
# restores it, (d) without the backup a NAMED error comes instead of a
# silent hole. Each is measured below, each with its counter-check.
step "Programs with no source: named, carried in the backup, restored -- and never quietly left out"
orphan_check() {
    RC=0
    local O="python3 pkg/opk.py"
    local T; T=$(mktemp -d "${TMPDIR:-/tmp}/orientos-orph-XXXXXX")
    local FIRNC=../firn/compiler/target/release/firnc

    mkdir -p "$T/appstore" "$T/backup"

    # A public source with two ordinary packages, and one program that
    # the owner built himself and never published.
    if [[ ! -f build/pakete/hallo.opk || ! -f build/pakete/dusk.opk ]]; then
        ./pkg/bauen.sh >/dev/null 2>&1
    fi
    cp build/pakete/hallo.opk build/pakete/dusk.opk "$T/appstore/"
    $O schluessel "$T/keys" >/dev/null 2>&1
    $O quelle "$T/appstore" --schluessel "$T/keys/geheim.key" >/dev/null 2>&1
    local PK; PK=$(python3 -c "print(open('$T/keys/oeffentlich.key','rb').read().hex())")

    # The hand-built program. Firn if it is here, otherwise any binary --
    # what matters is that it exists in no INDEX anywhere.
    if [[ -x $FIRNC && -f ../firn/tests/048_print_number.fi ]] \
       && $FIRNC --target=x86_64-linux -o "$T/mytool" ../firn/tests/048_print_number.fi >/dev/null 2>&1; then
        :
    else
        cp vendor/osum/bin/cat "$T/mytool"
    fi
    printf 'name=mytool\nfassung=1.0.0\ntitel=mytool\ninfo=built by hand, never published\nkeys=mytool\nhandle=konsole\ndatei=start %s\n' "$T/mytool" > "$T/mytool.rezept"
    $O bauen "$T/mytool.rezept" -o "$T/mytool.opk" >/dev/null 2>&1

    # ------------------------------------------------------- the device
    local R="$T/dev"
    $O arch --root "$R" x86_64 >/dev/null 2>&1
    $O source-add --root "$R" "file://$T/appstore" "$PK" >/dev/null 2>&1
    $O installieren --root "$R" --quelle "$T/appstore" --schluessel "$T/keys/oeffentlich.key" hallo >/dev/null 2>&1
    $O installieren --root "$R" "$T/mytool.opk" >/dev/null 2>&1      # by hand
    $O pref --root "$R" justin theme build/pakete/dusk.opk >/dev/null 2>&1
    mkdir -p "$R/users/justin/state/mytool"
    echo "the work of a human being" > "$R/users/justin/state/mytool/notiz"
    $O export --root "$R" -o "$T/dev.plan" >/dev/null 2>&1
    grep -q '^app	mytool	' "$T/dev.plan" \
        && ok "a hand-installed program is in the PLAN like any other: $(grep -m1 '^app	mytool' "$T/dev.plan" | cut -c1-38)..." \
        || nok "the hand-installed program is not in the plan"

    # ------------------------------------ (a) it is reported as orphaned
    $O orphans --root "$R" > "$T/orph.log" 2>&1
    grep -q '^  ORPHANED mytool' "$T/orph.log" \
        && ok "(a) orphans names it: $(grep -m1 -oE 'ORPHANED mytool +[0-9a-f]{12} +[a-z0-9_]+ +[0-9]+ octet\(s\) in the store' "$T/orph.log")" \
        || nok "(a) orphans does not report mytool as orphaned"
    grep -qE '^2 of 3 covered by a source, 1 ORPHANED' "$T/orph.log" \
        && ok "and it counts: $(grep -m1 -E 'covered by a source' "$T/orph.log")" \
        || nok "the count is wrong: $(grep -m1 -E 'covered by' "$T/orph.log")"
    # GEGENPROBE: the packages that DO have a source are not reported.
    grep -q '^  ok       hallo' "$T/orph.log" && grep -q '^  ok       the theme of justin' "$T/orph.log" \
        && ok "GEGENPROBE: the two packages WITH a source are listed as ok, and the wallpaper-class asset is named in words ('the theme of justin')" \
        || nok "a package that has a source was reported wrongly"
    # GEGENPROBE: --strict is the form a cron job needs.
    $O orphans --root "$R" --strict >/dev/null 2>&1
    [[ $? -eq 1 ]] \
        && ok "GEGENPROBE: --strict exits 1 while anything is orphaned, so this can be a nightly check and not a habit" \
        || nok "--strict does not signal the orphan through the exit code"

    # ---------------------------------------- (b) it lands in the backup
    $O backup-set --root "$R" -o "$T/backup/SET" >/dev/null 2>&1
    local n_pkg; n_pkg=$(grep -c '^package	' "$T/backup/SET")
    [[ "$n_pkg" == "1" ]] \
        && ok "(b) the backup set has exactly ONE package line: $(grep -m1 '^package' "$T/backup/SET" | cut -f2,3,4 | tr '\t' ' ' | cut -c1-40)" \
        || nok "(b) the backup set has $n_pkg package lines, expected 1"
    grep -q '^plan	system/generations/' "$T/backup/SET" \
        && grep -q '^tree	users/justin/state$' "$T/backup/SET" \
        && grep -q '^tree	users/justin/config$' "$T/backup/SET" \
        && ok "and the rest of the list is the old rule: the PLAN, config/ and state/ -- $(grep -c . "$T/backup/SET") lines a backup program can read without knowing what a plan is" \
        || nok "the backup set does not carry plan, config and state"
    # GEGENPROBE: the covered package is NOT carried. That is the point --
    # 99 % of the octets stay out of the archive.
    grep -q "hallo" "$T/backup/SET" \
        && nok "a package that has a source was put in the backup set" \
        || ok "GEGENPROBE: hallo is NOT in the backup set -- it has a source, so carrying it would be archiving an output"
    grep -q '^tree	users/justin/cache' "$T/backup/SET" \
        && nok "cache/ is in the backup set" \
        || ok "GEGENPROBE: no cache/ line -- the never-backed-up rule survived the new exception"

    $O vault-export --root "$R" -o "$T/backup/vault" > "$T/vault.log" 2>&1
    local n_v; n_v=$(ls "$T/backup/vault"/*.opk 2>/dev/null | wc -l)
    [[ "$n_v" == "1" ]] \
        && ok "the octets themselves are written out as one .opk: $(grep -m1 -oE '[0-9]+ orphaned package\(s\), [0-9]+ octet\(s\)' "$T/vault.log")" \
        || nok "vault-export wrote $n_v files, expected 1"
    # The vault entry is repacked from the UNPACKED store, so the claim
    # that has to hold is that it is the same octets again.
    local vh; vh=$(sha256sum "$T/backup/vault"/*.opk | cut -d' ' -f1)
    local oh; oh=$(sha256sum "$T/mytool.opk" | cut -d' ' -f1)
    [[ "$vh" == "$oh" ]] \
        && ok "and it is the ORIGINAL file again, octet for octet -- the store keeps packages unpacked, and repacking is deterministic" \
        || nok "the vault copy is not the package that was installed ($vh vs $oh)"

    # --------------------------- (c) a rebuild on an empty root restores it
    cp "$T/dev.plan" "$T/backup/PLAN"
    $O rebuild --root "$T/new" --plan "$T/backup/PLAN" --vault "$T/backup/vault" > "$T/rb.log" 2>&1
    [[ -f "$T/new/apps/mytool.prog/start" ]] \
        && ok "(c) rebuilt on an EMPTY root, and mytool is back: $(grep -m1 -oE '[0-9]+ package\(s\) fetched and verified, [0-9]+ octet\(s\), [0-9]+ of them from the vault' "$T/rb.log")" \
        || nok "(c) the rebuild did not restore mytool"
    local b1 b2
    b1=$(sha256sum "$R/apps/mytool.prog/start" | cut -d' ' -f1)
    b2=$(sha256sum "$T/new/apps/mytool.prog/start" | cut -d' ' -f1)
    [[ "$b1" == "$b2" && -n "$b1" ]] \
        && ok "and it is the same binary, not a similar one (${b1:0:16})" \
        || nok "the restored binary differs from the original"
    # THE WHOLE TREE, not just the one file.
    $O snapshot --root "$R"     2>/dev/null | grep -v '^#' > "$T/s1"
    $O snapshot --root "$T/new" 2>/dev/null | grep -v '^#' > "$T/s2"
    if diff -q "$T/s1" "$T/s2" >/dev/null; then
        ok "and the WHOLE tree matches, entry for entry: $(wc -l < "$T/s1") entries compared (kind, path, mode, size, SHA-256 of every content)"
    else
        nok "the restored tree differs in $(diff "$T/s1" "$T/s2" | grep -c '^[<>]') entries"
    fi
    $O verify --root "$T/new" >/dev/null 2>&1 \
        && ok "verify on the restored tree passes, and says nothing was left out" \
        || nok "verify fails on the restored tree"

    # ------------------------- (d) without the backup: a NAME, not a hole
    if $O rebuild --root "$T/nogap" --plan "$T/backup/PLAN" > "$T/gap.log" 2>&1; then
        nok "(d) a system was built although a package could be fetched from nowhere"
    else
        grep -q 'MISSING  mytool' "$T/gap.log" \
            && ok "(d) it refuses, and it says WHICH: $(grep -m1 -oE 'MISSING +mytool +[0-9a-f]+' "$T/gap.log")" \
            || nok "(d) the refusal does not name the missing package"
    fi
    local left; left=$(ls "$T/nogap/apps" 2>/dev/null | wc -l)
    [[ "$left" == "0" ]] \
        && ok "and NOTHING was built -- a half-tree that looks finished is worse than no tree ($left in /apps)" \
        || nok "the refused rebuild left $left bundle(s) behind"
    grep -q 'vault-export' "$T/gap.log" \
        && ok "and it says what to do about it, on the machine that still has the octets" \
        || nok "the refusal does not say how to fix it"

    # GEGENPROBE: --allow-missing builds the rest, and the TREE ITSELF
    # carries the evidence. A missing package mentioned once in a
    # terminal that has been closed is a missing package nobody knows
    # about.
    $O rebuild --root "$T/gap" --plan "$T/backup/PLAN" --allow-missing > "$T/gap2.log" 2>&1
    [[ -f "$T/gap/system/INCOMPLETE" ]] \
        && ok "GEGENPROBE: --allow-missing builds the rest AND leaves system/INCOMPLETE: $(grep -m1 '^missing' "$T/gap/system/INCOMPLETE" | cut -f3)" \
        || nok "--allow-missing did not record what is absent"
    [[ ! -e "$T/gap/apps/mytool.prog" && -e "$T/gap/apps/hallo.prog" ]] \
        && ok "the incomplete tree has the packages it could get and not the one it could not" \
        || nok "the incomplete tree has the wrong bundles"
    if $O verify --root "$T/gap" > "$T/ver2.log" 2>&1; then
        nok "verify calls an incomplete tree fine"
    else
        grep -q 'this tree is INCOMPLETE' "$T/ver2.log" \
            && ok "GEGENPROBE: verify REFUSES to call it fine -> $(grep -m1 -oE 'this tree is INCOMPLETE.*' "$T/ver2.log" | cut -c1-62)" \
            || nok "verify fails on the incomplete tree for the wrong reason"
    fi
    # And a complete tree does not carry the file, so its absence means
    # something.
    [[ ! -e "$T/new/system/INCOMPLETE" ]] \
        && ok "GEGENPROBE: the COMPLETE tree has no system/INCOMPLETE, so the file's absence is a statement and not a default" \
        || nok "a complete rebuild also left system/INCOMPLETE behind"

    # -------------------- 5. the orphan on a machine it cannot run on
    #
    # This is the case where "package missing" would be a lie: the octets
    # are right here, and they still cannot help.
    local N; N=$(cat "$R/system/AKTUELL")
    sed 's/^arch	x86_64$/arch	aarch64/' "$T/backup/PLAN" > "$T/arm.plan"
    if $O rebuild --root "$T/armdev" --plan "$T/arm.plan" --vault "$T/backup/vault" > "$T/arm.log" 2>&1; then
        nok "an x86_64 orphan was installed on an aarch64 system"
    else
        grep -q 'it is an ORPHAN' "$T/arm.log" && grep -q 'BUILT AGAIN from its source code' "$T/arm.log" \
            && ok "the orphan on the other machine gets its OWN message -> $(grep -m1 -oE 'and it is an ORPHAN: no source of this plan has it, so there is no [a-z0-9_]+ build to fetch instead' "$T/arm.log")" \
            || nok "the arch failure on an orphan says the generic thing: $(grep -m1 -o 'opk: .*' "$T/arm.log" | cut -c1-70)"
    fi
    # GEGENPROBE: the SAME situation with a package that has a source
    # gets the OTHER message, because then there really is something to
    # fetch.
    grep -q 'Fetch that one' "$T/arm.log" \
        && nok "the orphan was told to fetch a build that does not exist" \
        || ok "GEGENPROBE: it does NOT say 'fetch that one' -- that advice is for packages with a source, and would be wrong here"

    # ------------------------------ THE MAIN ROAD: a source of one's own
    mkdir -p "$T/private"; cp "$T/mytool.opk" "$T/private/"
    $O schluessel "$T/mykeys" >/dev/null 2>&1
    $O quelle "$T/private" --schluessel "$T/mykeys/geheim.key" >/dev/null 2>&1
    local MYPK; MYPK=$(python3 -c "print(open('$T/mykeys/oeffentlich.key','rb').read().hex())")
    $O source-add --root "$R" "file://$T/private" "$MYPK" >/dev/null 2>&1
    $O orphans --root "$R" > "$T/orph2.log" 2>&1
    grep -q '^3 of 3 covered by a source, 0 ORPHANED' "$T/orph2.log" \
        && ok "THE MAIN ROAD: one source-add of a directory of his own, and the orphan is gone -- $(grep -m1 'covered by a source' "$T/orph2.log")" \
        || nok "adding a private source did not cover the package: $(grep -m1 'covered by' "$T/orph2.log")"
    $O backup-set --root "$R" 2>/dev/null | grep -q '^package	' \
        && nok "the backup still carries package octets although a source has them now" \
        || ok "and the backup set stops carrying its octets -- the exception disappears when it is not needed"
    # GEGENPROBE: a plan may hold ARBITRARILY many sources; this one now
    # has two, and both are in the text.
    local n_src; n_src=$(grep -c '^source	' "$R/system/generations/$(cat "$R/system/AKTUELL")/PLAN")
    [[ "$n_src" == "2" ]] \
        && ok "GEGENPROBE: the plan now names $n_src sources, each with its own key -- a private source is not a special case of anything" \
        || nok "the plan holds $n_src source lines, expected 2"

    # GEGENPROBE: a source that cannot be read must not be mistaken for a
    # source that does not have the package. `backup-set` says so in the
    # file rather than quietly carrying too much.
    $O source-add --root "$R" "https://packages.example.invalid" "$PK" >/dev/null 2>&1
    $O backup-set --root "$R" 2>/dev/null | grep -q '^# unreachable' \
        && ok "GEGENPROBE: a source this host cannot read is named in the backup set as unreachable, not silently treated as empty" \
        || nok "an unreachable source is not distinguished from an empty one"
    if $O vault-export --root "$R" -o "$T/v2" > "$T/v2.log" 2>&1; then
        nok "a vault was written although this host could not check every source"
    else
        ok "GEGENPROBE: and vault-export refuses outright -> $(grep -m1 -oE 'cannot tell what is really orphaned' "$T/v2.log")"
    fi

    rm -rf "$T"
    return $RC
}
run orphan_check
