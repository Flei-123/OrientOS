//! Startdateisystem (Initramfs) — **Baustelle des Moduls "elf"**.
//!
//! **Schicht: core.** Der Kernel bekommt vom Bootloader einen zusammen-
//! haengenden Speicherbereich mit einem Archiv darin. Diese Datei kennt das
//! Archivformat und liefert die Dateien als Byte-Scheiben; sie laedt nichts
//! und bildet nichts ab.
//!
//! Die Formatwahl (und ihre Begruendung) gehoert nach FILESYSTEM.md. Bis das
//! Modul "elf" das Format implementiert, meldet diese Datei ehrlich, ob und
//! wie gross ein Archiv uebergeben wurde — mehr nicht.

use crate::kcore::mem::VirtAddr;
use crate::klog;

static mut BASE: u64 = 0;
static mut LEN: usize = 0;

/// Uebernimmt den vom Bootloader gelieferten Archivbereich.
///
/// # Safety
/// `base`/`len` muessen einen gueltigen, bis zum Ende der Laufzeit gueltigen
/// Bereich beschreiben (Limine-Module liegen dauerhaft im Speicher).
pub unsafe fn set(base: VirtAddr, len: usize) {
    unsafe {
        BASE = base.as_u64();
        LEN = len;
    }
}

/// Der Archivbereich als Byte-Scheibe, falls einer uebergeben wurde.
pub fn image() -> Option<&'static [u8]> {
    // SAFETY: nur lesend; der Bereich stammt aus [`set`] und bleibt gueltig.
    unsafe {
        let (b, l) = (BASE, LEN);
        if b == 0 || l == 0 {
            None
        } else {
            Some(core::slice::from_raw_parts(b as *const u8, l))
        }
    }
}

/// Sucht eine Datei im Archiv.
///
/// Bis das Archivformat implementiert ist, findet diese Funktion nichts —
/// aber sie stuerzt auch nicht ab.
pub fn find(_name: &str) -> Option<&'static [u8]> {
    None
}

/// Anzahl der Eintraege im Archiv.
pub fn count() -> usize {
    0
}

/// Boot-Meldung: was der Bootloader geliefert hat.
pub fn report() {
    match image() {
        Some(b) => klog!("init", "Initramfs   : {} Eintraege, {} B", count(), b.len()),
        None => klog!("init", "Initramfs   : kein Archiv uebergeben"),
    }
}
