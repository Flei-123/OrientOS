//! Verdraengender Zeitgeberpfad von x86_64.
//!
//! Hier steht (und NUR hier):
//! * ein Interrupt-Einsprung fuer IRQ0, der den VOLLEN Registersatz rettet
//!   (alle 15 GPR plus den von der CPU gelegten Rahmen), nicht nur die
//!   callee-saved Register wie [`super::context`],
//! * das Umschalten auf den naechsten Kontext, wenn
//!   [`crate::kcore::preempt::on_tick`] das verlangt,
//! * die Rueckkehr ueber den regulaeren Interrupt-Ruecksprung.
//!
//! Ablauf eines Ticks:
//!
//! ```text
//!   IRQ0 -> irq0_entry (naked)          alle 15 GPR auf den Stapel
//!            -> irq0_dispatch           Tick zaehlen, quittieren, fragen
//!               -> kcore::preempt::on_tick()   Zeitscheibe abgelaufen?
//!               -> kcore::preempt::switch_now() wechselt den Kontext
//!            <- (spaeter wieder hier)   GPR zurueck, iretq
//! ```
//!
//! Der Wechsel selbst benutzt bewusst dieselbe Registerrettung wie der
//! kooperative Fall ([`super::context`]): `switch_now` laeuft *innerhalb*
//! dieses Handlers, sichert dessen callee-saved Register auf den Stapel des
//! verdraengten Threads und kehrt genau hierher zurueck, sobald der Thread
//! wieder an der Reihe ist. Danach werden die GPR zurueckgeholt und `iretq`
//! setzt den unterbrochenen Code fort — mit exakt dem Zustand, den er hatte.
//!
//! Zwei Sicherheitsregeln, die dieser Pfad strikt einhaelt:
//! 1. Der Interrupt wird **vor** dem Wechsel quittiert (EOI) — sonst blieben
//!    weitere Ticks aus, solange der verdraengte Thread schlaeft.
//! 2. Gewechselt wird nur, wenn privilegierter Code unterbrochen wurde
//!    (RPL des gesicherten Codesegments == 0). Ein Tick, der ein
//!    unprivilegiertes Programm trifft, wird gezaehlt und dem Wachhund
//!    vorgelegt (`user::note_user_tick`): ein Programm, das sein Zeitbudget
//!    ueberzieht, wird abgeraeumt, damit eine Rechenschleife in Ring 3 den
//!    Kernel nicht anhaelt. Ein Wechsel MIT spaeterer Fortsetzung des
//!    unprivilegierten Programms ist noch nicht moeglich (der Rahmen liegt
//!    auf dem gemeinsamen Eintrittsstapel) und in ROADMAP.md offen gefuehrt.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::{cpu, idt, interrupts, pic};
use crate::kcore::arch_iface::InterruptCtlOps;

/// Laeuft der verdraengende Einsprung gerade in der IDT?
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Ticks, die ein unprivilegiertes Programm getroffen haben.
static USER_TICKS: AtomicU64 = AtomicU64::new(0);

/// Wie oft der Wachhund aus diesem Pfad ein Programm abgeraeumt hat.
static USER_ABORTS: AtomicU64 = AtomicU64::new(0);

/// Ende-Code, den ein vom Wachhund abgeraeumtes Programm bekommt.
///
/// Derselbe Wert wie im nicht verdraengenden Zeitgeberhandler
/// ([`interrupts::timer_irq`]) — der Core soll den Grund nicht daran
/// unterscheiden muessen, welcher Zeitgeberpfad gerade eingetragen ist.
const USER_TIMEOUT_CODE: u64 = 2;

/// Zeitgebertick(s), die bisher unprivilegierten Code unterbrochen haben.
pub fn user_ticks() -> u64 {
    USER_TICKS.load(Ordering::Relaxed)
}

/// Wie oft der Wachhund ein unprivilegiertes Programm abgeraeumt hat.
pub fn user_aborts() -> u64 {
    USER_ABORTS.load(Ordering::Relaxed)
}

/// Der volle Registersatz, wie [`irq0_entry`] ihn auf den Stapel legt.
///
/// Reihenfolge ist die *umgekehrte* Push-Reihenfolge: das zuletzt gepushte
/// Register liegt an der niedrigsten Adresse und steht deshalb hier zuerst.
#[repr(C)]
pub struct FullFrame {
    /// Gerettetes r15.
    pub r15: u64,
    /// Gerettetes r14.
    pub r14: u64,
    /// Gerettetes r13.
    pub r13: u64,
    /// Gerettetes r12.
    pub r12: u64,
    /// Gerettetes r11.
    pub r11: u64,
    /// Gerettetes r10.
    pub r10: u64,
    /// Gerettetes r9.
    pub r9: u64,
    /// Gerettetes r8.
    pub r8: u64,
    /// Gerettetes rdi.
    pub rdi: u64,
    /// Gerettetes rsi.
    pub rsi: u64,
    /// Geretteter Rahmenzeiger rbp.
    pub rbp: u64,
    /// Gerettetes rbx.
    pub rbx: u64,
    /// Gerettetes rdx.
    pub rdx: u64,
    /// Gerettetes rcx.
    pub rcx: u64,
    /// Gerettetes rax.
    pub rax: u64,
    // Ab hier der von der CPU gelegte Rahmen.
    /// Adresse der unterbrochenen Instruktion (von der CPU gelegt).
    pub rip: u64,
    /// Codesegment der unterbrochenen Stelle; RPL 3 heisst unprivilegiert.
    pub cs: u64,
    /// Flags der unterbrochenen Stelle.
    pub rflags: u64,
    /// Stapelzeiger der unterbrochenen Stelle.
    pub rsp: u64,
    /// Stapelsegment der unterbrochenen Stelle.
    pub ss: u64,
}

