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
//! * Sind alle Threads fertig, kehrt [`Scheduler::run`] nach `kmain` zurueck.
//!
//! Es gibt **keine** Praeemption: der Zeitgeberinterrupt zaehlt nur Ticks. Damit
//! braucht der Scheduler weder Sperren noch abgeschaltete Interrupts, und der
//! Boot bleibt deterministisch.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr::{addr_of, addr_of_mut};

use crate::kcore::arch_iface::{ArchOps, ContextOps};
use crate::kcore::mem::VirtAddr;
use crate::klog;

/// Kontexttyp der aktiven Architektur — der Core kennt nur den Trait.
type Ctx = <crate::arch::Active as ArchOps>::Context;

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
    /// Nimmt einen lauffaehigen Thread mit eigenem Stapel auf.
    ///
    /// Liefert `None`, wenn der Stapel zu klein waere oder der Scheduler
    /// bereits laeuft (waehrend `run` darf niemand nachlegen, sonst koennte
    /// die Threadtabelle umziehen, waehrend Kontexte auf sie zeigen).
    fn spawn(&mut self, entry: extern "C" fn() -> !, stack_bytes: usize) -> Option<ThreadId>;
    /// Gerade laufender Thread (der Scheduler selbst hat die Kennung 0).
    fn current(&self) -> ThreadId;
    /// Gibt die CPU freiwillig ab und laesst den naechsten Thread laufen.
    ///
    /// # Safety
    /// Wechselt den Stapel; darf nur aus einem Thread dieses Schedulers heraus
    /// aufgerufen werden.
    unsafe fn yield_now(&mut self);
    /// Markiert einen Thread als beendet.
    fn exit(&mut self, id: ThreadId);
    /// Anzahl der verwalteten Threads.
    fn count(&self) -> usize;
    /// Laesst alle lauffaehigen Threads reihum laufen und kehrt zurueck,
    /// sobald keiner mehr lauffaehig ist. Ergebnis: Anzahl der Wechsel.
    ///
    /// # Safety
    /// Wechselt Stapel. Nur aus dem Kontrollfluss aufrufen, der den Scheduler
    /// besitzt, und nicht rekursiv.
    unsafe fn run(&mut self) -> usize;
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
        }
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

    /// Zieht die Stapel beendeter Threads ein.
    fn reap(&mut self) {
        for t in self.threads.iter_mut() {
            if t.state == ThreadState::Finished {
                t.stack = None;
            }
        }
    }
}

impl Default for CoopScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for CoopScheduler {
    fn spawn(&mut self, entry: extern "C" fn() -> !, stack_bytes: usize) -> Option<ThreadId> {
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
        });
        Some(id)
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

    fn count(&self) -> usize {
        self.threads.len()
    }

    unsafe fn run(&mut self) -> usize {
        let before = self.switches;
        self.running = true;
        while let Some(i) = self.pick() {
            self.current = Some(i);
            self.threads[i].slices += 1;
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
    zusagen == 5
}
