//! CPU-Grundoperationen von x86_64: Interruptflag, Halt, Kontrollregister,
//! CPUID und das Frame-Pointer-Unwinding fuer den Panic-Backtrace.

use core::arch::asm;

/// Interrupts sperren (`cli`).
#[inline]
pub fn cli() {
    unsafe { asm!("cli", options(nomem, nostack)) }
}

/// Interrupts freigeben (`sti`).
#[inline]
pub fn sti() {
    unsafe { asm!("sti", options(nomem, nostack)) }
}

/// Ist das Interrupt-Flag in RFLAGS gesetzt?
#[inline]
pub fn interrupts_enabled() -> bool {
    let flags: u64;
    unsafe { asm!("pushfq; pop {}", out(reg) flags, options(nomem, preserves_flags)) };
    flags & (1 << 9) != 0
}

/// Bis zum naechsten Interrupt anhalten (`hlt`).
#[inline]
pub fn hlt() {
    unsafe { asm!("hlt", options(nomem, nostack)) }
}

/// Endgueltig anhalten — Interrupts aus, dann `hlt` in Endlosschleife.
pub fn halt_forever() -> ! {
    cli();
    loop {
        hlt();
    }
}

/// Liest CR2 (Fehleradresse eines Page Faults).
#[inline]
pub fn read_cr2() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr2", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

/// Liest CR3 (physische Adresse der Wurzel-Seitentabelle).
#[inline]
pub fn read_cr3() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr3", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

/// Schreibt CR3 und leert damit den TLB.
///
/// # Safety
/// Der Zielbaum MUSS Code, Stack und Daten der laufenden Ausfuehrung enthalten.
#[inline]
pub unsafe fn write_cr3(v: u64) {
    unsafe { asm!("mov cr3, {}", in(reg) v, options(nostack, preserves_flags)) }
}

/// Invalidiert einen einzelnen TLB-Eintrag.
///
/// # Safety
/// Reine Cachepflege, aber nur mit gueltiger Adresse sinnvoll.
#[inline]
pub unsafe fn invlpg(addr: u64) {
    unsafe { asm!("invlpg [{}]", in(reg) addr, options(nostack, preserves_flags)) }
}

/// Roher CPUID-Aufruf (Unterblatt 0).
#[inline]
fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    cpuid_count(leaf, 0)
}

/// CPUID mit Unterblatt in ECX — Blatt 7 (Schutzmerkmale) braucht das.
///
/// RBX ist unter dem SysV-ABI callee-saved und dient dem Compiler als
/// Basiszeiger; deshalb der Umweg ueber ein Tauschregister.
#[inline]
pub(super) fn cpuid_count(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (a, c, d): (u32, u32, u32);
    let b: u64;
    unsafe {
        asm!(
            "mov {tmp:r}, rbx",
            "cpuid",
            "xchg {tmp:r}, rbx",
            tmp = out(reg) b,
            inout("eax") leaf => a,
            inout("ecx") sub => c,
            out("edx") d,
            options(nostack, preserves_flags),
        );
    }
    (a, b as u32, c, d)
}

/// Liest CR4 (Merkmalsschalter der CPU, u. a. die Schutzbits gegen Zugriffe
/// auf unprivilegierte Seiten).
#[inline]
pub(super) fn read_cr4() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr4", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

/// Schreibt CR4.
///
/// # Safety
/// Falsche Bits machen die laufende Ausfuehrung sofort unbrauchbar.
#[inline]
pub(super) unsafe fn write_cr4(v: u64) {
    unsafe { asm!("mov cr4, {}", in(reg) v, options(nostack, preserves_flags)) }
}

/// Schreibt den CPU-Markennamen (CPUID 0x8000_0002..4) nach `buf`.
pub fn cpu_brand(buf: &mut [u8]) -> usize {
    let (max_ext, _, _, _) = cpuid(0x8000_0000);
    if max_ext < 0x8000_0004 {
        let s = b"unbekannt";
        let n = s.len().min(buf.len());
        buf[..n].copy_from_slice(&s[..n]);
        return n;
    }
    let mut n = 0;
    for leaf in 0x8000_0002u32..=0x8000_0004 {
        let (a, b, c, d) = cpuid(leaf);
        for word in [a, b, c, d] {
            for byte in word.to_le_bytes() {
                if n < buf.len() {
                    buf[n] = byte;
                    n += 1;
                }
            }
        }
    }
    // Fuehrende Leerzeichen und Nullbytes am Ende abschneiden.
    while n > 0 && (buf[n - 1] == 0 || buf[n - 1] == b' ') {
        n -= 1;
    }
    n
}

/// Untergrenze gueltiger Kerneladressen — alles darunter ist Userspace oder Muell
/// und wird beim Unwinding als Kettenende gewertet.
const KERNEL_MIN: u64 = 0xffff_8000_0000_0000;

/// Laeuft die RBP-Kette ab und meldet jede Ruecksprungadresse.
///
/// Der Kernel wird mit `-C force-frame-pointers=yes` uebersetzt (siehe
/// `.cargo/config.toml`), dadurch gilt `[rbp] = voriger rbp` und
/// `[rbp+8] = Ruecksprungadresse`.
pub fn backtrace(f: &mut dyn FnMut(u64)) {
    let mut rbp: u64;
    unsafe { asm!("mov {}, rbp", out(reg) rbp, options(nomem, nostack, preserves_flags)) };

    for _ in 0..32 {
        if rbp < KERNEL_MIN || rbp & 0x7 != 0 {
            break;
        }
        let next = unsafe { core::ptr::read_volatile(rbp as *const u64) };
        let ret = unsafe { core::ptr::read_volatile((rbp + 8) as *const u64) };
        if ret < KERNEL_MIN {
            break;
        }
        f(ret);
        if next <= rbp {
            break;
        }
        rbp = next;
    }
}
