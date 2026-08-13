//! Architekturimplementierung **x86_64**.
//!
//! Alles unterhalb dieses Moduls darf Register, Instruktionen und Portadressen
//! kennen. Alles ausserhalb nicht.

pub mod cpu;
pub mod gdt;
pub mod idt;
pub mod interrupts;
pub mod msr;
pub mod paging;
pub mod pic;
pub mod port;
pub mod selftest;
pub mod serial;
pub mod timer;

use crate::kcore::arch_iface::{ArchOps, InterruptCtlOps};

/// Markertyp der Architektur.
pub struct X86_64;

impl ArchOps for X86_64 {
    const NAME: &'static str = "x86_64";
    const PAGE_SIZE: usize = 4096;

    type AddressSpace = paging::X86AddressSpace;
    type Timer = timer::Pit;
    type IntCtl = pic::Pic8259;

    unsafe fn init_serial() {
        unsafe { serial::init() }
    }

    fn serial_write(s: &str) {
        serial::write_str(s)
    }

    unsafe fn init_cpu() {
        unsafe {
            cpu::cli();
            // NX freischalten, BEVOR eigene Tabellen mit Bit 63 entstehen.
            msr::enable_nx();
            // CR0.WP: auch Ring 0 respektiert schreibgeschuetzte Seiten.
            msr::enable_write_protect();
            gdt::init();
            interrupts::install();
            idt::load();
            <pic::Pic8259 as InterruptCtlOps>::init();
        }
    }

    fn disable_interrupts() {
        cpu::cli()
    }

    fn enable_interrupts() {
        cpu::sti()
    }

    fn interrupts_enabled() -> bool {
        cpu::interrupts_enabled()
    }

    fn wait_for_interrupt() {
        cpu::hlt()
    }

    fn halt_forever() -> ! {
        cpu::halt_forever()
    }

    fn backtrace(f: &mut dyn FnMut(u64)) {
        cpu::backtrace(f)
    }

    fn cpu_brand(buf: &mut [u8]) -> usize {
        cpu::cpu_brand(buf)
    }
}

/// Beendet QEMU mit einem Statuscode (`-device isa-debug-exit,iobase=0xf4`).
/// Nur fuer automatisierte Testlaeufe; auf echter Hardware passiert nichts.
///
/// # Safety
/// Schreibt auf einen IO-Port.
pub unsafe fn qemu_exit(code: u32) -> ! {
    unsafe { port::outl(0xf4, code) };
    cpu::halt_forever()
}
