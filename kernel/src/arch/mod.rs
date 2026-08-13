//! Schicht **arch** — Fassade ueber die aktive Architektur.
//!
//! Diese Datei ist absichtlich duenn: sie waehlt per `cfg` die Implementierung
//! und reicht die Traitmethoden aus [`crate::kcore::arch_iface`] als freie
//! Funktionen weiter, damit der Core sie ohne Generics aufrufen kann.
//!
//! Eine Portierung auf aarch64 besteht aus:
//! 1. `arch/aarch64/` anlegen und [`ArchOps`] implementieren,
//! 2. hier zwei `cfg`-Zeilen ergaenzen.
//! Kein einziges Modul ausserhalb von `arch/` muss angefasst werden.

use crate::kcore::arch_iface::ArchOps;

pub mod selftest;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

/// Die aktive Architektur.
#[cfg(target_arch = "x86_64")]
pub type Active = x86_64::X86_64;

/// Adressraumtyp der aktiven Architektur.
pub type AddressSpace = <Active as ArchOps>::AddressSpace;
/// Zeitgeber der aktiven Architektur.
pub type Timer = <Active as ArchOps>::Timer;
/// Interruptcontroller der aktiven Architektur.
pub type IntCtl = <Active as ArchOps>::IntCtl;

/// Name der aktiven Architektur.
pub const NAME: &str = <Active as ArchOps>::NAME;
/// Basisseitengroesse der aktiven Architektur.
pub const PAGE_SIZE: usize = <Active as ArchOps>::PAGE_SIZE;

/// Serielle Konsole initialisieren.
///
/// # Safety
/// Genau einmal, als erster Schritt in `kmain`.
#[inline]
pub unsafe fn init_serial() {
    unsafe { Active::init_serial() }
}

/// Text seriell ausgeben.
#[inline]
pub fn serial_write(s: &str) {
    Active::serial_write(s)
}

/// CPU-Grundstrukturen einrichten.
///
/// # Safety
/// Genau einmal, nach [`init_serial`].
#[inline]
pub unsafe fn init_cpu() {
    unsafe { Active::init_cpu() }
}

/// Interrupts sperren.
#[inline]
pub fn disable_interrupts() {
    Active::disable_interrupts()
}

/// Interrupts freigeben.
#[inline]
pub fn enable_interrupts() {
    Active::enable_interrupts()
}

/// Sind Interrupts frei?
#[inline]
pub fn interrupts_enabled() -> bool {
    Active::interrupts_enabled()
}

/// Bis zum naechsten Interrupt schlafen.
#[inline]
pub fn wait_for_interrupt() {
    Active::wait_for_interrupt()
}

/// CPU endgueltig anhalten.
#[inline]
pub fn halt_forever() -> ! {
    Active::halt_forever()
}

/// Backtrace ueber die Frame-Pointer-Kette.
#[inline]
pub fn backtrace(f: &mut dyn FnMut(u64)) {
    Active::backtrace(f)
}

/// CPU-Name in `buf` schreiben, Laenge zurueckgeben.
#[inline]
pub fn cpu_brand(buf: &mut [u8]) -> usize {
    Active::cpu_brand(buf)
}
