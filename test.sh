#!/usr/bin/env bash
# Gesamter Testlauf von Karstos: Host-Tests + echte QEMU-Boots.
# Exitcode 0 = alles bestanden.
set -uo pipefail
cd "$(dirname "$0")"
FAIL=0
step() { echo; echo "################ $* ################"; }
run()  { if "$@"; then echo "  => bestanden"; else echo "  => FEHLGESCHLAGEN"; FAIL=1; fi }

step "1/14 Host-Tests der architekturunabhaengigen Crates"
run cargo test --target x86_64-unknown-linux-gnu \
      -p karst-mem -p karst-abi-native -p karst-abi-posix

step "2/14 Kernel baut (Release, mit POSIX)"
run ./build.sh

step "3/14 Kernel baut OHNE POSIX-Schicht"
run ./build.sh --no-posix

step "4/14 Boot in QEMU (BIOS)"
run ./run-qemu.sh --check

step "5/14 Boot in QEMU ohne POSIX-Schicht"
run ./run-qemu.sh --check --no-posix

step "6/14 Selbsttest Page Fault"
run ./run-qemu.sh --test-pagefault

step "7/14 Selbsttest Double Fault (echter #DF auf IST-Stapel)"
run ./run-qemu.sh --test-doublefault

step "8/14 Selbsttest Panic-Handler"
run ./run-qemu.sh --test-panic

step "9/14 Selbsttest .rodata schreibgeschuetzt (#PF erwartet)"
run ./run-qemu.sh --test-rodata

step "10/14 Selbsttest NX: Daten ausfuehren (#PF mit Instruktionsabruf erwartet)"
run ./run-qemu.sh --test-nx

step "11/14 Selbsttest #GP: nicht kanonische Adresse (kein #PF, sondern #GP)"
run ./run-qemu.sh --test-gp

step "12/14 Selbsttest #UD: ungueltige Instruktion (ud2)"
run ./run-qemu.sh --test-ud

step "13/14 Boot ueber UEFI (OVMF)"
if ls /usr/share/OVMF/OVMF_CODE*.fd >/dev/null 2>&1; then
    run ./run-qemu.sh --check --uefi
else
    echo "  => uebersprungen (OVMF nicht installiert)"
fi

step "14/14 Architekturgrenze und Codehygiene"
arch_leak() {
    local hits
    hits=$(grep -rnE '\b(cr[0-3]|PML4|PTE|rdmsr|wrmsr|lgdt|lidt|outb|inb|asm!|invlpg|iretq)\b' \
             kernel/src --include='*.rs' | grep -v '^kernel/src/arch/')
    # Nur echter Code ist ein Verstoss; ein Kommentar, der die Grenze ERKLAERT,
    # ist keiner. Beides wird gemeldet, aber nur Code laesst den Schritt fallen.
    local code
    code=$(echo "$hits" | grep -vE ':[[:space:]]*(//|\*)' | grep -v '^$')
    if [[ -n "$code" ]]; then
        echo "x86-Details im Code ausserhalb kernel/src/arch/:"
        echo "$code"
        return 1
    fi
    echo "  keine x86-Details im Code ausserhalb kernel/src/arch/"
    if [[ -n "$hits" ]]; then
        echo "  Hinweis: x86-Begriffe erscheinen nur in erklaerenden Kommentaren:"
        echo "$hits" | sed 's/^/    /'
    fi
    local stubs
    stubs=$(grep -rnE '(todo!|unimplemented!)' kernel/src libs --include='*.rs' \
              | grep -vE ':[[:space:]]*(//|\*)')
    if [[ -n "$stubs" ]]; then
        echo "unfertige Stellen im Code:"
        echo "$stubs"
        return 1
    fi
    echo "  kein todo!()/unimplemented!() im Baum"
    local crates
    crates=$(grep -cE '^name = ' Cargo.lock)
    echo "  Crates in Cargo.lock (inkl. eigener): $crates"
    # Jeder Fehler-Selbsttest, den kernel/Cargo.toml anbietet, braucht auch
    # einen Schalter in run-qemu.sh — sonst gibt es tote Testpfade, die nie
    # jemand ausfuehrt.
    local feat missing=""
    for feat in $(grep -oE '^test-[a-z]+' kernel/Cargo.toml); do
        grep -q -- "--$feat)" run-qemu.sh || missing="$missing $feat"
    done
    if [[ -n "$missing" ]]; then
        echo "Selbsttest-Feature ohne Schalter in run-qemu.sh:$missing"
        return 1
    fi
    echo "  jedes test-*-Feature hat einen Schalter in run-qemu.sh"
    # Kennzahlen, damit Groessenwachstum sichtbar bleibt (siehe README).
    if [[ -f build/isoroot/boot/karst ]]; then
        echo "  Kernelabbild: $(( $(stat -c%s build/isoroot/boot/karst) / 1024 )) KiB (ELF, ungestrippt)"
    fi
    echo "  Rust-Zeilen kernel+libs: $(find kernel/src libs -name '*.rs' -exec cat {} + | wc -l)"
    return 0
}
run arch_leak

echo
if [[ $FAIL -eq 0 ]]; then
    echo "############ ALLE TESTS BESTANDEN ############"
else
    echo "############ TESTS FEHLGESCHLAGEN ############"
fi
exit $FAIL
