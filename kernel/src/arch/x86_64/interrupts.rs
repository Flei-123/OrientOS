//! Ausnahme- und Interrupt-Handler von x86_64.
//!
//! Alle Handler benutzen die ABI `extern "x86-interrupt"` (nightly-Feature
//! `abi_x86_interrupt`, begruendet im ARCHITECTURE.md): der Compiler erzeugt
//! damit selbst Prolog/Epilog samt `iretq` und dem korrekten Umgang mit dem
//! Fehlercode. Von Hand geschriebene Stubs waeren fehleranfaelliger.
//!
//! **Ausnahmen aus Ring 3 sind kein Kernelfehler.** Trifft eine Ausnahme ein
//! unprivilegiertes Programm (RPL des gesicherten Codesegments == 3), wird
//! nicht angehalten: der Vorfall wird gemeldet, das Programm abgeraeumt
//! ([`super::user::abort_user`]) und der Kernel laeuft weiter. Nur Ausnahmen
//! aus privilegiertem Code fuehren weiterhin in den Panic-Handler.

use super::{cpu, gdt, idt, pic, timer, user};
use crate::kcore::arch_iface::InterruptCtlOps;

/// Der von der CPU auf den Stack gelegte Rahmen.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrapFrame {
    /// Adresse der unterbrochenen Instruktion.
    pub rip: u64,
    /// Codesegment zum Zeitpunkt der Ausnahme.
    pub cs: u64,
    /// Flags zum Zeitpunkt der Ausnahme.
    pub rflags: u64,
    /// Stapelzeiger zum Zeitpunkt der Ausnahme.
    pub rsp: u64,
    /// Stapelsegment zum Zeitpunkt der Ausnahme.
    pub ss: u64,
}

/// Erste Hardware-IRQ-Nummer nach der Umleitung des PIC.
pub const IRQ_BASE: u8 = 0x20;
/// Vektor des Systemzeitgebers (IRQ0).
pub const VEC_TIMER: u8 = IRQ_BASE;
/// Vektor der Tastatur (IRQ1) — noch ohne Treiber, wird nur quittiert.
pub const VEC_KEYBOARD: u8 = IRQ_BASE + 1;

fn report(name: &str, frame: &TrapFrame, err: Option<u64>) {
    let rip = frame.rip;
    let cs = frame.cs;
    let rflags = frame.rflags;
    let rsp = frame.rsp;
    let ss = frame.ss;
    crate::serial_println!("\n=============== CPU-AUSNAHME ===============");
    crate::serial_println!("Ausnahme : {}", name);
    crate::serial_println!("RIP      : {:#018x}   CS : {:#06x}", rip, cs);
    crate::serial_println!("RSP      : {:#018x}   SS : {:#06x}", rsp, ss);
    crate::serial_println!("RFLAGS   : {:#018x}", rflags);
    if let Some(e) = err {
        crate::serial_println!("Fehlercode: {:#x}", e);
    }
}

/// Uebersetzt den Fehlercode eines Page Faults in Klartext.
fn page_fault_reason(code: u64) -> (&'static str, &'static str, &'static str) {
    let present = if code & 1 != 0 { "Schutzverletzung" } else { "Seite nicht praesent" };
    let access = if code & 2 != 0 { "Schreibzugriff" } else { "Lesezugriff" };
    let mode = if code & 4 != 0 { "Userspace" } else { "Kernel" };
    (present, access, mode)
}

/// Stammt die Ausnahme aus einem laufenden unprivilegierten Programm?
///
/// Beides muss stimmen: der gesicherte Selektor muss RPL 3 tragen UND der
/// Kernel muss wissen, dass er gerade ein Programm gestartet hat. Sonst waere
/// ein verbogener Rahmen ein Weg, den Notausstieg auszuloesen.
fn from_user(frame: &TrapFrame) -> bool {
    frame.cs & 3 == 3 && user::in_user()
}

/// Meldet eine Ausnahme aus Ring 3 und raeumt das Programm ab.
///
/// # Safety
/// Nur aufrufen, wenn [`from_user`] wahr ist.
unsafe fn kill_user(name: &str, frame: &TrapFrame, addr: u64) -> ! {
    user::note_user_frame(frame.cs, frame.rsp);
    crate::kcore::user::note_fault(addr, name, "Userspace");
    unsafe { user::abort_user(1) }
}

