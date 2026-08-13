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

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::kcore::arch_iface::ArchOps;

/// Voreingestellte Zeitscheibe in Zeitgebertick(s).
pub const DEFAULT_QUANTUM: u32 = 2;

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Tiefe der Verdraengungssperre (0 = offen), siehe [`hold`].
static HOLD: AtomicU32 = AtomicU32::new(0);
/// Ticks, die waehrend einer Sperre angefallen sind und noch gebucht werden.
static OWED: AtomicU64 = AtomicU64::new(0);
/// Ticks, die insgesamt in eine Sperre gefallen sind (Statistik).
static HELD_TICKS: AtomicU64 = AtomicU64::new(0);
/// Wechsel, die nach dem Ende einer Sperre nachgeholt wurden.
static DEFERRED_SWITCHES: AtomicU64 = AtomicU64::new(0);
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
    if held() {
        // Verdraengungssperre: waehrend eines geschuetzten Abschnitts fasst
        // dieser Pfad den Planer NICHT an — weder zum Buchen noch zum
        // Wechseln. Der Tick geht nicht verloren, er wird beim Ende der Sperre
        // nachgebucht ([`release`]).
        HELD_TICKS.fetch_add(1, Ordering::Relaxed);
        OWED.fetch_add(1, Ordering::Relaxed);
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

// ------------------------------------------------------- Verdraengungssperre

/// Wache ueber einen Abschnitt, der nicht verdraengt werden darf.
///
/// Solange eine solche Wache lebt, ruehrt der Zeitgeberpfad den Planer nicht
/// an: er bucht keinen Tick und nimmt niemandem die CPU weg. Das ist noetig,
/// wo ein Thread die Datenstruktur des Planers selbst veraendert (z. B.
/// [`crate::kcore::sched::unblock`]) — ein Tick mitten darin haette dieselbe
/// Struktur ein zweites Mal veraendert.
///
/// Die Ticks gehen dabei nicht verloren: sie werden gezaehlt und beim Ende der
/// Wache nachgebucht. Ist die Zeitscheibe dadurch aufgebraucht, wird der
/// Wechsel sofort **nachgeholt** — verzoegerte Verdraengung statt gar keiner.
///
/// Die Wache gilt fuer den laufenden Kontrollfluss und darf deshalb NICHT ueber
/// einen Kontextwechsel hinweg gehalten werden.
#[must_use = "die Sperre gilt nur, solange die Wache lebt"]
pub struct Hold {
    /// Verhindert, dass ausserhalb dieses Moduls eine Wache gebaut wird.
    _privat: (),
}

impl Drop for Hold {
    fn drop(&mut self) {
        release();
    }
}

/// Sperrt die Verdraengung fuer die Lebensdauer der zurueckgelieferten Wache.
///
/// Verschachtelung ist erlaubt; erst die aeusserste Wache gibt wieder frei.
pub fn hold() -> Hold {
    HOLD.fetch_add(1, Ordering::Acquire);
    Hold { _privat: () }
}

/// Laeuft gerade ein geschuetzter Abschnitt?
pub fn held() -> bool {
    HOLD.load(Ordering::Relaxed) > 0
}

/// Gibt die Sperre frei und holt einen faelligen Wechsel nach.
fn release() {
    if HOLD.fetch_sub(1, Ordering::Release) != 1 {
        return;
    }
    let offen = OWED.swap(0, Ordering::Relaxed);
    if offen == 0 {
        return;
    }
    let mut faellig = false;
    for _ in 0..offen {
        if let Some(scheibe_zuende) = crate::kcore::sched::charge_tick() {
            faellig |= scheibe_zuende;
        }
    }
    if faellig {
        DEFERRED_SWITCHES.fetch_add(1, Ordering::Relaxed);
        crate::kcore::sched::preempt_current();
    }
}

/// Ticks, die bisher in eine Verdraengungssperre gefallen sind.
pub fn held_ticks() -> u64 {
    HELD_TICKS.load(Ordering::Relaxed)
}

/// Wechsel, die nach dem Ende einer Sperre nachgeholt wurden.
pub fn deferred_switches() -> u64 {
    DEFERRED_SWITCHES.load(Ordering::Relaxed)
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
