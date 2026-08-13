//! Erzeugung echter CPU-Ausnahmen fuer die Selbsttests.

use super::{cpu, idt};

/// Kanonische, garantiert nicht abgebildete Kerneladresse.
///
/// Wichtig: die Adresse MUSS kanonisch sein (Bits 63..48 gleich Bit 47).
/// Eine nicht kanonische Adresse wie `0xdead_beef_0000` erzeugt naemlich `#GP`
/// statt `#PF` — und der Selbsttest wuerde die falsche Ausnahme pruefen.
/// Dieser Wert liegt im PML4-Eintrag 510, aber weit hinter Heap und Testseite.
const UNMAPPED: u64 = 0xffff_ff40_0000_0000;

/// `int3` — Breakpoint. Das Tor ist ein Trap-Tor, der Handler kehrt zurueck.
pub fn breakpoint() {
    unsafe { core::arch::asm!("int3", options(nomem, nostack)) };
}

/// Schreibzugriff auf eine kanonische, aber nicht abgebildete Adresse.
///
/// # Safety
/// Loest einen Page Fault aus.
pub unsafe fn page_fault() -> ! {
    unsafe { core::ptr::write_volatile(UNMAPPED as *mut u64, 42) };
    cpu::halt_forever()
}

/// Konstante, die der Linker nach `.rodata` legt. `#[used]` verhindert, dass
/// sie wegoptimiert wird, die Sektionsangabe erzwingt die Lage — nur so prueft
/// der Test wirklich die Rechte des Kernelabbilds und nicht die von `.data`.
#[used]
#[unsafe(link_section = ".rodata.selftest")]
static RODATA_PROBE: u64 = 0x5241_444F_5441_4B4F;

/// Schreibversuch auf `.rodata`. Weil der Kernel seine eigenen Tabellen mit
/// R-ohne-W fuer `.rodata` aufgebaut hat UND CR0.WP gesetzt ist, muss das auch
/// in Ring 0 einen `#PF` mit gesetztem WRITE-Bit ausloesen. Kehrt der Zugriff
/// zurueck, sind die Rechte NICHT wirksam — dann meldet der Test genau das.
///
/// # Safety
/// Loest im Erfolgsfall einen Page Fault aus; der Kernel haelt danach an.
pub unsafe fn rodata_write() -> ! {
    let p = &raw const RODATA_PROBE as *mut u64;
    unsafe { core::ptr::write_volatile(p, 0) };
    crate::serial_println!(
        "[karst] test       FEHLER: Schreiben auf .rodata bei {:p} war erlaubt — Rechte wirkungslos",
        p
    );
    cpu::halt_forever()
}

/// Puffer im beschreibbaren Datenbereich, der eine gueltige Instruktionsfolge
/// enthaelt (`0xC3` = `ret`). Der Linker legt ihn wegen des Initialwerts nach
/// `.data`; diese Sektion bildet der Kernel mit gesetztem NX-Bit ab.
#[used]
#[unsafe(link_section = ".data.selftest")]
static NX_PROBE: [u8; 8] = [0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3];

/// Sprung in eine Seite ohne Ausfuehrungsrecht (`.data`, NX gesetzt).
///
/// Beweist, dass das NX-Bit der eigenen Seitentabellen wirklich in Hardware
/// wirkt: die CPU muss einen `#PF` mit gesetztem Bit 4 des Fehlercodes
/// (Instruktionsabruf) melden. Kehrt der Aufruf zurueck, ist NX wirkungslos —
/// dann meldet der Test genau das im Klartext.
///
/// # Safety
/// Loest im Erfolgsfall einen Page Fault aus; der Kernel haelt danach an.
pub unsafe fn nx_exec() -> ! {
    let p = &raw const NX_PROBE as *const u8;
    let f: extern "C" fn() = unsafe { core::mem::transmute(p) };
    f();
    crate::serial_println!(
        "[karst] test       FEHLER: Ausfuehren von .data bei {:p} war erlaubt — NX wirkungslos",
        p
    );
    cpu::halt_forever()
}

/// Erzeugt einen ECHTEN Double Fault — keine Simulation per `int 8`.
///
/// Vorgehen: Das Tor fuer `#PF` wird als "nicht praesent" markiert. Ein
/// anschliessender Page Fault kann dann nicht zugestellt werden; die dabei
/// entstehende `#NP` waehrend der Zustellung einer Ausnahme ist laut Intel SDM
/// Vol.3 Tab. 6-5 genau die Bedingung fuer `#DF`. Der Double-Fault-Handler
/// laeuft auf seinem eigenen IST-Stapel und kann deshalb noch berichten.
///
/// # Safety
/// Die CPU wird danach angehalten.
pub unsafe fn double_fault() -> ! {
    unsafe {
        idt::clear_gate(14);
        core::ptr::write_volatile(UNMAPPED as *mut u64, 42);
    }
    cpu::halt_forever()
}
