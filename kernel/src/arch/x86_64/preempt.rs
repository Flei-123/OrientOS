//! Verdraengender Zeitgeberpfad von x86_64 — **Baustelle des Moduls
//! "preempt"**.
//!
//! Hier gehoert hin (und NUR hier):
//! * ein Interrupt-Einsprung fuer IRQ0, der den VOLLEN Registersatz rettet
//!   (alle 15 GPR plus den von der CPU gelegten Rahmen), nicht nur die
//!   callee-saved Register wie [`super::context`],
//! * das Umschalten auf den naechsten Kontext, wenn
//!   [`crate::kcore::preempt::on_tick`] das verlangt,
//! * die Rueckkehr ueber den regulaeren Interrupt-Ruecksprung.
//!
//! Solange nichts davon steht, meldet dieses Modul ehrlich "nicht verfuegbar";
//! der Kernel laeuft dann kooperativ weiter. Das ist Absicht: ein halb
//! verdrahteter Interruptpfad waere ein Totalausfall, keine Zwischenstufe.

/// Kann diese Architektur verdraengen?
pub fn available() -> bool {
    false
}

/// Schaltet den verdraengenden Zeitgeberpfad ein bzw. aus.
///
/// Liefert den Zustand DANACH.
///
/// # Safety
/// Veraendert den Interruptpfad; nur mit lauffaehigem Scheduler aufrufen.
pub unsafe fn set_enabled(_on: bool) -> bool {
    false
}
