//! Schedulergrenze der core-Schicht **und** ein kooperativer Scheduler.
//!
//! **Schicht: core.** Hier steht, WAS ein Scheduler koennen muss und WER als
//! naechstes laufen darf — nicht, wie ein Registersatz gerettet wird. Das
//! Umschalten der Register ist Sache von `arch` (siehe
//! `arch/<name>/context.rs`, Trait [`ContextOps`]). In dieser Datei kommt
//! deshalb kein einziger Registername vor.
//!
//! Modell (Phase 2, kooperativ):
//!
//! * Der Scheduler laeuft auf dem Stapel seines Aufrufers (`kmain`) und ist
//!   selbst ein Kontext ([`CoopScheduler`] Feld `main`).
//! * Jeder Thread bekommt einen **eigenen Kernelstapel** aus dem Heap.
//! * [`yield_now`] wechselt vom laufenden Thread zurueck in den Scheduler,
//!   der reihum den naechsten lauffaehigen Thread waehlt (Round-Robin).
//! * [`exit_current`] meldet den laufenden Thread ab; sein Stapel wird
//!   eingezogen, sobald der Scheduler wieder laeuft.
//! * [`block_current`] legt den laufenden Thread schlafen, bis ihn jemand mit
//!   [`unblock`] weckt; [`sleep_ticks`] legt ihn fuer eine Anzahl Zeitgeber-
//!   Ticks schlafen und wird vom Scheduler selbst wieder geweckt.
//! * Sind alle Threads fertig, kehrt [`Scheduler::run`] nach `kmain` zurueck.
//!   Schlafen alle uebrigen Threads ohne Weckzeit, meldet `run` eine
//!   **Verklemmung** und kehrt ebenfalls zurueck, statt haengen zu bleiben.
//!
//! Der Scheduler liest den Tickzaehler
//! ([`crate::kcore::arch_iface::TimerOps`]), um Schlaefer faellig zu machen;
//! wecken tut er sie erst, wenn er selbst wieder laeuft. Damit braucht der
//! kooperative Teil weder Sperren noch abgeschaltete Interrupts.
//!
//! Modell (Phase 3, verdraengend — **derselbe** Planer, kein zweiter):
//!
//! * Jeder Thread traegt eine Prioritaetsstufe; seine Zeitscheibe ist
//!   `Grundscheibe * Stufe` Ticks ([`quantum_for`]).
//! * Der Zeitgeberpfad der Architektur bucht jeden Tick ueber [`charge_tick`]
//!   auf den laufenden Thread. Ist dessen Scheibe leer, nimmt ihm
//!   [`preempt_current`] die CPU weg — ohne dass er `yield` gerufen haette.
//! * Verdraengt werden nur Threads aus [`Scheduler::spawn_prio`]. Threads aus
//!   [`Scheduler::spawn`] (die kooperativen Selbsttests) bleiben unberuehrt,
//!   damit ein Thread nicht mitten in einer Protokollzeile unterbrochen wird.
//! * Erzwungene und freiwillige Wechsel werden **getrennt** gezaehlt
//!   ([`crate::kcore::preempt`]).

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::kcore::arch_iface::{ArchOps, ContextOps, TimerOps};
use crate::kcore::mem::VirtAddr;
use crate::klog;

/// Kontexttyp der aktiven Architektur — der Core kennt nur den Trait.
type Ctx = <crate::arch::Active as ArchOps>::Context;

/// Zeitgeber der aktiven Architektur — der Core kennt nur [`TimerOps`].
type Clock = crate::arch::Timer;

/// Aktueller Stand des Zeitgebers in Ticks.
fn now() -> u64 {
    <Clock as TimerOps>::ticks()
}

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

/// Warum ein blockierter Thread wieder lauffaehig wird.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WakeReason {
    /// Nur durch [`Scheduler::unblock`] — der Scheduler weckt ihn nie von selbst.
    Explicit,
    /// Sobald der Zeitgeber diesen Tickstand erreicht hat.
    AtTick(u64),
}

/// Ein Scheduler. Architekturneutral: er entscheidet nur, WER als naechstes
/// laeuft, und stoesst den Wechsel ueber die arch-Seite an.
pub trait Scheduler {
    /// Nimmt einen lauffaehigen Thread mit eigenem Stapel auf.
    ///
    /// Liefert `None`, wenn der Stapel zu klein waere oder der Scheduler
    /// bereits laeuft (waehrend `run` darf niemand nachlegen, sonst koennte
    /// die Threadtabelle umziehen, waehrend Kontexte auf sie zeigen).
    fn spawn(&mut self, entry: extern "C" fn() -> !, stack_bytes: usize) -> Option<ThreadId>;
    /// Wie [`Scheduler::spawn`], aber mit Prioritaet — und der Thread laesst
    /// sich verdraengen.
    ///
    /// Die Prioritaet bemisst die Zeitscheibe: ein Thread der Stufe `p`
    /// bekommt `p`-mal so viele Ticks am Stueck wie einer der Stufe 1. Sie
    /// wird auf [`PRIO_MIN`]..=[`PRIO_MAX`] begrenzt.
    fn spawn_prio(
        &mut self,
        entry: extern "C" fn() -> !,
        stack_bytes: usize,
        prio: u8,
    ) -> Option<ThreadId>;
    /// Gerade laufender Thread (der Scheduler selbst hat die Kennung 0).
    fn current(&self) -> ThreadId;
    /// Bucht einen Zeitgebertick auf das Konto des laufenden Threads.
    ///
    /// Liefert `None`, wenn gerade kein Thread laeuft (dann ist nichts zu
    /// verdraengen), sonst `Some(true)`, wenn dessen Zeitscheibe damit
    /// aufgebraucht ist. Wird aus dem Interruptpfad gerufen und darf deshalb
    /// weder blockieren noch Speicher anfordern.
    fn charge_tick(&mut self) -> Option<bool>;
    /// Nimmt dem laufenden Thread die CPU weg, ohne dass er das wollte.
    ///
    /// Kehrt zurueck, sobald der Thread wieder an der Reihe ist. Ein Thread,
    /// der Verdraengung nicht zulaesst, bleibt unberuehrt.
    ///
    /// # Safety
    /// Wechselt den Stapel; nur aus dem Zeitgeberpfad der Architektur heraus
    /// aufrufen, und nur waehrend ein Thread dieses Planers laeuft.
    unsafe fn preempt_current(&mut self);
    /// Gibt die CPU freiwillig ab und laesst den naechsten Thread laufen.
    ///
    /// # Safety
    /// Wechselt den Stapel; darf nur aus einem Thread dieses Schedulers heraus
    /// aufgerufen werden.
    unsafe fn yield_now(&mut self);
    /// Markiert einen Thread als beendet.
    fn exit(&mut self, id: ThreadId);
    /// Legt den laufenden Thread schlafen und gibt die CPU ab. Er wird erst
    /// wieder lauffaehig, wenn [`Scheduler::unblock`] ihn weckt bzw. der
    /// Zeitgeber die Weckzeit erreicht hat.
    ///
    /// # Safety
    /// Wechselt den Stapel; nur aus einem Thread dieses Schedulers heraus.
    unsafe fn block_current(&mut self, wake: WakeReason);
    /// Macht einen blockierten Thread wieder lauffaehig. Liefert `false`, wenn
    /// es den Thread nicht gibt oder er gar nicht blockiert war.
    fn unblock(&mut self, id: ThreadId) -> bool;
    /// Anzahl der verwalteten Threads.
    fn count(&self) -> usize;
    /// Anzahl der gerade blockierten Threads.
    fn blocked(&self) -> usize;
    /// Laesst alle lauffaehigen Threads reihum laufen und kehrt zurueck,
    /// sobald keiner mehr lauffaehig ist. Ergebnis: Anzahl der Wechsel.
    ///
    /// # Safety
    /// Wechselt Stapel. Nur aus dem Kontrollfluss aufrufen, der den Scheduler
    /// besitzt, und nicht rekursiv.
    unsafe fn run(&mut self) -> usize;
}