macro_rules! fault {
    ($fn_name:ident, $text:expr) => {
        /// Ausnahmehandler.
        pub extern "x86-interrupt" fn $fn_name(frame: TrapFrame) {
            if from_user(&frame) {
                // SAFETY: `from_user` hat bestaetigt, dass ein
                // unprivilegiertes Programm laeuft.
                unsafe { kill_user($text, &frame, frame.rip) }
            }
            report($text, &frame, None);
            panic!("nicht behandelte CPU-Ausnahme: {}", $text);
        }
    };
}

macro_rules! fault_err {
    ($fn_name:ident, $text:expr) => {
        /// Ausnahmehandler mit Fehlercode.
        pub extern "x86-interrupt" fn $fn_name(frame: TrapFrame, code: u64) {
            if from_user(&frame) {
                // SAFETY: wie oben.
                unsafe { kill_user($text, &frame, frame.rip) }
            }
            report($text, &frame, Some(code));
            panic!("nicht behandelte CPU-Ausnahme: {} (code {:#x})", $text, code);
        }
    };
}

fault!(divide_error, "#DE Division durch Null");
fault!(debug_exception, "#DB Debug");
fault!(nmi, "NMI");
fault!(overflow, "#OF Ueberlauf");
fault!(bound_range, "#BR Bereichsueberschreitung");
fault!(invalid_opcode, "#UD ungueltige Instruktion");
fault!(device_not_available, "#NM FPU nicht verfuegbar");
fault!(x87_floating_point, "#MF x87-Gleitkommafehler");
fault!(machine_check_like, "#MC Machine Check");
fault!(simd_floating_point, "#XM SIMD-Gleitkommafehler");
fault!(virtualization, "#VE Virtualisierung");

fault_err!(invalid_tss, "#TS ungueltiges TSS");
fault_err!(segment_not_present, "#NP Segment nicht praesent");
fault_err!(stack_segment_fault, "#SS Stapelsegmentfehler");
fault_err!(general_protection, "#GP allgemeine Schutzverletzung");
fault_err!(alignment_check, "#AC Ausrichtungsfehler");
fault_err!(control_protection, "#CP Kontrollflussschutz");

/// `#BP` — Breakpoint. Kehrt bewusst zurueck: `int3` ist ein Werkzeug,
/// kein Fehler. Wird vom Selbsttest `--test-breakpoint` benutzt.
pub extern "x86-interrupt" fn breakpoint(frame: TrapFrame) {
    let rip = frame.rip;
    if frame.cs & 3 == 3 {
        // Der Messpunkt der unprivilegierten Ebene: Selektor, Privilegstufe
        // und Stapel stehen hier so, wie die HARDWARE sie beim Uebergang
        // gesichert hat — genau das ist der Nachweis, dass Ring 3 echt ist.
        user::note_user_frame(frame.cs, frame.rsp);
        crate::klog!(
            "trap",
            "#BP aus Ring 3: CS={:#06x} (CPL={}), RSP={:#018x}, RIP={:#018x} — setze fort",
            frame.cs,
            frame.cs & 3,
            frame.rsp,
            rip
        );
        return;
    }
    crate::klog!("trap", "#BP Breakpoint bei RIP={:#018x} — setze fort", rip);
}

/// `#PF` — Page Fault. Gibt die Fehleradresse aus CR2 samt Ursache aus.
pub extern "x86-interrupt" fn page_fault(frame: TrapFrame, code: u64) {
    let cr2 = cpu::read_cr2();
    let (present, access, mode) = page_fault_reason(code);
    if from_user(&frame) {
        // Ein unprivilegiertes Programm hat etwas angefasst, das ihm nicht
        // gehoert. Der Kernel meldet den Vorfall und raeumt das Programm ab —
        // anhalten waere hier der Fehler, nicht der Zugriff.
        user::note_user_frame(frame.cs, frame.rsp);
        crate::kcore::user::note_fault(cr2, present, mode);
        crate::klog!(
            "trap",
            "#PF aus Ring 3: CS={:#06x} (CPL={}), RSP={:#018x}, RIP={:#018x}, {}, {}",
            frame.cs,
            frame.cs & 3,
            frame.rsp,
            frame.rip,
            access,
            present
        );
        // SAFETY: `from_user` hat bestaetigt, dass ein unprivilegiertes
        // Programm laeuft und der Kernel einen Ruecksprungpunkt hat.
        unsafe { user::abort_user(1) }
    }
    report("#PF Page Fault", &frame, Some(code));
    crate::serial_println!("Adresse  : {:#018x} (CR2)", cr2);
    crate::serial_println!("Ursache  : {}, {}, ausgeloest im {}", present, access, mode);
    if code & (1 << 4) != 0 {
        crate::serial_println!("Hinweis  : Zugriff war ein Instruktionsabruf (NX verletzt?)");
    }
    panic!("Page Fault an {:#018x}", cr2);
}