/// Kann diese Architektur verdraengen?
pub fn available() -> bool {
    true
}

/// Schaltet den verdraengenden Zeitgeberpfad ein bzw. aus.
///
/// Liefert den Zustand DANACH.
///
/// # Safety
/// Veraendert den Interruptpfad; nur mit lauffaehigem Scheduler aufrufen.
pub unsafe fn set_enabled(on: bool) -> bool {
    let vorher = cpu::interrupts_enabled();
    cpu::cli();
    unsafe {
        if on {
            // Ein Funktions-*Item* hat keine Adresse — erst der Zeiger. Deshalb
            // der Umweg ueber die typisierte Bindung (siehe idt::set_handler).
            let entry: unsafe extern "C" fn() = irq0_entry;
            idt::set_gate(interrupts::VEC_TIMER, entry as usize as u64, 0, 0, false);
        } else {
            // Zurueck auf den einfachen, nicht verdraengenden Handler.
            idt::set_handler(interrupts::VEC_TIMER, interrupts::timer_irq, 0, 0, false);
        }
    }
    ACTIVE.store(on, Ordering::Release);
    if vorher {
        cpu::sti();
    }
    on
}

/// Ist der verdraengende Einsprung eingetragen?
pub fn enabled() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// Rust-Seite des Zeitgeberinterrupts mit vollem Registersatz.
///
/// # Safety
/// Wird ausschliesslich von [`irq0_entry`] mit einem gueltigen Zeiger auf den
/// gerade gesicherten Registersatz aufgerufen.
unsafe extern "C" fn irq0_dispatch(frame: *mut FullFrame) {
    super::timer::on_tick();
    // Erst quittieren, dann eventuell wechseln (siehe Modulkommentar).
    unsafe { pic::Pic8259::eoi(0) };
    let faellig = crate::kcore::preempt::on_tick();
    // Sicher: `frame` zeigt auf den Registersatz auf dem eigenen Stapel.
    let (cs, rip, rsp) = unsafe { ((*frame).cs, (*frame).rip, (*frame).rsp) };
    if cs & 3 == 3 {
        // Der Tick hat ein unprivilegiertes Programm getroffen. Umschalten auf
        // einen anderen Thread kann dieser Pfad noch nicht (der Rahmen liegt
        // auf dem gemeinsamen Eintrittsstapel, siehe ROADMAP), aber der
        // Wachhund muss auch hier laufen: sonst haelt eine Rechenschleife in
        // Ring 3 den ganzen Startvorgang an, sobald die Verdraengung
        // eingeschaltet ist.
        USER_TICKS.fetch_add(1, Ordering::Relaxed);
        if super::user::note_user_tick() {
            USER_ABORTS.fetch_add(1, Ordering::Relaxed);
            super::user::note_user_frame(cs, rsp);
            crate::klog!(
                "trap",
                "Zeitbudget von Ring 3 erschoepft (RIP={:#018x}) — Programm wird abgeraeumt",
                rip
            );
            // Der Ruecksprung verlaesst diesen Interrupt ohne `iretq`; das
            // Interruptbit aus dem gesicherten Rahmen wird deshalb nicht
            // wiederhergestellt. Damit der Kernel danach weiter Ticks bekommt,
            // wird es hier von Hand gesetzt — vorher ist der Interrupt bereits
            // quittiert, und der Wachhund meldet sich fuer dasselbe Programm
            // kein zweites Mal (`note_user_tick` prueft `in_user`).
            cpu::sti();
            // SAFETY: wie im nicht verdraengenden Zeitgeberhandler
            // (`interrupts::timer_irq`): der Rahmen stammt aus Ring 3, der
            // Interrupt ist quittiert, und `abort_user` kehrt in den Kernel
            // zurueck, der das Programm gestartet hat.
            unsafe { super::user::abort_user(USER_TIMEOUT_CODE) }
        }
        return;
    }
    if faellig {
        crate::kcore::preempt::switch_now();
    }
}

/// Interrupt-Einsprung fuer IRQ0 mit vollstaendiger Registerrettung.
///
/// Rettet alle 15 Allzweckregister (rax, rcx, rdx, rbx, rbp, rsi, rdi, r8–r15)
/// zusaetzlich zu dem Rahmen, den die CPU selbst gelegt hat (rip, cs, rflags,
/// rsp, ss). Der Stapel ist am Eintritt 16-ausgerichtet (die CPU richtet ihn im
/// long mode vor dem Rahmen aus), nach 5 CPU-Worten und 15 Pushes gilt
/// `rsp % 16 == 0`; der `call` legt die Ruecksprungadresse nach und stellt
/// damit genau die vom SysV-ABI erwartete Ausrichtung her.
///
/// # Safety
/// Nur als Interrupt-Tor eintragen, nie direkt aufrufen.
#[unsafe(naked)]
unsafe extern "C" fn irq0_entry() {
    core::arch::naked_asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rbx",
        "push rbp",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov rdi, rsp",
        "call {dispatch}",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rbp",
        "pop rbx",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
        dispatch = sym irq0_dispatch,
    )
}