/// Niedrigste Prioritaetsstufe (kuerzeste Zeitscheibe).
pub const PRIO_MIN: u8 = 1;
/// Hoechste Prioritaetsstufe (laengste Zeitscheibe).
pub const PRIO_MAX: u8 = 8;

/// Zeitscheibe eines Threads in Ticks: Grundscheibe mal Prioritaetsstufe.
///
/// Damit wirkt die Prioritaet **messbar** und trotzdem verhungert niemand —
/// jeder Thread kommt in jeder Runde dran, nur unterschiedlich lange.
fn quantum_for(prio: u8) -> u32 {
    let p = prio.clamp(PRIO_MIN, PRIO_MAX) as u64;
    (crate::kcore::preempt::quantum() * p).max(1) as u32
}

/// Muster, mit dem ein frischer Stapel gefuellt wird. Am unteren Ende muss es
/// nach dem Lauf noch stehen — sonst gab es einen Stapelueberlauf.
const STACK_FILL: u8 = 0xa5;
/// So viele Bytes am unteren Stapelende werden als Wachzone geprueft.
const GUARD_BYTES: usize = 64;

/// Ein Thread: Kennung, Zustand, gesicherter Kontext und sein eigener Stapel.
struct Thread {
    /// Kennung (ab 1; 0 gehoert dem Scheduler selbst).
    id: ThreadId,
    /// Aktueller Zustand.
    state: ThreadState,
    /// Gesicherter Kontext, solange der Thread nicht laeuft.
    ctx: Ctx,
    /// Eigener Kernelstapel. `None`, sobald er eingezogen wurde.
    stack: Option<Box<[u8]>>,
    /// Wie oft dieser Thread die CPU bekommen hat.
    slices: usize,
    /// Weckbedingung, solange `state == Blocked`.
    wake: WakeReason,
    /// Prioritaetsstufe ([`PRIO_MIN`]..=[`PRIO_MAX`]); bemisst die Zeitscheibe.
    prio: u8,
    /// Verbleibende Ticks der laufenden Zeitscheibe.
    left: u32,
    /// Wie viele Zeitgebertick(s) dieser Thread verbraucht hat.
    ticks: u64,
    /// Darf der Zeitgeber diesem Thread die CPU wegnehmen?
    ///
    /// Nur Threads aus [`Scheduler::spawn_prio`] lassen das zu. Die
    /// kooperativen Selbsttests laufen damit weiterhin deterministisch, und
    /// ein Thread, der eine Sperre halten koennte (z. B. um zu protokollieren),
    /// wird nicht mitten darin unterbrochen.
    preemptible: bool,
}

/// Kooperativer Round-Robin-Scheduler.
pub struct CoopScheduler {
    /// Alle bekannten Threads in Aufnahmereihenfolge.
    threads: Vec<Thread>,
    /// Index des laufenden Threads; `None` = der Scheduler selbst laeuft.
    current: Option<usize>,
    /// Index, ab dem beim naechsten Mal gesucht wird.
    next: usize,
    /// Kontext des Schedulers (der Stapel von `kmain`).
    main: Ctx,
    /// Gezaehlte Kontextwechsel (Hin- und Rueckweg einzeln).
    switches: usize,
    /// Kennung, die der naechste Thread bekommt.
    next_id: u64,
    /// Laeuft gerade [`Scheduler::run`]?
    running: bool,
    /// Wie oft der Scheduler einen Schlaefer zur Weckzeit lauffaehig gemacht hat.
    wakeups: usize,
    /// Wie oft er auf den Zeitgeber gewartet hat, weil niemand lauffaehig war.
    idle_waits: usize,
    /// Hat [`Scheduler::run`] wegen einer Verklemmung abgebrochen?
    deadlocked: bool,
    /// Groesste gemessene Stapelnutzung eines beendeten Threads (Bytes).
    peak_stack: usize,
}

impl CoopScheduler {
    /// Neuer, leerer Scheduler.
    pub fn new() -> Self {
        CoopScheduler {
            threads: Vec::new(),
            current: None,
            next: 0,
            main: Ctx::empty(),
            switches: 0,
            next_id: 1,
            running: false,
            wakeups: 0,
            idle_waits: 0,
            deadlocked: false,
            peak_stack: 0,
        }
    }

    /// Wie oft ein Schlaefer durch die Weckzeit lauffaehig wurde.
    pub fn wakeups(&self) -> usize {
        self.wakeups
    }

