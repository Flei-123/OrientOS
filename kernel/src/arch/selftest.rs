//! Absichtliche Fehlerausloesung fuer die Boot-Selbsttests.
//!
//! Steht in `arch`, weil "einen Double Fault erzeugen" architekturabhaengig ist.
//! Der Kernel-Core ruft nur diese drei neutralen Funktionen auf.

#[cfg(target_arch = "x86_64")]
use super::x86_64::selftest as imp;

/// Loest einen Breakpoint-Trap aus. Kehrt zurueck — genau das ist der Beweis,
/// dass Trap-Tore korrekt fortsetzen.
pub fn breakpoint() {
    imp::breakpoint()
}

/// Greift auf eine nicht abgebildete Adresse zu.
///
/// # Safety
/// Loest garantiert einen Page Fault aus; der Kernel haelt danach an.
pub unsafe fn page_fault() -> ! {
    unsafe { imp::page_fault() }
}

/// Versucht, in den nur lesbaren Teil des Kernelabbilds zu schreiben. Beweist,
/// dass die Rechte der Seitentabellen wirklich gelten.
///
/// # Safety
/// Loest einen Page Fault aus, sofern die Rechte wirken; der Kernel haelt an.
pub unsafe fn rodata_write() -> ! {
    unsafe { imp::rodata_write() }
}

/// Erzwingt einen ECHTEN Double Fault.
///
/// # Safety
/// Der Kernel haelt danach an.
pub unsafe fn double_fault() -> ! {
    unsafe { imp::double_fault() }
}
