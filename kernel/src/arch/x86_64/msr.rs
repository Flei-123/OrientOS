//! Model-Specific Registers, soweit der fruehe Boot sie braucht.

use core::arch::asm;

/// `IA32_EFER` — enthaelt u. a. das NXE-Bit fuer das No-Execute-Seitenbit.
pub const IA32_EFER: u32 = 0xC000_0080;
/// NXE-Bit in `IA32_EFER`.
pub const EFER_NXE: u64 = 1 << 11;

/// Liest ein MSR.
///
/// # Safety
/// Unbekannte MSR-Nummern loesen `#GP` aus.
#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi,
             options(nomem, nostack, preserves_flags));
    }
    ((hi as u64) << 32) | lo as u64
}

/// Schreibt ein MSR.
///
/// # Safety
/// Falsche Werte koennen die CPU unbrauchbar konfigurieren.
#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    unsafe {
        asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32,
             options(nostack, preserves_flags));
    }
}

/// Schaltet No-Execute frei. Ohne das ist Bit 63 einer Seitentabelle reserviert
/// und ein gesetztes NX-Bit erzeugt einen Page Fault mit RSVD-Flag.
///
/// # Safety
/// Genau einmal im Boot, bevor eigene Seitentabellen aktiv werden.
pub unsafe fn enable_nx() {
    unsafe {
        let e = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, e | EFER_NXE);
    }
}

/// Setzt CR0.WP: auch der Kernel darf dann nicht in schreibgeschuetzte Seiten
/// schreiben. Faengt eine ganze Klasse stiller Fehler ab.
///
/// # Safety
/// Aendert das Speicherschutzverhalten der laufenden CPU.
pub unsafe fn enable_write_protect() {
    unsafe {
        let mut cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        cr0 |= 1 << 16;
        asm!("mov cr0, {}", in(reg) cr0, options(nostack, preserves_flags));
    }
}