    /// Wie oft der Scheduler mangels lauffaehiger Threads auf den Zeitgeber
    /// gewartet hat.
    pub fn idle_waits(&self) -> usize {
        self.idle_waits
    }

    /// Hat der letzte [`Scheduler::run`] wegen einer Verklemmung abgebrochen?
    pub fn deadlocked(&self) -> bool {
        self.deadlocked
    }

    /// Ist die Wachzone am unteren Ende jedes noch vorhandenen Stapels
    /// unversehrt?
    pub fn stacks_intact(&self) -> bool {
        self.threads.iter().all(|t| match &t.stack {
            Some(s) => s[..GUARD_BYTES].iter().all(|&b| b == STACK_FILL),
            None => true,
        })
    }

    /// Wie oft jeder Thread an der Reihe war (in Aufnahmereihenfolge).
    pub fn slices(&self) -> impl Iterator<Item = (ThreadId, usize)> + '_ {
        self.threads.iter().map(|t| (t.id, t.slices))
    }

    /// Prioritaet und verbrauchte Zeitgebertick(s) je Thread — der Nachweis,
    /// dass Prioritaeten wirken.
    pub fn tick_shares(&self) -> impl Iterator<Item = (ThreadId, u8, u64)> + '_ {
        self.threads.iter().map(|t| (t.id, t.prio, t.ticks))
    }

    /// Waehlt reihum den naechsten lauffaehigen Thread.
    fn pick(&mut self) -> Option<usize> {
        let n = self.threads.len();
        for k in 0..n {
            let i = (self.next + k) % n;
            if self.threads[i].state == ThreadState::Ready {
                self.next = (i + 1) % n;
                return Some(i);
            }
        }
        None
    }

    /// Macht alle Schlaefer lauffaehig, deren Weckzeit erreicht ist.
    /// Liefert die Anzahl der geweckten Threads.
    fn wake_due(&mut self) -> usize {
        let jetzt = now();
        let mut n = 0;
        for t in self.threads.iter_mut() {
            if t.state == ThreadState::Blocked {
                if let WakeReason::AtTick(deadline) = t.wake {
                    if jetzt >= deadline {
                        t.state = ThreadState::Ready;
                        t.wake = WakeReason::Explicit;
                        n += 1;
                    }
                }
            }
        }
        self.wakeups += n;
        n
    }

    /// Wartet ein Thread auf eine Weckzeit, die der Zeitgeber noch erreichen
    /// kann?
    fn has_timed_sleeper(&self) -> bool {
        self.threads
            .iter()
            .any(|t| t.state == ThreadState::Blocked && matches!(t.wake, WakeReason::AtTick(_)))
    }

    /// Zieht die Stapel beendeter Threads ein und merkt sich vorher, wie tief
    /// der Thread seinen Stapel wirklich benutzt hat.
    fn reap(&mut self) {
        for t in self.threads.iter_mut() {
            if t.state == ThreadState::Finished {
                if let Some(s) = &t.stack {
                    let unberuehrt = s.iter().take_while(|&&b| b == STACK_FILL).count();
                    let benutzt = s.len() - unberuehrt;
                    if benutzt > self.peak_stack {
                        self.peak_stack = benutzt;
                    }
                }
                t.stack = None;
            }
        }
    }

    /// Gemeinsamer Rumpf von [`Scheduler::spawn`] und [`Scheduler::spawn_prio`].
    fn spawn_inner(
        &mut self,
        entry: extern "C" fn() -> !,
        stack_bytes: usize,
        prio: u8,
        preemptible: bool,
    ) -> Option<ThreadId> {
        if self.running || stack_bytes < Ctx::MIN_STACK_BYTES {
            return None;
        }
        let mut stack = vec![STACK_FILL; stack_bytes].into_boxed_slice();
        let low = stack.as_mut_ptr() as u64;
        let top = VirtAddr(low + stack_bytes as u64);
        // Sicher: `top` ist das obere Ende eines exklusiv besessenen,
        // ausreichend grossen Bereichs, der genau so lange lebt wie der Thread.
        let ctx = unsafe { Ctx::new(top, entry) };
        let id = ThreadId(self.next_id);
        self.next_id += 1;
        self.threads.push(Thread {
            id,
            state: ThreadState::Ready,
            ctx,
            stack: Some(stack),
            slices: 0,
            wake: WakeReason::Explicit,
            prio,
            left: quantum_for(prio),
            ticks: 0,
            preemptible,
        });
        Some(id)
    }

    /// Groesste gemessene Stapelnutzung eines beendeten Threads in Bytes.
    ///
    /// Gemessen am Fuellmuster: alles unterhalb der tiefsten beruehrten Stelle
    /// traegt noch [`STACK_FILL`].
    pub fn peak_stack_bytes(&self) -> usize {
        self.peak_stack
    }
}

impl Default for CoopScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for CoopScheduler {
    fn spawn(&mut self, entry: extern "C" fn() -> !, stack_bytes: usize) -> Option<ThreadId> {
        self.spawn_inner(entry, stack_bytes, PRIO_MIN, false)
    }

    fn spawn_prio(
        &mut self,
        entry: extern "C" fn() -> !,
        stack_bytes: usize,
        prio: u8,
    ) -> Option<ThreadId> {
        self.spawn_inner(entry, stack_bytes, prio.clamp(PRIO_MIN, PRIO_MAX), true)
    }

    fn charge_tick(&mut self) -> Option<bool> {
        let i = self.current?;
        let t = &mut self.threads[i];
        t.ticks += 1;
        if !t.preemptible || t.state != ThreadState::Ready {
            return Some(false);
        }
        if t.left > 1 {
            t.left -= 1;
            Some(false)
        } else {
            t.left = quantum_for(t.prio);
            Some(true)
        }
    }

