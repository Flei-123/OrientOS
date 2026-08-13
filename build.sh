#!/usr/bin/env bash
# karst — Bauen von Kernel und bootfaehigem ISO-Abbild.
#
#   ./build.sh                 Release-Build + ISO
#   ./build.sh --debug         Debug-Build
#   ./build.sh --features test-pagefault
#   ./build.sh --no-posix      Gegenprobe: Kernel ohne POSIX-Schicht
set -euo pipefail
cd "$(dirname "$0")"

PROFILE=release
CARGO_PROFILE_FLAG=--release
FEATURE_ARGS=()
EXTRA=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug) PROFILE=debug; CARGO_PROFILE_FLAG=""; shift ;;
        --features) FEATURE_ARGS+=(--features "$2"); shift 2 ;;
        --no-posix) EXTRA+=(--no-default-features); shift ;;
        -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
        *) echo "unbekannte Option: $1" >&2; exit 1 ;;
    esac
done

# build-std steht bewusst hier und nicht in .cargo/config.toml, damit
# Host-Tests (cargo test) nicht ebenfalls ein zweites `core` bauen.
BUILD_STD=(-Z build-std=core,compiler_builtins,alloc
           -Z build-std-features=compiler-builtins-mem)

echo ">> cargo build (${PROFILE})"
mkdir -p build
# Mitschreiben, damit test.sh Schritt 14 die Warnungsfreiheit pruefen kann.
cargo build "${BUILD_STD[@]}" ${CARGO_PROFILE_FLAG} \
    "${FEATURE_ARGS[@]}" "${EXTRA[@]}" 2>&1 | tee build/cargo-build.log

KERNEL="target/x86_64-karst-none/${PROFILE}/karst"
test -f "$KERNEL" || { echo "Kernelabbild fehlt: $KERNEL" >&2; exit 1; }

echo ">> Symboltabelle fuer Backtraces"
mkdir -p build
if command -v nm >/dev/null; then
    nm -n --demangle "$KERNEL" > build/karst.map || true
fi

# --------------------------------------------------------------- Userland
# Die unprivilegierten Programme werden mit nasm+ld gebaut (statisch, ET_EXEC,
# keine Laufzeitbibliothek) und zusammen mit einer Textdatei und einem
# absichtlich kaputten Abbild in ein Startdateisystem gepackt. Format und
# Begruendung: userland/mkinitramfs.py.
echo ">> Userland und Startdateisystem"
UROOT=build/userland
mkdir -p "$UROOT"
INITRAMFS=build/initramfs.img
PACK=()
if command -v nasm >/dev/null && command -v ld >/dev/null && command -v python3 >/dev/null; then
    nasm -f elf64 -o "$UROOT/hello.o" userland/hello.asm
    ld -n --build-id=none -T userland/user.ld -o "$UROOT/hello" "$UROOT/hello.o"
    python3 userland/mkbroken.py "$UROOT/hello" "$UROOT/kaputt.elf"
    PACK+=("hello=$UROOT/hello" "kaputt.elf=$UROOT/kaputt.elf")
    echo "   hello: $(stat -c%s "$UROOT/hello") B (ELF64, statisch)"
else
    echo "   Hinweis: nasm/ld/python3 fehlen — kein unprivilegiertes Programm im Archiv" >&2
fi
# Bewusst laenger als ein ELF-Kopf (64 B): so scheitert der Ladeversuch an der
# KENNUNG und nicht schon an der Laenge — der Negativtest prueft damit die
# Stelle, die er pruefen soll.
{
    echo 'Startdateisystem von build.sh — dies ist absichtlich kein Programm,'
    echo 'sondern eine Textdatei fuer den Negativtest des ELF-Laders.'
} > "$UROOT/liesmich.txt"
PACK+=("liesmich.txt=$UROOT/liesmich.txt")
if command -v python3 >/dev/null; then
    python3 userland/mkinitramfs.py "$INITRAMFS" "${PACK[@]}"
else
    # Ohne Packer kein Archiv — der Kernel meldet das ehrlich und bootet
    # trotzdem. Ein halb geschriebenes Abbild waere schlimmer als keines.
    rm -f "$INITRAMFS"
    echo "   Hinweis: python3 fehlt — Startdateisystem wird nicht gebaut" >&2
fi

echo ">> ISO bauen"
ROOT=build/isoroot
rm -rf "$ROOT"
mkdir -p "$ROOT/boot/limine" "$ROOT/EFI/BOOT"
cp "$KERNEL" "$ROOT/boot/karst"
if [[ -s "$INITRAMFS" ]]; then
    cp "$INITRAMFS" "$ROOT/boot/initramfs.img"
fi
cp limine.conf "$ROOT/boot/limine/limine.conf"
cp vendor/limine/limine-bios.sys \
   vendor/limine/limine-bios-cd.bin \
   vendor/limine/limine-uefi-cd.bin "$ROOT/boot/limine/"
cp vendor/limine/BOOTX64.EFI "$ROOT/EFI/BOOT/"

xorriso -as mkisofs -quiet -R -r -J \
    -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    -hfsplus -apm-block-size 2048 \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image \
    --protective-msdos-label \
    "$ROOT" -o build/karstos.iso

# BIOS-Bootsektor eintragen (das Host-Tool wird bei Bedarf gebaut).
if [[ ! -x vendor/limine/limine ]]; then
    make -s -C vendor/limine >/dev/null
fi
vendor/limine/limine bios-install build/karstos.iso >/dev/null

SIZE_K=$(( $(stat -c%s build/karstos.iso) / 1024 ))
KSIZE_K=$(( $(stat -c%s "$KERNEL") / 1024 ))
echo ">> fertig: build/karstos.iso (${SIZE_K} KiB), Kernel ${KSIZE_K} KiB"