/// `#DF` — Double Fault. Laeuft auf einem eigenen IST-Stapel, damit ein
/// zerstoerter Kernelstack nicht sofort zum Triple Fault fuehrt.
pub extern "x86-interrupt" fn double_fault(frame: TrapFrame, code: u64) -> ! {
    report("#DF Double Fault", &frame, Some(code));
    crate::serial_println!(
        "IST-Stapel: {:#018x} (eigener Stack, deshalb lebt der Kernel noch)",
        gdt::double_fault_stack_top()
    );
    crate::serial_println!("Kein Weiterlaufen moeglich — CPU wird angehalten.");
    crate::serial_println!("============================================");
    cpu::halt_forever()
}

/// IRQ0 — Systemzeitgeber.
///
/// Trifft ein Tick ein unprivilegiertes Programm, laeuft ausserdem der
/// Wachhund mit: ein Programm, das sich nicht mehr meldet, wird nach einem
/// festen Zeitbudget abgeraeumt, statt den Startvorgang anzuhalten.
pub extern "x86-interrupt" fn timer_irq(frame: TrapFrame) {
    timer::on_tick();
    unsafe { pic::Pic8259::eoi(0) };
    if from_user(&frame) && user::note_user_tick() {
        user::note_user_frame(frame.cs, frame.rsp);
        crate::klog!(
            "trap",
            "Zeitbudget von Ring 3 erschoepft (RIP={:#018x}) — Programm wird abgeraeumt",
            frame.rip
        );
        // SAFETY: wie im Ausnahmepfad; der Interrupt ist bereits quittiert.
        unsafe { user::abort_user(2) }
    }
}

/// Sammelhandler fuer noch unbenutzte Hardware-IRQs: quittieren und ignorieren.
pub extern "x86-interrupt" fn generic_irq(_frame: TrapFrame) {
    unsafe { pic::Pic8259::eoi(1) };
}

/// Traegt alle Tore in die IDT ein.
///
/// # Safety
/// Genau einmal im Boot, vor [`idt::load`].
pub unsafe fn install() {
    unsafe {
        idt::set_handler(0, divide_error, 0, 0, false);
        idt::set_handler(1, debug_exception, 0, 0, false);
        idt::set_handler(2, nmi, gdt::IST_NMI, 0, false);
        idt::set_handler(3, breakpoint, 0, 3, true);
        idt::set_handler(4, overflow, 0, 0, false);
        idt::set_handler(5, bound_range, 0, 0, false);
        idt::set_handler(6, invalid_opcode, 0, 0, false);
        idt::set_handler(7, device_not_available, 0, 0, false);
        idt::set_handler_diverging(8, double_fault, gdt::IST_DOUBLE_FAULT, 0, false);
        idt::set_handler_err(10, invalid_tss, 0, 0, false);
        idt::set_handler_err(11, segment_not_present, 0, 0, false);
        idt::set_handler_err(12, stack_segment_fault, 0, 0, false);
        idt::set_handler_err(13, general_protection, 0, 0, false);
        idt::set_handler_err(14, page_fault, gdt::IST_PAGE_FAULT, 0, false);
        idt::set_handler(16, x87_floating_point, 0, 0, false);
        idt::set_handler_err(17, alignment_check, 0, 0, false);
        idt::set_handler(18, machine_check_like, 0, 0, false);
        idt::set_handler(19, simd_floating_point, 0, 0, false);
        idt::set_handler(20, virtualization, 0, 0, false);
        idt::set_handler_err(21, control_protection, 0, 0, false);

        idt::set_handler(VEC_TIMER, timer_irq, 0, 0, false);
        idt::set_handler(VEC_KEYBOARD, generic_irq, 0, 0, false);
        for v in (IRQ_BASE + 2)..(IRQ_BASE + 16) {
            idt::set_handler(v, generic_irq, 0, 0, false);
        }
    }
}