    unsafe fn preempt_current(&mut self) {
        let i = match self.current {
            Some(i) => i,
            None => return,
        };
        if !self.threads[i].preemptible {
            return;
        }
        self.switches += 1;
        crate::kcore::preempt::note_preemptive_switch();
        let from: *mut Ctx = addr_of_mut!(self.threads[i].ctx);
        let to: *const Ctx = addr_of!(self.main);
        // Sicher: derselbe Wechsel wie bei [`Scheduler::yield_now`] — nur
        // ausgeloest vom Zeitgeber statt vom Thread. `main` gehoert dem Stapel
        // von `run`, der weiterlebt; der Aufrufer ist der Interrupt-Einsprung
        // der Architektur, der seine Register bereits gerettet hat.
        unsafe { Ctx::switch(from, to) };
    }

    fn current(&self) -> ThreadId {
        match self.current {
            Some(i) => self.threads[i].id,
            None => ThreadId(0),
        }
    }

    unsafe fn yield_now(&mut self) {
        let i = match self.current {
            Some(i) => i,
            // Der Scheduler selbst hat niemanden, an den er abgeben koennte.
            None => return,
        };
        self.switches += 1;
        crate::kcore::preempt::note_cooperative_switch();
        let from: *mut Ctx = addr_of_mut!(self.threads[i].ctx);
        let to: *const Ctx = addr_of_mut!(self.main);
        // Sicher: `main` wurde beim Wechsel in diesen Thread gesichert und
        // gehoert einem Stapel, der noch lebt (dem von `run`).
        unsafe { Ctx::switch(from, to) };
    }

    fn exit(&mut self, id: ThreadId) {
        if let Some(t) = self.threads.iter_mut().find(|t| t.id == id) {
            t.state = ThreadState::Finished;
        }
    }

    unsafe fn block_current(&mut self, wake: WakeReason) {
        if let Some(i) = self.current {
            self.threads[i].state = ThreadState::Blocked;
            self.threads[i].wake = wake;
        }
        // Sicher: Bedingung wie bei [`Scheduler::yield_now`] — der Aufrufer ist
        // ein Thread dieses Schedulers.
        unsafe { self.yield_now() };
    }

    fn unblock(&mut self, id: ThreadId) -> bool {
        match self.threads.iter_mut().find(|t| t.id == id) {
            Some(t) if t.state == ThreadState::Blocked => {
                t.state = ThreadState::Ready;
                t.wake = WakeReason::Explicit;
                true
            }
            _ => false,
        }
    }

    fn count(&self) -> usize {
        self.threads.len()
    }

    fn blocked(&self) -> usize {
        self.threads.iter().filter(|t| t.state == ThreadState::Blocked).count()
    }

    unsafe fn run(&mut self) -> usize {
        let before = self.switches;
        self.running = true;
        self.deadlocked = false;
        loop {
            self.wake_due();
            let i = match self.pick() {
                Some(i) => i,
                None => {
                    // Niemand lauffaehig. Entweder sind alle fertig (dann ist
                    // der Lauf zu Ende), oder es wartet jemand auf eine
                    // Weckzeit (dann warten wir stromsparend auf den
                    // Zeitgeber), oder alle warten auf ein Wecken, das
                    // niemand mehr ausloesen kann: Verklemmung.
                    if !self.has_timed_sleeper() {
                        self.deadlocked = self.blocked() > 0;
                        break;
                    }
                    if !crate::arch::interrupts_enabled() {
                        // Ohne Interrupts wird der Tickzaehler nie weiterlaufen
                        // — warten waere ein Haenger.
                        self.deadlocked = true;
                        break;
                    }
                    self.idle_waits += 1;
                    crate::arch::wait_for_interrupt();
                    continue;
                }
            };
            self.current = Some(i);
            self.threads[i].slices += 1;
            // Frische Zeitscheibe: ein Thread, der gerade verdraengt wurde,
            // faengt beim naechsten Zug wieder von vorne an.
            self.threads[i].left = quantum_for(self.threads[i].prio);
            self.switches += 1;
            let from: *mut Ctx = addr_of_mut!(self.main);
            let to: *const Ctx = addr_of_mut!(self.threads[i].ctx);
            // Sicher: `to` ist entweder ein frischer Startkontext oder wurde
            // beim letzten `yield_now` desselben Threads gesichert; sein Stapel
            // lebt, solange der Thread nicht `Finished` ist.
            unsafe { Ctx::switch(from, to) };
            self.current = None;
            self.reap();
        }
        self.running = false;
        self.switches - before
    }
}

// ------------------------------------------------------------ globaler Zugriff
//
// Threads bekommen keine Argumente (`extern "C" fn() -> !`) und koennen den
// Scheduler daher nur ueber eine bekannte Stelle erreichen. Das ist zulaessig,
// weil dieser Kernel einfaedig laeuft und der Scheduler kooperativ ist: es gibt
// immer genau einen Kontrollfluss, der diesen Zeiger benutzt.

/// Zeiger auf den gerade laufenden Scheduler (nur waehrend [`with_active`]).
static mut ACTIVE: *mut CoopScheduler = core::ptr::null_mut();

/// Fuehrt `f` mit `s` als "aktivem" Scheduler aus, sodass dessen Threads ihn
/// ueber [`yield_now`] und [`exit_current`] erreichen.
fn with_active<R>(s: &mut CoopScheduler, f: impl FnOnce(&mut CoopScheduler) -> R) -> R {
    let ptr: *mut CoopScheduler = s;
    // Sicher: einfaedig, kein zweiter Schreiber; der Zeiger wird nach `f`
    // wieder zurueckgesetzt.
    unsafe { ACTIVE = ptr };
    let r = f(s);
    unsafe { ACTIVE = core::ptr::null_mut() };
    r
}

/// Der aktive Scheduler, oder ein Nullzeiger.
fn active() -> *mut CoopScheduler {
    // Sicher: nur Lesen eines Zeigerwortes.
    unsafe { ACTIVE }
}

/// Gibt die CPU aus einem Thread heraus freiwillig ab.
///
/// Ausserhalb eines Threads (oder ohne aktiven Scheduler) passiert nichts.
pub fn yield_now() {
    let s = active();
    if s.is_null() {
        return;
    }
    // Sicher: `s` zeigt auf den Scheduler, der diesen Thread gestartet hat und
    // waehrend des gesamten Laufs an derselben Stelle liegt.
    unsafe { (*s).yield_now() };
}

