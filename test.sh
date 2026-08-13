#!/usr/bin/env bash
# Gesamter Testlauf von Karstos: Host-Tests + echte QEMU-Boots.
# Exitcode 0 = alles bestanden.
set -uo pipefail
cd "$(dirname "$0")"
FAIL=0
step() { echo; echo "################ $* ################"; }
run()  { if "$@"; then echo "  => bestanden"; else echo "  => FEHLGESCHLAGEN"; FAIL=1; fi }

step "1/10 Host-Tests der architekturunabhaengigen Crates"
run cargo test --target x86_64-unknown-linux-gnu \
      -p karst-mem -p karst-abi-native -p karst-abi-posix

step "2/10 Kernel baut (Release, mit POSIX)"
run ./build.sh

step "3/10 Kernel baut OHNE POSIX-Schicht"
run ./build.sh --no-posix

step "4/10 Boot in QEMU (BIOS)"
run ./run-qemu.sh --check

step "5/10 Boot in QEMU ohne POSIX-Schicht"
run ./run-qemu.sh --check --no-posix

step "6/10 Selbsttest Page Fault"
run ./run-qemu.sh --test-pagefault

step "7/10 Selbsttest Double Fault (echter #DF auf IST-Stapel)"
run ./run-qemu.sh --test-doublefault

step "8/10 Selbsttest Panic-Handler"
run ./run-qemu.sh --test-panic

step "9/10 Selbsttest .rodata schreibgeschuetzt (#PF erwartet)"
run ./run-qemu.sh --test-rodata

step "10/10 Boot ueber UEFI (OVMF)"
if ls /usr/share/OVMF/OVMF_CODE*.fd >/dev/null 2>&1; then
    run ./run-qemu.sh --check --uefi
else
    echo "  => uebersprungen (OVMF nicht installiert)"
fi

echo
if [[ $FAIL -eq 0 ]]; then
    echo "############ ALLE TESTS BESTANDEN ############"
else
    echo "############ TESTS FEHLGESCHLAGEN ############"
fi
exit $FAIL
