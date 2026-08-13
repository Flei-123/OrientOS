//! Panic-Handling der core-Schicht.
//!
//! **Schicht: core.** Enthaelt keine Architekturdetails: der Backtrace wird
//! ueber [`crate::arch::backtrace`] geholt (Frame-Pointer-Unwinding, in `arch`
//! implementiert, weil der Frame-Pointer ein ABI-/ISA-Detail ist), das Anhalten
//! der CPU ueber [`crate::arch::halt_forever`].

use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

static PANICKING: AtomicBool = AtomicBool::new(false);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Interrupts aus: ein Timer-Interrupt mitten im Panic-Druck wuerde die
    // Ausgabe zerreissen oder erneut panicken.
    let irq_war_an = crate::arch::interrupts_enabled();
    crate::arch::disable_interrupts();

    if PANICKING.swap(true, Ordering::SeqCst) {
        crate::serial_println!("\n{} DOPPEL-PANIC — halte an.", crate::kcore::branding::LOG_TAG);
        crate::arch::halt_forever();
    }

    crate::serial_println!("\n=============== KERNEL PANIC ===============");
    if let Some(loc) = info.location() {
        crate::serial_println!("Ort     : {}:{}:{}", loc.file(), loc.line(), loc.column());
    }
    crate::serial_println!("Grund   : {}", info.message());
    // Laufzeit aus dem Zeitgeber: rein atomar gelesen, deshalb auch dann
    // gefahrlos, wenn der Panic mitten in einer gesperrten Datenstruktur
    // auftritt (kein Lock, kein Deadlock im Panic-Pfad).
    let hz = <crate::arch::Timer as crate::kcore::arch_iface::TimerOps>::hz() as u64;
    let ticks = <crate::arch::Timer as crate::kcore::arch_iface::TimerOps>::ticks();
    if hz > 0 {
        crate::serial_println!("Laufzeit: {} Ticks bei {} Hz ({} s)", ticks, hz, ticks / hz);
    } else {
        crate::serial_println!("Laufzeit: Zeitgeber laeuft noch nicht ({} Ticks)", ticks);
    }
    crate::serial_println!(
        "Zustand : Interrupts beim Eintritt {}, jetzt abgeschaltet",
        if irq_war_an { "an" } else { "aus" }
    );
    let frames = backtrace();
    crate::serial_println!("Backtrace: {} Frame(s) aufgeloest", frames);
    crate::serial_println!("============================================");

    crate::arch::halt_forever();
}

/// Gibt einen Backtrace ueber Frame-Pointer-Unwinding aus und liefert die Zahl
/// der aufgeloesten Frames zurueck (0 = keine verwertbare Kette).
///
/// Wir bauen den Kernel mit `-C force-frame-pointers=yes` (siehe `build.sh`),
/// dadurch bildet RBP eine verkettete Liste `[rbp] = voriger rbp`,
/// `[rbp+8] = Ruecksprungadresse`. Die Adressen sind gegen die Sektionsadressen
/// aus `build/karst.map` bzw. via `llvm-addr2line` aufloesbar — siehe README,
/// Abschnitt "Backtrace lesen".
pub fn backtrace() -> usize {
    crate::serial_println!("Backtrace (Frame-Pointer, Adressen via addr2line aufloesbar):");
    let mut n = 0usize;
    crate::arch::backtrace(&mut |frame_ip: u64| {
        crate::serial_println!("  #{:<2} ip={:#018x}", n, frame_ip);
        n += 1;
    });
    if n == 0 {
        crate::serial_println!("  <keine Frame-Pointer-Kette verfuegbar>");
    }
    n
}