/// Bucht einen Zeitgebertick auf den laufenden Thread des aktiven Planers.
///
/// Wird aus dem Interruptpfad gerufen (siehe [`crate::kcore::preempt::on_tick`])
/// und liefert `Some(true)`, wenn dessen Zeitscheibe damit aufgebraucht ist.
/// `None` heisst: es laeuft gerade kein Thread, es gibt nichts zu verdraengen.
pub fn charge_tick() -> Option<bool> {
    let s = active();
    if s.is_null() {
        return None;
    }
    // Sicher: einfaedig; der Zeiger zeigt auf den Planer, der gerade laeuft,
    // und der Interrupt unterbricht genau dessen Thread.
    unsafe { (*s).charge_tick() }
}

/// Nimmt dem laufenden Thread die CPU weg (Aufruf aus dem Zeitgeberpfad).
///
/// Liefert `false`, wenn es nichts zu verdraengen gab.
pub fn preempt_current() -> bool {
    let s = active();
    if s.is_null() {
        return false;
    }
    // Sicher: siehe [`charge_tick`]; der Wechsel kehrt zurueck, sobald der
    // verdraengte Thread wieder an der Reihe ist.
    unsafe {
        if (*s).current() == ThreadId(0) {
            return false;
        }
        (*s).preempt_current();
    }
    true
}

/// Kennung des laufenden Threads (0 = kein Scheduler bzw. der Scheduler selbst).
pub fn current_id() -> ThreadId {
    let s = active();
    if s.is_null() {
        return ThreadId(0);
    }
    // Sicher: siehe [`yield_now`].
    unsafe { (*s).current() }
}

/// Legt den laufenden Thread schlafen, bis ihn jemand mit [`unblock`] weckt.
///
/// Ausserhalb eines Threads passiert nichts.
pub fn block_current() {
    let s = active();
    if s.is_null() {
        return;
    }
    // Sicher: siehe [`yield_now`].
    unsafe { (*s).block_current(WakeReason::Explicit) };
}

/// Legt den laufenden Thread fuer mindestens `ticks` Zeitgeber-Ticks schlafen.
///
/// `0` verhaelt sich wie [`yield_now`]. Ausserhalb eines Threads passiert nichts.
pub fn sleep_ticks(ticks: u64) {
    let s = active();
    if s.is_null() {
        return;
    }
    if ticks == 0 {
        yield_now();
        return;
    }
    let deadline = now() + ticks;
    // Sicher: siehe [`yield_now`].
    unsafe { (*s).block_current(WakeReason::AtTick(deadline)) };
}

/// Weckt einen blockierten Thread des aktiven Schedulers.
pub fn unblock(id: ThreadId) -> bool {
    let s = active();
    if s.is_null() {
        return false;
    }
    // Sicher: siehe [`yield_now`].
    unsafe { (*s).unblock(id) }
}

/// Meldet den laufenden Thread ab; kehrt nie zurueck.
pub fn exit_current() -> ! {
    let s = active();
    if !s.is_null() {
        // Sicher: siehe [`yield_now`].
        unsafe {
            let id = (*s).current();
            (*s).exit(id);
            (*s).yield_now();
        }
    }
    // Ein `Finished`-Thread wird nie wieder eingelastet; hierher kommt nur, wer
    // ohne Scheduler laeuft.
    crate::arch::halt_forever()
}

// ------------------------------------------------------------------ Selbsttest

/// Wie oft sich jeder Testthread meldet.
const ROUNDS: usize = 3;
/// Stapelgroesse der Testthreads.
const TEST_STACK: usize = 16 * 1024;
/// Platz fuer die aufgezeichnete Laufreihenfolge.
const TRACE_MAX: usize = 16;

/// Aufgezeichnete Laufreihenfolge (Threadkennungen als Ziffern).
static mut TRACE: [u8; TRACE_MAX] = [0; TRACE_MAX];
/// Belegte Stellen in [`TRACE`].
static mut TRACE_LEN: usize = 0;

/// Haengt eine Kennung an die Laufspur an.
fn trace(id: u64) {
    // Sicher: einfaedig und kooperativ — es schreibt immer nur ein
    // Kontrollfluss, und kein Interrupt-Handler fasst diese Daten an.
    unsafe {
        let len = TRACE_LEN;
        if len < TRACE_MAX {
            addr_of_mut!(TRACE).cast::<u8>().add(len).write(b'0' + (id % 10) as u8);
            TRACE_LEN = len + 1;
        }
    }
}

/// Rumpf beider Testthreads: melden, abgeben, am Ende sauber abmelden.
fn thread_body(name: &str) -> ! {
    for runde in 1..=ROUNDS {
        let s = active();
        // Sicher: der Scheduler laeuft, solange dieser Thread laeuft.
        let id = unsafe { (*s).current() };
        trace(id.0);
        klog!("sched", "Thread {} (Kennung {}): Durchlauf {}/{}", name, id.0, runde, ROUNDS);
        yield_now();
    }
    exit_current()
}

/// Erster Testthread.
extern "C" fn thread_a() -> ! {
    thread_body("A")
}

/// Zweiter Testthread.
extern "C" fn thread_b() -> ! {
    thread_body("B")
}

