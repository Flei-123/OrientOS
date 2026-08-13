//! Kontextwechsel von x86_64 — die Registerrettung, sonst nichts.
//!
//! **Die einzige Datei im Baum, die weiss, WELCHE Register beim Wechsel eines
//! Kontrollflusses zu retten sind.** Wer als naechstes laufen darf, entscheidet
//! `kcore::sched` — der kennt von hier nur [`ContextOps`].
//!
//! Verfahren: der SysV-ABI-Aufruf `switch(from, to)` rettet die *callee-saved*
//! Register (rbx, rbp, r12–r15) und die Flags auf den **eigenen** Stapel,
//! merkt sich den Stapelzeiger in `*from`, laedt den Stapelzeiger aus `*to` und
//! holt die Register des anderen Kontextes zurueck. Alle *caller-saved*
//! Register darf `switch` nach ABI ohnehin zerstoeren — der Compiler hat sie um
//! den Aufruf herum bereits gesichert. Deshalb besteht ein gesicherter Kontext
//! aus genau einem Wort: dem Stapelzeiger.
//!
//! Startstapel (von oben nach unten, `top` ist 16-ausgerichtet):
//!
//! ```text
//!   top-8   : Ruecksprungadresse fuer den Fall, dass der Rumpf doch endet
//!   top-16  : Einsprungadresse   <- 16-ausgerichtet, damit `ret` die
//!                                   ABI-Ausrichtung (rsp%16==8 am Eintritt)
//!                                   herstellt
//!   top-24  : rbp = 0            <- Kettenende fuer den Backtrace
//!   top-32  : rbx = 0
//!   top-40  : r12 = 0
//!   top-48  : r13 = 0
//!   top-56  : r14 = 0
//!   top-64  : r15 = 0
//!   top-72  : RFLAGS = 0x202     <- reserviertes Bit 1 + IF
//! ```

use crate::kcore::arch_iface::ContextOps;
use crate::kcore::mem::VirtAddr;

/// Anfangswert von RFLAGS eines neuen Kontextes: Bit 1 (immer 1) + IF.
const RFLAGS_INIT: u64 = 0x202;

/// Anzahl der Worte, die [`Context::new`] auf den Stapel legt.
const INIT_WORDS: usize = 9;

/// Gesicherter Zustand eines Kontrollflusses: sein Stapelzeiger.
///
/// Der eigentliche Registersatz liegt auf dem Stapel dieses Kontextes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    /// Gesicherter Stapelzeiger (0 = noch nie gelaufen bzw. leer).
    rsp: u64,
}

/// Landeplatz fuer einen Rumpf, der trotz `-> !` zurueckkehrt.
///
/// Wird nur erreicht, wenn ein Thread seinen Einsprung verlaesst, ohne sich
/// abzumelden — dann ist der Stapel leer und Weiterlaufen unmoeglich.
extern "C" fn context_runaway() -> ! {
    panic!("Kontextwechsel: Thread ist aus seinem Einsprung zurueckgekehrt");
}

impl Context {
    /// Legt `value` auf den absteigenden Startstapel.
    ///
    /// # Safety
    /// `sp` muss auf beschreibbaren, ausgerichteten Speicher zeigen.
    #[inline]
    unsafe fn push(sp: &mut u64, value: u64) {
        *sp -= 8;
        unsafe { core::ptr::write(*sp as *mut u64, value) };
    }
}

impl ContextOps for Context {
    const MIN_STACK_BYTES: usize = INIT_WORDS * 8 + 512;

    fn empty() -> Self {
        Context { rsp: 0 }
    }

    unsafe fn new(stack_top: VirtAddr, entry: extern "C" fn() -> !) -> Self {
        // 16 ausrichten: `ret` in `switch` erwartet den Einsprung an einer
        // 16-ausgerichteten Stelle (siehe Modulkommentar).
        let mut sp = stack_top.as_u64() & !0xf;
        unsafe {
            let runaway: extern "C" fn() -> ! = context_runaway;
            Self::push(&mut sp, runaway as usize as u64);
            Self::push(&mut sp, entry as usize as u64);
            Self::push(&mut sp, 0); // rbp — Backtrace endet hier
            Self::push(&mut sp, 0); // rbx
            Self::push(&mut sp, 0); // r12
            Self::push(&mut sp, 0); // r13
            Self::push(&mut sp, 0); // r14
            Self::push(&mut sp, 0); // r15
            Self::push(&mut sp, RFLAGS_INIT);
        }
        Context { rsp: sp }
    }

    fn stack_pointer(&self) -> VirtAddr {
        VirtAddr(self.rsp)
    }

    unsafe fn switch(from: *mut Self, to: *const Self) {
        unsafe { switch_stacks(from as *mut u64, (*to).rsp) };
    }
}

/// Der eigentliche Wechsel: rettet nach `*save` und laedt `load`.
///
/// # Safety
/// `save` muss auf ein beschreibbares Wort zeigen, `load` ein von
/// [`Context::new`] oder einem frueheren Aufruf gebauter Stapelzeiger sein.
/// Ein falscher Wert fuehrt unweigerlich in einen Triple Fault.
#[unsafe(naked)]
unsafe extern "C" fn switch_stacks(save: *mut u64, load: u64) {
    core::arch::naked_asm!(
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "pushfq",
        "mov [rdi], rsp",
        "mov rsp, rsi",
        "popfq",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "ret",
    )
}
