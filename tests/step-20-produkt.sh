# tests/step-20-produkt.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# AUS EINEM KERNEL UND EINEM HAUFEN PROGRAMME WIRD EIN PRODUKT.
#
# Das ist die Arbeit, die nach dem Kernelwechsel bei OrientOS liegt, und
# sie besteht aus vier Stuecken, die alle stillschweigend kaputtgehen
# koennen:
#
#   1. Im ISO liegt GENAU das Abbild, das aus dem festgenagelten Commit
#      gebaut wurde — nicht eines von gestern.
#   2. Daneben liegt ein DATEISYSTEM, das dieses Repo zusammengestellt
#      hat (`userland/PROGRAMME`), und der Lader reicht es als
#      Boot-Modul weiter.
#   3. Die Kommandozeile nennt dem Kernel die CRC32 dieses Dateisystems.
#      Rechnet der Wirt eine andere als die, die im ISO steht, laedt der
#      Kernel nichts — und dann waere das Produkt ohne Userland.
#   4. Der Multiboot-Kopf verlangt einen linearen Rahmenpuffer. Ohne
#      Flag-Bit 2 bricht Limine unter UEFI mit "Cannot use text mode with
#      UEFI" ab; das ist der Grund, warum Osums Kopf geaendert werden
#      musste (KERNELWECHSEL.md § 3.1).
step "Das Produkt: Kernel, Userland-Modul und ein bootfaehiges ISO"
produkt_check() {
    RC=0
    ./build.sh >/dev/null || { nok "./build.sh ist fehlgeschlagen"; return 1; }
    local iso="build/${SLUG}.iso"
    local img="build/${SLUG}-userland.img"
    local conf=build/isoroot/boot/limine/limine.conf

    [[ -s "$iso" ]] && ok "$iso ($(( $(stat -c%s "$iso") / 1024 )) KiB)" \
                    || { nok "$iso fehlt"; return 1; }

    if cmp -s vendor/osum/osum.mb build/isoroot/boot/osum; then
        ok "das Abbild im ISO ist byteweise das gebaute Osum-Abbild ($(( $(stat -c%s vendor/osum/osum.mb) / 1024 )) KiB)"
    else
        nok "das Abbild im ISO ist nicht das gebaute Osum-Abbild"
    fi

    # --- das Userland-Modul
    if [[ -s "$img" ]]; then
        ok "das Userland-Dateisystem ist gebaut ($(( $(stat -c%s "$img") / 1024 )) KiB)"
    else
        nok "$img fehlt"; return 1
    fi
    if cmp -s "$img" "build/isoroot/boot/${SLUG}-userland.img"; then
        ok "und liegt byteweise gleich im ISO"
    else
        nok "das Modul im ISO ist ein anderes als das gebaute"
    fi
    # Der Wirt liest sein eigenes Abbild zurueck: steht wirklich drin, was
    # userland/PROGRAMME sagt?
    local liste soll ist fehlt=""
    liste=$(python3 vendor/osum/mkfs.py list "$img" 2>/dev/null)
    soll=0
    while read -r z; do
        z=${z%%#*}; z=$(echo "$z" | xargs || true)
        [[ -z "$z" ]] && continue
        case "$z" in bloecke:*|verzeichnis\ *|datei\ *) continue ;; esac
        soll=$((soll + 1))
        grep -q "^/bin/$z " <<<"$liste" || fehlt="$fehlt $z"
    done < userland/PROGRAMME
    if [[ -z "$fehlt" ]]; then
        ok "alle $soll Programme aus userland/PROGRAMME stehen im Abbild"
    else
        nok "im Abbild fehlen:$fehlt"
    fi
    # ...und die Datei, die aus DIESEM Repo kommt.
    if grep -q '^/etc/ausgabe.txt' <<<"$liste"; then
        ok "und die Datei, die dieses Repo beisteuert (/etc/ausgabe.txt)"
    else
        nok "/etc/ausgabe.txt fehlt im Abbild — OrientOS steuert dem Produkt nichts Eigenes bei"
    fi
    ist=$(grep -c '^/bin/[a-z]' <<<"$liste")
    ok "im Abbild: $ist Programme unter /bin"

    # --- die Kommandozeile
    local crc soll_crc
    soll_crc=$(python3 -c 'import zlib,sys;print("%08x"%zlib.crc32(open(sys.argv[1],"rb").read()))' "$img")
    crc=$(grep -m1 'cmdline:' "$conf" | grep -oE 'modcrc=[0-9a-f]+' | cut -d= -f2)
    if [[ "$crc" == "$soll_crc" ]]; then
        ok "limine.conf nennt dem Kernel die richtige Pruefsumme (0x$crc)"
    else
        nok "limine.conf nennt modcrc=$crc, das Abbild hat 0x$soll_crc"
    fi
    grep -q 'modfs' "$conf" && ok "und das Wort modfs — der Kernel soll das Modul als Wurzel nehmen" \
                            || nok "modfs fehlt in der Kommandozeile"
    grep -qE '^ *module_path: ' "$conf" && ok "der Lader bekommt das Modul ueber module_path" \
                                        || nok "module_path fehlt in limine.conf"
    grep -q '^ *protocol: multiboot1' "$conf" \
        && ok "gestartet wird ueber das Multiboot-Protokoll" \
        || nok "limine.conf nennt nicht das Multiboot-Protokoll"

    # --- der Kopf, der den UEFI-Start ueberhaupt erlaubt
    local video
    video=$(python3 - vendor/osum/osum.mb <<'PY'
import struct, sys
d = open(sys.argv[1], 'rb').read()
for off in range(0, min(len(d), 8192) - 48, 4):
    if d[off:off+4] == b'\x02\xb0\xad\x1b':
        magie, flags, pruef = struct.unpack_from('<3I', d, off)
        modus = struct.unpack_from('<I', d, off + 32)[0]
        print(flags, (magie + flags + pruef) & 0xFFFFFFFF, modus)
        break
else:
    print("0 1 9")
PY
)
    set -- $video
    if [[ "$2" == 0 && $(( $1 & 4 )) -ne 0 && "$3" == 0 ]]; then
        ok "Multiboot-Kopf: Pruefsumme geht auf, Bit 2 gesetzt, linearer Rahmenpuffer"
    else
        nok "Multiboot-Kopf: flags=$1 pruefsumme=$2 mode_type=$3 — ohne Bit 2 bricht Limine unter UEFI ab"
    fi
    if [[ $(( $1 & 8 )) -ne 0 ]]; then
        ok "und Bit 3: der Kern nimmt Boot-Module entgegen"
    else
        nok "Flag-Bit 3 fehlt — der Kern wuerde das Modul gar nicht sehen"
    fi
    return $RC
}
run produkt_check
