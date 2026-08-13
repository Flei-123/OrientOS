//! Architekturimplementierung **x86_64**.
//!
//! Alles unterhalb dieses Moduls darf Register, Instruktionen und Portadressen
//! kennen. Alles ausserhalb nicht.

pub mod context;
pub mod cpu;
pub mod gdt;
pub mod idt;
pub mod interrupts;
pub mod msr;
pub mod paging;
pub mod pic;
pub mod port;
pub mod preempt;
pub mod selftest;
pub mod serial;
pub mod timer;
pub mod user;

use crate::kcore::arch_iface::{ArchOps, CpuFeatures, InterruptCtlOps, UserExit};
use crate::kcore::mem::VirtAddr;

/// Markertyp der Architektur.
pub struct X86_64;

impl ArchOps for X86_64 {
    const NAME: &'static str = "x86_64";
    const PAGE_SIZE: usize = 4096;

    type AddressSpace = paging::X86AddressSpace;
    type Context = context::Context;
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

    fn fault_stack_top() -> Option<VirtAddr> {
        Some(VirtAddr(gdt::double_fault_stack_top()))
    }

    // Die folgenden Methoden sind bewusst reine Weiterleitungen: die Arbeit
    // steht in `preempt.rs` bzw. `user.rs`, damit diese Datei die Uebersicht
    // ueber die Architektur bleibt und zwei Baustellen sich nicht ins Gehege
    // kommen.

    unsafe fn set_preemption(on: bool) -> bool {
        unsafe { preempt::set_enabled(on) }
    }

    fn preemption_available() -> bool {
        preempt::available()
    }

    fn cpu_features() -> CpuFeatures {
        user::features()
    }

    unsafe fn init_user_support() -> bool {
        unsafe { user::init() }
    }

    fn user_support_ready() -> bool {
        user::ready()
    }

    fn set_kernel_stack(top: VirtAddr) {
        user::set_kernel_stack(top)
    }

    unsafe fn enter_user(entry: VirtAddr, stack_top: VirtAddr) -> UserExit {
        unsafe { user::enter(entry, stack_top) }
    }

    fn code_selector() -> u16 {
        user::code_selector()
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