/// Selbsttest der Threadumschaltung im Boot.
///
/// Startet zwei Kernel-Threads mit eigenen Stapeln, laesst sie abwechselnd ins
/// Boot-Log schreiben und kehrt danach nach `kmain` zurueck. Liefert `true`,
/// wenn alle fuenf Zusagen erfuellt sind:
///
/// 1. beide Threads liefen genau [`ROUNDS`] + 1 Zeitscheiben,
/// 2. sie wechselten sich strikt ab (Spur `121212`),
/// 3. die Zahl der Wechsel stimmt mit dem Modell ueberein,
/// 4. keine Stapelwachzone wurde beruehrt,
/// 5. ein zu kleiner Stapel wird abgewiesen.
pub fn boot_selftest() -> bool {
    let mut s = CoopScheduler::new();
    let a = s.spawn(thread_a, TEST_STACK);
    let b = s.spawn(thread_b, TEST_STACK);
    if a.is_none() || b.is_none() {
        klog!("sched", "FEHLER: konnte keine zwei Threads anlegen (Heap?)");
        return false;
    }
    // Zu kleiner Stapel wird abgewiesen — der Fehlerpfad ist erreichbar.
    let zwerg = s.spawn(thread_a, 8).is_none();

    let wechsel = with_active(&mut s, |s| {
        // Sicher: `run` laeuft genau einmal, aus dem Kontext von `kmain`.
        unsafe { s.run() }
    });

    let mut slices_ok = s.count() == 2;
    for (_, n) in s.slices() {
        slices_ok &= n == ROUNDS + 1; // ROUNDS Durchlaeufe + einmal zum Abmelden
    }
    // Sicher: einfaedig, der Lauf ist beendet.
    let spur_len = unsafe { TRACE_LEN };
    let spur = unsafe { core::slice::from_raw_parts(addr_of!(TRACE).cast::<u8>(), spur_len) };
    let wechselnd = spur_len == 2 * ROUNDS
        && spur.iter().enumerate().all(|(i, &c)| c == if i % 2 == 0 { b'1' } else { b'2' });
    // Jeder Thread wird (ROUNDS + 1)-mal eingelastet und gibt ebenso oft ab.
    let wechsel_ok = wechsel == 2 * 2 * (ROUNDS + 1);
    let stapel_ok = s.stacks_intact();

    let zusagen = (slices_ok as u8)
        + (wechselnd as u8)
        + (wechsel_ok as u8)
        + (stapel_ok as u8)
        + (zwerg as u8);
    if zusagen != 5 {
        klog!(
            "sched",
            "FEHLER: Zusagen verletzt (Scheiben {}, Wechselspiel {}, Zahl {}, Stapel {}, Mindeststapel {})",
            slices_ok,
            wechselnd,
            wechsel_ok,
            stapel_ok,
            zwerg
        );
    }
    klog!(
        "sched",
        "Threadwechsel: {} Wechsel, zurueck in kmain ({} Threads a {} KiB Stapel, Spur {}, {}/5 Zusagen)",
        wechsel,
        s.count(),
        TEST_STACK / 1024,
        core::str::from_utf8(spur).unwrap_or("?"),
        zusagen
    );
    let schlaf_ok = sleep_selftest();
    let verklemmung_ok = deadlock_selftest();
    schlaf_ok && verklemmung_ok && zusagen == 5
}

// ------------------------------------------------- Selbsttest Schlafen/Wecken

/// So viele Ticks schlaeft der Schlaefer-Thread.
const SLEEP_TICKS: u64 = 3;

/// Tatsaechlich verstrichene Ticks des Schlaefers.
static SLEPT: AtomicU64 = AtomicU64::new(0);
/// Fortlaufende Nummer, um die Reihenfolge der Ereignisse zu belegen.
static ORDER: AtomicUsize = AtomicUsize::new(0);
/// Reihenfolgenummer, als der Schlaefer den Wartenden geweckt hat (0 = nie).
static WOKE_AT: AtomicUsize = AtomicUsize::new(0);
/// Reihenfolgenummer, als der Wartende wieder lief (0 = nie).
static RESUMED_AT: AtomicUsize = AtomicUsize::new(0);
/// Kennung des wartenden Threads, sobald bekannt.
static WAITER: AtomicU64 = AtomicU64::new(0);

/// Schlaefer: schlaeft echte Zeitgeber-Ticks ab und weckt danach den Wartenden.
extern "C" fn thread_sleeper() -> ! {
    let vorher = now();
    klog!("sched", "Thread S (Kennung {}): schlaeft {} Ticks", current_id().0, SLEEP_TICKS);
    sleep_ticks(SLEEP_TICKS);
    let verstrichen = now() - vorher;
    SLEPT.store(verstrichen, Ordering::Relaxed);
    klog!("sched", "Thread S: nach {} Ticks aufgewacht", verstrichen);
    let waiter = ThreadId(WAITER.load(Ordering::Relaxed));
    if unblock(waiter) {
        WOKE_AT.store(ORDER.fetch_add(1, Ordering::Relaxed) + 1, Ordering::Relaxed);
        klog!("sched", "Thread S: weckt Thread W (Kennung {})", waiter.0);
    }
    exit_current()
}

/// Wartender: blockiert ohne Weckzeit und laeuft erst weiter, wenn ihn der
/// Schlaefer weckt.
extern "C" fn thread_waiter() -> ! {
    WAITER.store(current_id().0, Ordering::Relaxed);
    klog!("sched", "Thread W (Kennung {}): blockiert bis zum Wecken", current_id().0);
    block_current();
    RESUMED_AT.store(ORDER.fetch_add(1, Ordering::Relaxed) + 1, Ordering::Relaxed);
    klog!("sched", "Thread W: geweckt, laeuft weiter");
    exit_current()
}

