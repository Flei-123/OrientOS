//! Unprivilegierte Ebene von x86_64 — **Baustelle des Moduls "ring3"**.
//!
//! Hier gehoert hin (und NUR hier):
//! * die Modellregister des schnellen Systemaufrufpfads samt Freischaltung,
//! * der Per-CPU-Block und der Wechsel seiner Basis beim Privilegwechsel,
//! * getrennte Kernel-/User-Stapel und der Eintrag im Aufgabensegment,
//! * die Schutzbits gegen Ausfuehren/Anfassen unprivilegierter Seiten, sofern
//!   die CPU sie meldet,
//! * der Ein- und Ruecksprung selbst.
//!
//! Solange nichts davon steht, meldet dieses Modul ehrlich "nicht
//! verfuegbar"; der Kernel bootet dann wie bisher.

use crate::kcore::arch_iface::{CpuFeatures, UserExit};
use crate::kcore::mem::VirtAddr;

/// Was die CPU an Schutzmerkmalen meldet.
pub fn features() -> CpuFeatures {
    CpuFeatures::default()
}

/// Richtet den Systemaufrufpfad ein.
///
/// # Safety
/// Genau einmal im Boot.
pub unsafe fn init() -> bool {
    false
}

/// Ist der Weg in die unprivilegierte Ebene fertig verdrahtet?
pub fn ready() -> bool {
    false
}

/// Hinterlegt den Kernelstapel fuer den naechsten Privilegwechsel.
pub fn set_kernel_stack(_top: VirtAddr) {}

/// Startet `entry` unprivilegiert auf `stack_top`.
///
/// # Safety
/// Beide Adressen muessen unprivilegiert erreichbar abgebildet sein.
pub unsafe fn enter(_entry: VirtAddr, _stack_top: VirtAddr) -> UserExit {
    UserExit { reason: "unprivilegierte Ebene noch nicht eingerichtet", ..UserExit::default() }
}

/// Codesegment-Selektor des laufenden Kontrollflusses.
pub fn code_selector() -> u16 {
    super::gdt::KERNEL_CODE
}
