//! Schedulergrenze der core-Schicht.
//!
//! **Schicht: core.** Hier steht ausschliesslich, WAS ein Scheduler koennen
//! muss — nicht, wie ein Registersatz gerettet wird. Das Umschalten der
//! Register ist Sache von `arch` (siehe `arch/<name>/context.rs`).
//!
//! Stand: der Trait ist definiert, eine Implementierung (kooperativer
//! Round-Robin mit eigenen Kernelstapeln) ist Phase 2 der ROADMAP. Solange
//! keine aktiv ist, meldet [`boot_selftest`] das ehrlich ins Boot-Log —
//! es wird nichts behauptet, was nicht laeuft.

use crate::klog;

/// Kennung eines lauffaehigen Kontrollflusses.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ThreadId(pub u64);

/// Zustand eines Threads aus Sicht des Schedulers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThreadState {
    /// Lauffaehig bzw. laeuft gerade.
    Ready,
    /// Freiwillig blockiert.
    Blocked,
    /// Beendet, Stapel darf eingezogen werden.
    Finished,
}

/// Ein Scheduler. Architekturneutral: er entscheidet nur, WER als naechstes
/// laeuft, und stoesst den Wechsel ueber die arch-Seite an.
pub trait Scheduler {
    /// Nimmt einen lauffaehigen Thread auf.
    fn spawn(&mut self, entry: extern "C" fn() -> !, stack_bytes: usize) -> Option<ThreadId>;
    /// Gerade laufender Thread.
    fn current(&self) -> ThreadId;
    /// Gibt die CPU freiwillig ab und laesst den naechsten Thread laufen.
    ///
    /// # Safety
    /// Wechselt den Stapel; darf nur mit gueltigem eigenen Kontext laufen.
    unsafe fn yield_now(&mut self);
    /// Beendet den aktuellen Thread.
    fn exit(&mut self, id: ThreadId);
    /// Anzahl der verwalteten Threads.
    fn count(&self) -> usize;
}

/// Selbsttest der Threadumschaltung im Boot.
///
/// Liefert `true`, wenn ein Scheduler vorhanden ist UND der Test bestanden hat.
/// Ohne Implementierung wird das im Log als solches vermerkt.
pub fn boot_selftest() -> bool {
    klog!("sched", "kein Scheduler aktiv (Phase 2 offen) — Kernel laeuft einzelfaedig");
    false
}