/// Selbsttest von Blockieren, Schlafen und Wecken.
///
/// Zwei Threads: einer schlaeft echte Zeitgeber-Ticks ab (der Scheduler wartet
/// derweil stromsparend), der andere blockiert ohne Weckzeit und wird vom
/// Schlaefer geweckt. Fuenf Zusagen:
///
/// 1. der Schlaefer hat mindestens [`SLEEP_TICKS`] Ticks geschlafen,
/// 2. der Scheduler hat genau einen Schlaefer zur Weckzeit lauffaehig gemacht,
/// 3. er hat dabei mindestens einmal auf den Zeitgeber gewartet,
/// 4. der Wartende lief erst **nach** dem Wecken wieder,
/// 5. keine Verklemmung, am Ende ist kein Thread mehr blockiert.
fn sleep_selftest() -> bool {
    SLEPT.store(0, Ordering::Relaxed);
    ORDER.store(0, Ordering::Relaxed);
    WOKE_AT.store(0, Ordering::Relaxed);
    RESUMED_AT.store(0, Ordering::Relaxed);
    WAITER.store(0, Ordering::Relaxed);

    let mut s = CoopScheduler::new();
    // Reihenfolge: der Wartende muss zuerst laufen, damit er seine Kennung
    // hinterlegt und blockiert, bevor der Schlaefer ihn wecken will.
    if s.spawn(thread_waiter, TEST_STACK).is_none() || s.spawn(thread_sleeper, TEST_STACK).is_none()
    {
        klog!("sched", "FEHLER: Schlaftest konnte keine Threads anlegen (Heap?)");
        return false;
    }
    with_active(&mut s, |s| {
        // Sicher: genau ein Lauf, aus dem Kontext von `kmain`.
        unsafe { s.run() }
    });

    let geschlafen = SLEPT.load(Ordering::Relaxed);
    let woke = WOKE_AT.load(Ordering::Relaxed);
    let resumed = RESUMED_AT.load(Ordering::Relaxed);
    let schlaf_ok = geschlafen >= SLEEP_TICKS;
    let weck_ok = s.wakeups() == 1;
    let idle_ok = s.idle_waits() > 0;
    let reihenfolge_ok = woke > 0 && resumed > woke;
    let sauber_ok = !s.deadlocked() && s.blocked() == 0 && s.stacks_intact();
    // Fehlerpfade: unbekannte Kennung und bereits beendeter Thread lassen sich
    // nicht wecken; ausserhalb eines Threads sind die globalen Helfer folgenlos.
    let unbekannt_ok = !s.unblock(ThreadId(0)) && !s.unblock(ThreadId(4711)) && !unblock(ThreadId(1));
    let ausserhalb_ok = {
        yield_now();
        block_current();
        sleep_ticks(1);
        current_id() == ThreadId(0)
    };
    let zusagen = (schlaf_ok as u8)
        + (weck_ok as u8)
        + (idle_ok as u8)
        + (reihenfolge_ok as u8)
        + (sauber_ok as u8)
        + (unbekannt_ok as u8)
        + (ausserhalb_ok as u8);
    if zusagen != 7 {
        klog!(
            "sched",
            "FEHLER: Schlaftest (Schlafdauer {}, Weckung {}, Warten {}, Reihenfolge {}, Abschluss {}, Fehlerpfade {}, ausserhalb {})",
            schlaf_ok,
            weck_ok,
            idle_ok,
            reihenfolge_ok,
            sauber_ok,
            unbekannt_ok,
            ausserhalb_ok
        );
    }
    klog!(
        "sched",
        "Schlafen/Wecken: {} Ticks geschlafen (>= {}), {} Zeitweckung(en), {} Leerlaufwarten, Wartender geweckt: {}, {}/7 Zusagen",
        geschlafen,
        SLEEP_TICKS,
        s.wakeups(),
        s.idle_waits(),
        if reihenfolge_ok { "ja" } else { "nein" },
        zusagen
    );
    klog!(
        "sched",
        "Stapelnutzung: hoechstens {} von {} B je Thread benutzt, Wachzonen unversehrt",
        s.peak_stack_bytes(),
        TEST_STACK
    );
    zusagen == 7
}

// ----------------------------------------------- Selbsttest Verklemmungsschutz

/// Thread, der sich ohne Weckzeit blockiert und deshalb nie wieder laeuft.
extern "C" fn thread_sleeping_beauty() -> ! {
    klog!("sched", "Thread V (Kennung {}): blockiert ohne Wecker", current_id().0);
    block_current();
    // Unerreichbar: niemand weckt diesen Thread. Faende der Scheduler doch
    // einen Weg hierher, meldete es der Selbsttest ueber diese Zeile.
    klog!("sched", "FEHLER: verklemmter Thread wurde geweckt");
    exit_current()
}

/// Selbsttest des Verklemmungsschutzes.
///
/// Ein einziger Thread blockiert ohne Weckzeit. [`Scheduler::run`] darf dann
/// **nicht** haengen bleiben, sondern muss die Verklemmung melden und nach
/// `kmain` zurueckkehren. Drei Zusagen: Rueckkehr mit gesetzter
/// Verklemmungsmeldung, genau ein blockierter Thread, genau zwei Wechsel
/// (hin und zurueck).
fn deadlock_selftest() -> bool {
    let mut s = CoopScheduler::new();
    if s.spawn(thread_sleeping_beauty, TEST_STACK).is_none() {
        klog!("sched", "FEHLER: Verklemmungstest konnte keinen Thread anlegen (Heap?)");
        return false;
    }
    let wechsel = with_active(&mut s, |s| {
        // Sicher: genau ein Lauf, aus dem Kontext von `kmain`.
        unsafe { s.run() }
    });
    let erkannt = s.deadlocked();
    let blockiert = s.blocked() == 1;
    let wechsel_ok = wechsel == 2;
    let zusagen = (erkannt as u8) + (blockiert as u8) + (wechsel_ok as u8);
    if zusagen != 3 {
        klog!(
            "sched",
            "FEHLER: Verklemmungstest (erkannt {}, blockiert {}, Wechsel {})",
            erkannt,
            blockiert,
            wechsel
        );
    }
    klog!(
        "sched",
        "Verklemmungsschutz: run() kehrte nach {} Wechseln zurueck, {} Thread(s) blockiert, Meldung {}, {}/3 Zusagen",
        wechsel,
        s.blocked(),
        if erkannt { "gesetzt" } else { "fehlt" },
        zusagen
    );
    zusagen == 3
}

// ------------------------------------------------------ Selbsttest Verdraengung

/// So viele Zaehlschleifen laufen im Verdraengungstest gegeneinander.
const PREEMPT_THREADS: usize = 3;
/// So viele Zeitgebertick(s) lang laufen sie (100 Hz -> 0,6 s).
const PREEMPT_TICKS: u64 = 60;
/// Stapelgroesse der Zaehlschleifen.
const PREEMPT_STACK: usize = 16 * 1024;

/// Schleifendurchlaeufe je Zaehlthread.
static SPIN_COUNT: [AtomicU64; PREEMPT_THREADS] =
    [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];
/// Tickstand, ab dem die Zaehlschleifen sich abmelden.
static PREEMPT_DEADLINE: AtomicU64 = AtomicU64::new(0);

/// Rumpf einer Zaehlschleife: **kein** `yield`, **kein** Protokolleintrag,
/// **keine** Speicheranforderung — nur zaehlen, bis die Frist abgelaufen ist.
///
/// Dass sich diese Threads trotzdem abwechseln, kann nur am Zeitgeber liegen.
/// Ohne Protokollierung im Rumpf kann eine Verdraengung auch keine Sperre
/// mitten im Schreiben zerreissen.
fn spin_body(slot: usize) -> ! {
    let frist = PREEMPT_DEADLINE.load(Ordering::Relaxed);
    loop {
        SPIN_COUNT[slot].fetch_add(1, Ordering::Relaxed);
        if now() >= frist {
            break;
        }
    }
    exit_current()
}

