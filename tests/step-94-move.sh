# tests/step-94-move.sh -- sourced by test.sh, not started on its own.
#
# "IF I HAVE A BACKUP ON A USB STICK -- CAN I SET A NEW DEVICE UP FROM IT
# WITHOUT SIGNING IN TO AN ACCOUNT? JUST SAY: I WANT THIS DEVICE TO BE
# LIKE THAT ONE TOO."
#
# The answer has to be yes, and it is a principle rather than a feature:
#
#     AN ACCOUNT IS A CONVENIENCE. IT IS NEVER A CONDITION.
#
# If a server is ever added it may make this EASIER; it may never make it
# POSSIBLE, because it already is. This step is what stops that sentence
# from being a slogan: the whole move is measured with the source
# directory MOVED AWAY, so nothing can quietly reach for a network, and
# on a root that did not exist a second earlier.
#
# WHAT IS MEASURED, and the third one is the part that is easy to get
# wrong:
#
#   1. THAT IT WORKS AT ALL. Empty root, no account, no network, and
#      afterwards the tree is compared with the old one ENTRY FOR ENTRY,
#      including the documents -- a comparison that skipped the documents
#      would go green on losing every one of them.
#
#   2. THAT THE INTENTIONAL DIFFERENCES ARE EXACTLY THE INTENDED ONES.
#      Not "few", not "explainable": the counter-check restores the same
#      stick with --keep-identity and requires the two trees to be
#      IDENTICAL. That is what proves the earlier difference was three
#      things and not four.
#
#   3. THAT WHAT MUST BE NEW IS NEW. "I want this device to be like that
#      one" moves a state onto a DIFFERENT machine, and two machines with
#      one identity is a fault that is silent on the day it is made.
#      machine-id, the device key, the hostname and a static address must
#      not travel -- and the identity is kept OUT OF THE PLAN entirely,
#      so that it cannot travel even by accident.
step "Moving a system on a stick: no account, no network -- and an identity that is new"
move_check() {
    RC=0
    local O="python3 pkg/opk.py"
    local T; T=$(mktemp -d "${TMPDIR:-/tmp}/orientos-move-XXXXXX")
    local FIRNC=../firn/compiler/target/release/firnc

    if [[ ! -f build/pakete/hallo.opk ]]; then ./pkg/bauen.sh >/dev/null 2>&1; fi
    mkdir -p "$T/appstore"
    cp build/pakete/hallo.opk build/pakete/editor.opk build/pakete/dusk.opk \
       build/pakete/deep.opk build/pakete/osum.opk "$T/appstore/" 2>/dev/null
    $O schluessel "$T/keys" >/dev/null 2>&1
    $O quelle "$T/appstore" --schluessel "$T/keys/geheim.key" >/dev/null 2>&1
    local PK; PK=$(python3 -c "print(open('$T/keys/oeffentlich.key','rb').read().hex())")

    # A program the owner built himself: in no source, so the stick has
    # to carry it whatever mode it is written in.
    if [[ -x $FIRNC && -f ../firn/tests/048_print_number.fi ]] \
       && $FIRNC --target=x86_64-linux -o "$T/mytool" ../firn/tests/048_print_number.fi >/dev/null 2>&1; then :; else
        cp vendor/osum/bin/cat "$T/mytool"; fi
    printf 'name=mytool\nfassung=1.0.0\ntitel=mytool\ninfo=self built\nkeys=mytool\nhandle=console\ndatei=start %s\n' "$T/mytool" > "$T/mytool.rezept"
    $O bauen "$T/mytool.rezept" -o "$T/mytool.opk" >/dev/null 2>&1

    # ------------------------------------------------- the old device
    local R="$T/old"
    $O arch --root "$R" x86_64 >/dev/null 2>&1
    $O source-add --root "$R" "file://$T/appstore" "$PK" >/dev/null 2>&1
    local p
    for p in hallo editor; do
        $O installieren --root "$R" --quelle "$T/appstore" --schluessel "$T/keys/oeffentlich.key" "$p" >/dev/null 2>&1
    done
    $O installieren --root "$R" "$T/mytool.opk" >/dev/null 2>&1
    $O kernel --root "$R" build/pakete/osum.opk >/dev/null 2>&1
    $O account-add --root "$R" justin >/dev/null 2>&1
    echo "geheim123" > "$T/pw"; $O secret-set --root "$R" justin "$T/pw" >/dev/null 2>&1
    $O set --root "$R" hostname alterrechner >/dev/null 2>&1
    $O set --root "$R" timezone Europe/Vienna >/dev/null 2>&1
    $O set --root "$R" net.mode static >/dev/null 2>&1
    $O set --root "$R" net.address 192.168.1.41 >/dev/null 2>&1
    $O set --root "$R" net.netmask 255.255.255.0 >/dev/null 2>&1
    $O set --root "$R" net.gateway 192.168.1.1 >/dev/null 2>&1
    $O pref --root "$R" justin theme build/pakete/dusk.opk >/dev/null 2>&1
    $O pref --root "$R" justin wallpaper build/pakete/deep.opk >/dev/null 2>&1
    $O pref --root "$R" justin taskbar.edge left >/dev/null 2>&1
    mkdir -p "$R/users/justin/state/editor" "$R/users/justin/config/editor" \
             "$R/users/justin/cache/editor"
    echo "mein roman, kapitel eins" > "$R/users/justin/state/editor/roman.txt"
    head -c 40000 /dev/urandom > "$R/users/justin/state/editor/bild.bin"
    echo "schriftgroesse=14" > "$R/users/justin/config/editor/settings"
    head -c 300000 /dev/urandom > "$R/users/justin/cache/editor/thumbs"

    # EVERY ROOT HAS AN IDENTITY, from the moment it exists.
    local oldid; oldid=$(cat "$R/system/machine-id" 2>/dev/null)
    [[ ${#oldid} -eq 32 ]] \
        && ok "a root has a machine-id of its own as soon as it exists (${oldid:0:16}…), and a device key at mode $(stat -c%a "$R/system/device.key")" \
        || nok "the root has no machine identity"
    # AND IT IS NOT IN THE PLAN. That is the rule, and this is the check
    # that it is a rule and not an intention: identity lives outside the
    # generations, so a plan cannot carry it even by accident.
    grep -rq "$oldid" "$R/system/generations/" 2>/dev/null \
        && nok "the machine-id appears inside a generation -- then it could travel" \
        || ok "and it is NOT in any PLAN: the plan holds wishes, and an identity is not a wish"

    # ---------------------------------------------- small versus full
    $O stick-write --root "$R" -o "$T/small" --mode small > "$T/sw-s.log" 2>&1
    $O stick-write --root "$R" -o "$T/full"  --mode full  > "$T/sw-f.log" 2>&1
    groesse() { python3 -c "
import os,sys
t=n=0
for p,_v,ns in os.walk(sys.argv[1]):
    for x in ns:
        t+=os.path.getsize(os.path.join(p,x)); n+=1
print('%d %d' % (t,n))" "$1"; }
    local so sn fo fn
    read -r so sn <<< "$(groesse "$T/small")"
    read -r fo fn <<< "$(groesse "$T/full")"
    if [[ "$fo" -gt "$so" ]]; then
        ok "small $so octets in $sn file(s), full $fo in $fn -- full is $(python3 -c "print('%.1f' % ($fo/$so))")x the size and needs no network"
    else
        nok "full ($fo) is not larger than small ($so)"
    fi
    grep -q 'needs a network when it is read' "$T/sw-s.log" \
        && ok "and the small stick SAYS it needs one: $(grep -m1 -oE 'this stick needs a network when it is read -- [0-9]+ package\(s\)' "$T/sw-s.log")" \
        || nok "the small stick does not warn that it needs a network"
    grep -q 'needs NO network and NO account' "$T/sw-f.log" \
        && ok "while full says the opposite, which is why it is the default for a move" \
        || nok "the full stick does not state that it is self-sufficient"
    # GEGENPROBE: the self-built package is on the SMALL stick too. It
    # is the one thing no mode may ever leave behind.
    local n_small; n_small=$(ls "$T/small/vault"/*.opk 2>/dev/null | wc -l)
    [[ "$n_small" -ge 1 ]] && grep -q 'mytool' "$T/small/SET" \
        && ok "GEGENPROBE: even the small stick carries mytool -- no source has it, so no mode may leave it behind" \
        || nok "the small stick does not carry the orphaned package"

    # THE STICK IS THE BACKUP SET, not a second format.
    local fehlend=0 zeile art pfad
    while IFS=$'\t' read -r art pfad _rest; do
        [[ "$art" == "plan" || "$art" == "secret" || "$art" == "tree" ]] || continue
        [[ -e "$T/full/$pfad" ]] || fehlend=$((fehlend+1))
    done < <(grep -v '^#' "$T/full/SET")
    [[ "$fehlend" == "0" ]] \
        && ok "every path the stick's SET names is really on the stick ($(grep -vc '^#' "$T/full/SET") lines) -- the layout IS the backup set, not a second format" \
        || nok "$fehlend path(s) named by SET are missing from the stick"

    # ------------------------------------- THE MOVE: no network, no account
    #
    # The source is moved out of the way for the whole restore. If
    # anything reached for it, this would fail rather than pass quietly.
    mv "$T/appstore" "$T/appstore.gone"
    $O stick-restore --root "$T/new" --from "$T/full" > "$T/rest.log" 2>&1
    local rc=$?
    mv "$T/appstore.gone" "$T/appstore"
    [[ $rc -eq 0 ]] \
        && ok "a new device was set up from the stick with the source MOVED AWAY: $(grep -m1 -oE '[0-9]+ package\(s\) fetched and verified, [0-9]+ octet\(s\), [0-9]+ of them from the vault' "$T/rest.log")" \
        || nok "the restore failed: $(grep -m1 -o 'opk: .*' "$T/rest.log" | cut -c1-70)"
    grep -q 'REFUSED file://' "$T/rest.log" \
        && ok "and the log proves the source really was unreachable at the time: $(grep -m1 'REFUSED file://' "$T/rest.log" | sed 's/^ *//' | cut -c1-56)" \
        || nok "the source was reachable during the restore, so offline was not measured"
    grep -q 'No account was asked for and no network was used' "$T/rest.log" \
        && ok "no account was asked for at any point -- an account is a convenience, never a condition" \
        || nok "the restore did not complete without an account"
    $O verify --root "$T/new" >/dev/null 2>&1 \
        && ok "verify on the new device passes" \
        || nok "verify fails on the moved system: $($O verify --root "$T/new" 2>&1 | grep -m1 FAILED)"
    grep -q '1 credential(s) restored and checked against the plan' "$T/rest.log" \
        && ok "and the credential went through the plan's own check, so a stick whose secrets and plan disagree is caught" \
        || nok "the credential was not checked against the plan"

    # ------------------------------- entry for entry, WITH the documents
    $O snapshot --root "$R"     --with-data 2>/dev/null | grep -v '^#' > "$T/s-old"
    $O snapshot --root "$T/new" --with-data 2>/dev/null | grep -v '^#' > "$T/s-new"
    # The `count` line is a summary of the others, not an entry, so it is
    # dropped before counting -- otherwise every real difference would be
    # counted twice, once on its own and once inside the total.
    diff <(grep -v '^count ' "$T/s-old") <(grep -v '^count ' "$T/s-new") > "$T/d1" 2>&1
    local n_alt; n_alt=$(( $(wc -l < "$T/s-old") - 2 ))
    local n_diff; n_diff=$(grep -c '^[<>]' "$T/d1")
    # THE THREE INTENTIONAL ONES, and nothing else may be in here: the
    # PLAN (five settings shorter) and the two files those settings
    # rendered. Four diff lines, because the plan is CHANGED (< and >)
    # while the two files are only on the old side.
    if [[ "$n_diff" == "4" ]] && grep -q '^< f etc/hostname' "$T/d1" \
       && grep -q '^< f etc/network.conf' "$T/d1" && grep -q '^< plan ' "$T/d1" \
       && grep -q '^> plan ' "$T/d1"; then
        ok "compared over $n_alt entries: the ONLY differences are the PLAN ($(grep -m1 '^< plan' "$T/d1" | awk '{print $4}') -> $(grep -m1 '^> plan' "$T/d1" | awk '{print $4}') octets) and the two files those machine settings rendered (etc/hostname, etc/network.conf)"
    else
        nok "compared over $n_alt entries and got $n_diff unexpected difference(s): $(grep '^[<>]' "$T/d1" | head -4 | tr '\n' ' ')"
    fi
    grep -q 'f users/justin/state/editor/roman.txt' "$T/s-new" \
        && ok "the documents came along and are compared by content: $(grep -m1 -oE 'users/justin/state/editor/roman.txt 644 [0-9]+' "$T/s-new")" \
        || nok "the documents are not in the comparison -- then it measures nothing"
    grep -q 'users/justin/cache' "$T/s-new" && ! grep -q 'cache/editor/thumbs' "$T/s-new" \
        && ok "GEGENPROBE: the cache directory exists but its contents were NOT carried -- 300 000 octets that regenerate" \
        || nok "the cache was carried or the pot is missing"

    # -------- THE COUNTER-CHECK THAT MAKES THE ABOVE A MEASUREMENT
    $O stick-restore --root "$T/keep" --from "$T/full" --keep-identity > "$T/keep.log" 2>&1
    $O snapshot --root "$T/keep" --with-data 2>/dev/null | grep -v '^#' > "$T/s-keep"
    if diff -q "$T/s-old" "$T/s-keep" >/dev/null 2>&1; then
        ok "GEGENPROBE: with --keep-identity the two trees are IDENTICAL over all $n_alt entries -- so the differences above were exactly three, not roughly three"
    else
        nok "even with --keep-identity $(diff "$T/s-old" "$T/s-keep" | grep -c '^[<>]') entries differ"
    fi

    # ------------------------------------------ what MUST be new, is new
    local newid keepid
    newid=$(cat "$T/new/system/machine-id"); keepid=$(cat "$T/keep/system/machine-id")
    local n_u; n_u=$(printf '%s\n%s\n%s\n' "$oldid" "$newid" "$keepid" | sort -u | wc -l)
    [[ "$n_u" == "3" ]] \
        && ok "three trees, three machine-ids (${oldid:0:8}…, ${newid:0:8}…, ${keepid:0:8}…) -- even --keep-identity does not carry the identity itself" \
        || nok "only $n_u distinct machine-ids across three trees"
    [[ "$(cat "$T/new/system/device.pub")" != "$(cat "$R/system/device.pub")" ]] \
        && ok "and the device key is new too, mode $(stat -c%a "$T/new/system/device.key")" \
        || nok "the device key was copied from the old machine"
    grep -q '^setting	hostname' "$T/new/system/generations/1/PLAN" \
        && nok "the hostname of the old machine travelled" \
        || ok "the hostname did NOT travel: $(grep -m1 -oE 'hostname +alterrechner +two machines answering to one name' "$T/rest.log")"
    grep -q '^setting	net.address' "$T/new/system/generations/1/PLAN" \
        && nok "the static address travelled -- two machines would claim 192.168.1.41" \
        || ok "nor did the static address: $(grep -m1 -oE 'a fixed address is a claim on a network' "$T/rest.log")"
    grep -q '^setting	timezone	Europe/Vienna' "$T/new/system/generations/1/PLAN" \
        && ok "GEGENPROBE: the timezone DID travel -- it is a wish and not a machine, and the split is by meaning and not by convenience" \
        || nok "the timezone was dropped although it is not an identity"

    # ----------------------------------------------- partial moves
    $O stick-write --root "$R" -o "$T/stick-other" --mode full --no-personal >/dev/null 2>&1
    $O stick-restore --root "$T/other" --from "$T/stick-other" --no-personal > "$T/other.log" 2>&1
    local n_apps n_docs n_acc n_pref
    n_apps=$(ls "$T/other/apps" 2>/dev/null | wc -l)
    n_docs=$(find "$T/other/users" -type f 2>/dev/null | wc -l)
    n_acc=$(grep -c '^account' "$T/other/system/generations/1/PLAN")
    n_pref=$(grep -c '^pref' "$T/other/system/generations/1/PLAN")
    [[ "$n_apps" -ge 3 && "$n_docs" == "0" && "$n_acc" == "0" && "$n_pref" == "0" ]] \
        && ok "--no-personal: $n_apps program(s), $n_docs document(s), $n_acc account(s), $n_pref preference(s) -- the machine without the person, for a device that goes to somebody else" \
        || nok "--no-personal gave $n_apps apps / $n_docs docs / $n_acc accounts / $n_pref prefs"
    grep -q '^setting	timezone' "$T/other/system/generations/1/PLAN" \
        && ok "and the settings that belong to nobody in particular stayed" \
        || nok "--no-personal also dropped the plain settings"
    [[ ! -s "$T/stick-other/SET" || $(grep -c '^tree' "$T/stick-other/SET") == "0" ]] \
        && ok "GEGENPROBE: a --no-personal STICK has no document on it at all -- nothing to leak, not merely nothing restored" \
        || nok "the --no-personal stick still carries the documents"

    $O stick-restore --root "$T/nodata" --from "$T/full" --no-data > "$T/nodata.log" 2>&1
    local c_cfg c_st c_pr
    c_cfg=$(find "$T/nodata/users/justin/config" -type f 2>/dev/null | wc -l)
    c_st=$(find "$T/nodata/users/justin/state" -type f 2>/dev/null | wc -l)
    c_pr=$(grep -c '^pref' "$T/nodata/system/generations/1/PLAN")
    [[ "$c_cfg" -ge 1 && "$c_st" == "0" && "$c_pr" == "3" ]] \
        && ok "--no-data: the same person, the same look ($c_pr preferences), $c_cfg settings file(s) and $c_st documents -- a fresh start that is still his machine" \
        || nok "--no-data gave $c_cfg config / $c_st state / $c_pr prefs"

    # ------------------------------------------------ the wrong machine
    if $O stick-restore --root "$T/armdev" --from "$T/full" --arch aarch64 > "$T/arm.log" 2>&1; then
        nok "an x86_64 stick set up an aarch64 device"
    else
        grep -q 'a backup cannot translate them' "$T/arm.log" \
            && ok "an x86-64 stick at an ARM device: $(grep -m1 -oE 'this stick holds a [a-z0-9_]+ system and you are setting up a [a-z0-9_]+ device' "$T/arm.log")" \
            || nok "the architecture failure does not explain itself: $(grep -m1 -o 'opk: .*' "$T/arm.log" | cut -c1-60)"
    fi
    [[ "$(ls "$T/armdev/apps" 2>/dev/null | wc -l)" == "0" ]] \
        && ok "and nothing was built -- a half-working device is worse than a refusal" \
        || nok "the refused restore left bundles behind"
    grep -q 'What CAN be carried over is the plan itself' "$T/arm.log" \
        && ok "and it says what CAN still be done: the plan travels, the packages must come from a source for that machine" \
        || nok "the message does not say what is still possible"

    # ------------------------- one format: TRESOR may copy store directories
    #
    # A block-deduplicating backup wants to copy `store/<hash>/` as
    # directories, not to pack them into files. If that produced
    # something this could not read, there would be two formats on the
    # stick and one of them would rot.
    mkdir -p "$T/tstick"; cp -a "$T/full/." "$T/tstick/"
    rm -rf "$T/tstick/vault"; mkdir "$T/tstick/vault"
    local d
    for d in "$R"/store/*/; do cp -a "$d" "$T/tstick/vault/"; done
    mv "$T/appstore" "$T/appstore.gone"
    $O stick-restore --root "$T/tdev" --from "$T/tstick" > "$T/t.log" 2>&1
    mv "$T/appstore.gone" "$T/appstore"
    $O snapshot --root "$T/tdev" --with-data 2>/dev/null | grep -v '^#' > "$T/s-t"
    if diff -q "$T/s-new" "$T/s-t" >/dev/null 2>&1; then
        ok "a stick whose vault holds COPIED STORE DIRECTORIES instead of .opk files restores to the identical tree -- one format for TRESOR, not two"
    else
        nok "the store-directory form gives a different tree ($(diff "$T/s-new" "$T/s-t" | grep -c '^[<>]') entries)"
    fi

    # GEGENPROBE: a directory that is not a stick.
    mkdir -p "$T/notastick"
    if $O stick-restore --root "$T/zz" --from "$T/notastick" > "$T/ns.log" 2>&1; then
        nok "a directory with no STICK file was accepted as a system state"
    else
        ok "GEGENPROBE: a plain directory is refused -> $(grep -m1 -oE 'has no STICK file' "$T/ns.log") (a pile of files is not a system state)"
    fi

    rm -rf "$T"
    return $RC
}
run move_check
