//! Verdraengung (Praeemption) — die core-Seite.
//!
//! **Schicht: core.** Hier steht nur, WAS bei einem Zeitgebertick zu
//! entscheiden ist: laeuft die Zeitscheibe des aktuellen Kontrollflusses ab,
//! und soll deshalb umgeschaltet werden? WIE dabei ein Registersatz gerettet
//! wird, weiss ausschliesslich `arch` (siehe Trait
//! [`crate::kcore::arch_iface::ArchOps::set_preemption`]).
//!
//! Der Ablauf ist bewusst einfach und ohne Sperren:
//!
//! 1. `arch` schaltet den verdraengenden Zeitgeberpfad ein
//!    ([`enable`]) und ruft bei JEDEM Tick [`on_tick`] auf.
//! 2. [`on_tick`] zaehlt den Tick auf das Konto des laufenden Kontrollflusses
//!    und liefert `true`, wenn dessen Zeitscheibe aufgebraucht ist.
//! 3. Nur dann fuehrt `arch` den Wechsel durch und meldet ihn mit
//!    [`note_preemptive_switch`].
//!
//! Kooperative Wechsel (`sched::yield_now`) zaehlen ueber
//! [`note_cooperative_switch`] in einen EIGENEN Zaehler — im Boot-Log muessen
//! beide Arten getrennt sichtbar sein, sonst ist "Praeemption" nicht belegt.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::kcore::arch_iface::ArchOps;

/// Voreingestellte Zeitscheibe in Zeitgebertick(s).
pub const DEFAULT_QUANTUM: u32 = 2;

static ENABLED: AtomicBool = AtomicBool::new(false);
static TICKS: AtomicU64 = AtomicU64::new(0);
static PREEMPTIVE: AtomicU64 = AtomicU64::new(0);
static COOPERATIVE: AtomicU64 = AtomicU64::new(0);
/// Verbleibende Ticks der laufenden Zeitscheibe.
static LEFT: AtomicU64 = AtomicU64::new(DEFAULT_QUANTUM as u64);
/// Zeitscheibe, die nach einem Wechsel neu geladen wird.
static QUANTUM: AtomicU64 = AtomicU64::new(DEFAULT_QUANTUM as u64);

/// Schaltet die Verdraengung ein. Liefert den Zustand DANACH: `false` heisst,
/// dass die Architektur sie (noch) nicht anbietet — der Kernel laeuft dann
/// kooperativ weiter, ohne dass irgendetwas kaputtgeht.
///
/// # Safety
/// Nur aufrufen, wenn ein Scheduler bereit ist, aus dem Interruptpfad heraus
/// befragt zu werden.
pub unsafe fn enable() -> bool {
    let on = unsafe { <crate::arch::Active as ArchOps>::set_preemption(true) };
    ENABLED.store(on, Ordering::Release);
    on
}

/// Schaltet die Verdraengung wieder ab (z. B. vor kritischen Boot-Schritten).
///
/// # Safety
/// Wie [`enable`].
pub unsafe fn disable() {
    let _ = unsafe { <crate::arch::Active as ArchOps>::set_preemption(false) };
    ENABLED.store(false, Ordering::Release);
}

/// Laeuft der verdraengende Pfad gerade?
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// Kann die aktive Architektur ueberhaupt verdraengen?
pub fn available() -> bool {
    <crate::arch::Active as ArchOps>::preemption_available()
}

/// Setzt die Zeitscheibe (in Ticks) fuer den naechsten Wechsel.
pub fn set_quantum(ticks: u32) {
    let t = ticks.max(1) as u64;
    QUANTUM.store(t, Ordering::Relaxed);
}

/// Aktuelle Zeitscheibe in Ticks.
pub fn quantum() -> u64 {
    QUANTUM.load(Ordering::Relaxed)
}

/// Laedt die Zeitscheibe neu — nach jedem Wechsel aufzurufen.
pub fn refill(ticks: u64) {
    LEFT.store(ticks.max(1), Ordering::Relaxed);
}

/// Rueckruf aus dem Zeitgeberpfad der Architektur.
///
/// Liefert `true`, wenn die Zeitscheibe abgelaufen ist und `arch` deshalb
/// umschalten soll. Diese Funktion darf NICHTS tun, was blockieren koennte.
pub fn on_tick() -> bool {
    TICKS.fetch_add(1, Ordering::Relaxed);
    if !ENABLED.load(Ordering::Relaxed) {
        return false;
    }
    // Laeuft gerade ein verdraengbarer Kontrollfluss, bucht der Planer den
    // Tick auf dessen Konto — er allein kennt dessen Zeitscheibe (Groesse nach
    // Prioritaet) und meldet, wenn sie aufgebraucht ist.
    if let Some(scheibe_zuende) = crate::kcore::sched::charge_tick() {
        return scheibe_zuende;
    }
    // Sonst laeuft niemand, den man verdraengen koennte (z. B. der Planer
    // selbst oder ein Thread, der keine Verdraengung zulaesst): Zeitscheibe
    // fuer den naechsten Einlastvorgang frisch laden.
    let left = LEFT.load(Ordering::Relaxed);
    if left > 1 {
        LEFT.store(left - 1, Ordering::Relaxed);
    } else {
        LEFT.store(QUANTUM.load(Ordering::Relaxed), Ordering::Relaxed);
    }
    false
}

/// Fuehrt den vom Zeitgeber verlangten Wechsel durch.
///
/// Wird ausschliesslich aus dem Interruptpfad der Architektur gerufen, und nur
/// wenn [`on_tick`] `true` geliefert hat. WIE gewechselt wird, weiss der
/// Planer; er zaehlt den Wechsel ueber [`note_preemptive_switch`] mit.
pub fn switch_now() {
    crate::kcore::sched::preempt_current();
}

/// Meldet einen aus dem Zeitgeberpfad erzwungenen Wechsel.
pub fn note_preemptive_switch() {
    PREEMPTIVE.fetch_add(1, Ordering::Relaxed);
}

/// Meldet einen freiwilligen Wechsel (`yield`).
pub fn note_cooperative_switch() {
    COOPERATIVE.fetch_add(1, Ordering::Relaxed);
}

/// Anzahl erzwungener Wechsel seit dem Start.
pub fn preemptive_switches() -> u64 {
    PREEMPTIVE.load(Ordering::Relaxed)
}

/// Anzahl freiwilliger Wechsel seit dem Start.
pub fn cooperative_switches() -> u64 {
    COOPERATIVE.load(Ordering::Relaxed)
}

/// Ticks, die der verdraengende Pfad gesehen hat.
pub fn ticks_seen() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Boot-Meldung und Selbsttest der Verdraengung.
///
/// Liefert `(bestanden, gesamt)` fuer die Selbsttestbilanz in `kmain`.
/// Der Kern des Nachweises — mehrere Zaehlschleifen ohne `yield`, die sich
/// trotzdem abwechseln — gehoert in [`crate::kcore::sched`] und wird von hier
/// angestossen.
pub fn boot_selftest() -> (usize, usize) {
    if !available() {
        crate::klog!("sched", "Praeemption : nicht verfuegbar auf {}", crate::arch::NAME);
        return (0, 0);
    }
    crate::kcore::sched::preempt_selftest()
}