/// Zaehlschleife mit der hoechsten Prioritaet.
extern "C" fn spin_a() -> ! {
    spin_body(0)
}

/// Zaehlschleife mit mittlerer Prioritaet.
extern "C" fn spin_b() -> ! {
    spin_body(1)
}

/// Zaehlschleife mit der niedrigsten Prioritaet.
extern "C" fn spin_c() -> ! {
    spin_body(2)
}

/// Selbsttest der Verdraengung — **Baustelle des Moduls "preempt"**.
///
/// Zu belegen sind (jeweils im Boot-Log, maschinell pruefbar):
/// 1. mehrere Threads, die NIE freiwillig abgeben (reine Zaehlschleifen),
///    wechseln sich trotzdem ab,
/// 2. die Zahl der erzwungenen Wechsel steht getrennt von der der freiwilligen,
/// 3. die Verteilung der Ticks je Thread zeigt, dass Prioritaeten wirken.
///
/// Wird nur aufgerufen, wenn die Architektur Verdraengung anbietet
/// ([`crate::kcore::preempt::available`]); liefert `(bestanden, gesamt)`.
pub fn preempt_selftest() -> (usize, usize) {
    // Grundzeitscheibe 1 Tick: die Prioritaetsstufe vervielfacht sie, damit die
    // Verteilung in 0,6 s Laufzeit deutlich messbar wird.
    crate::kcore::preempt::set_quantum(1);
    // Sicher: der Planer dieses Tests ist gleich lauffaehig, und der
    // Interruptpfad fragt ausschliesslich ueber `charge_tick` nach.
    let aktiv = unsafe { crate::kcore::preempt::enable() };
    if !aktiv {
        klog!("sched", "Praeemption : Einschalten abgelehnt — bleibe kooperativ");
        return (0, 5);
    }
    klog!(
        "sched",
        "Praeemption : aktiv, Zeitscheibe {} Tick(s) je Prioritaetsstufe, voller Registersatz im Zeitgebereinsprung",
        crate::kcore::preempt::quantum()
    );

    for c in SPIN_COUNT.iter() {
        c.store(0, Ordering::Relaxed);
    }
    let mut s = CoopScheduler::new();
    let mut angelegt = 0;
    for (i, entry) in [spin_a as extern "C" fn() -> !, spin_b, spin_c].into_iter().enumerate() {
        // Prioritaet 3, 2, 1 — absteigend, damit die Verteilung eindeutig ist.
        if s.spawn_prio(entry, PREEMPT_STACK, (PREEMPT_THREADS - i) as u8).is_some() {
            angelegt += 1;
        }
    }
    if angelegt != PREEMPT_THREADS {
        klog!("sched", "FEHLER: Verdraengungstest konnte keine {} Threads anlegen", PREEMPT_THREADS);
        return (0, 5);
    }

    let p_vorher = crate::kcore::preempt::preemptive_switches();
    let k_vorher = crate::kcore::preempt::cooperative_switches();
    PREEMPT_DEADLINE.store(now() + PREEMPT_TICKS, Ordering::Relaxed);
    with_active(&mut s, |s| {
        // Sicher: genau ein Lauf, aus dem Kontext von `kmain`.
        unsafe { s.run() }
    });
    let erzwungen = crate::kcore::preempt::preemptive_switches() - p_vorher;
    let freiwillig = crate::kcore::preempt::cooperative_switches() - k_vorher;

    klog!(
        "sched",
        "praeemptive Wechsel: {}, kooperative Wechsel: {} (nur die {} Abmeldungen)",
        erzwungen,
        freiwillig,
        PREEMPT_THREADS
    );
    klog!(
        "sched",
        "ohne yield: {} Wechsel zwischen {} Threads (reine Zaehlschleifen, kein einziges yield)",
        erzwungen,
        PREEMPT_THREADS
    );

    let mut ticks = [0u64; PREEMPT_THREADS];
    let mut prios = [0u8; PREEMPT_THREADS];
    let mut alle_liefen = true;
    for (n, (id, prio, t)) in s.tick_shares().enumerate() {
        if n < PREEMPT_THREADS {
            ticks[n] = t;
            prios[n] = prio;
        }
        alle_liefen &= t > 0 && SPIN_COUNT[n.min(PREEMPT_THREADS - 1)].load(Ordering::Relaxed) > 0;
        klog!(
            "sched",
            "Thread {}: Prio {}, {} Ticks, {} Schleifendurchlaeufe",
            id.0,
            prio,
            t,
            SPIN_COUNT[n.min(PREEMPT_THREADS - 1)].load(Ordering::Relaxed)
        );
    }
    let prio_wirkt = ticks[0] > ticks[1] && ticks[1] > ticks[2];
    klog!(
        "sched",
        "Prioritaet wirkt: Thread 1 (Prio {}) {} Ticks > Thread 2 (Prio {}) {} Ticks > Thread 3 (Prio {}) {} Ticks",
        prios[0],
        ticks[0],
        prios[1],
        ticks[1],
        prios[2],
        ticks[2]
    );

    let erzwungen_ok = erzwungen > 0;
    let sauber = !s.deadlocked() && s.blocked() == 0 && s.stacks_intact() && s.count() == PREEMPT_THREADS;
    let zusagen = (aktiv as usize)
        + (erzwungen_ok as usize)
        + (alle_liefen as usize)
        + (prio_wirkt as usize)
        + (sauber as usize);
    if zusagen != 5 {
        klog!(
            "sched",
            "FEHLER: Verdraengung (aktiv {}, erzwungen {}, alle liefen {}, Prioritaet {}, Abschluss {})",
            aktiv,
            erzwungen_ok,
            alle_liefen,
            prio_wirkt,
            sauber
        );
    }
    klog!(
        "sched",
        "Verdraengung: {} erzwungene Wechsel in {} Ticks, {}/5 Zusagen",
        erzwungen,
        PREEMPT_TICKS,
        zusagen
    );
    (zusagen, 5)
}
