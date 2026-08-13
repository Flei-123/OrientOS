//! IDT — Interrupt Descriptor Table.
//!
//! 256 Torbeschreiber. Die Handler selbst stehen in [`super::interrupts`];
//! diese Datei kennt nur das Bitformat der Tore.

use core::arch::asm;
use core::mem::size_of;
use core::ptr::addr_of_mut;

/// Ein 16-Byte-Interrupt-Tor im long mode.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Gate {
    offset_low: u16,
    selector: u16,
    /// Bits 0..2: IST-Index (0 = aktueller Stack).
    ist: u8,
    /// Present | DPL | Typ (0xE = Interrupt-Tor, 0xF = Trap-Tor).
    flags: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl Gate {
    const fn empty() -> Self {
        Gate {
            offset_low: 0,
            selector: 0,
            ist: 0,
            flags: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    fn set(&mut self, handler: u64, selector: u16, ist: u8, dpl: u8, trap: bool) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.selector = selector;
        self.ist = ist & 0x7;
        let ty: u8 = if trap { 0xF } else { 0xE };
        self.flags = 0x80 | ((dpl & 0x3) << 5) | ty;
        self.reserved = 0;
    }
}

static mut IDT: [Gate; 256] = [Gate::empty(); 256];

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

/// Traegt ein Tor ein.
///
/// # Safety
/// `handler` muss auf eine Funktion mit `extern "x86-interrupt"` zeigen.
pub unsafe fn set_gate(vector: u8, handler: u64, ist: u8, dpl: u8, trap: bool) {
    unsafe {
        let idt = addr_of_mut!(IDT);
        (*idt)[vector as usize].set(handler, super::gdt::KERNEL_CODE, ist, dpl, trap);
    }
}

/// Laedt die IDT in die CPU.
///
/// # Safety
/// Alle benutzten Tore muessen vorher gesetzt sein.
pub unsafe fn load() {
    unsafe {
        let idt = addr_of_mut!(IDT);
        let dtr = DescriptorTablePointer {
            limit: (size_of::<[Gate; 256]>() - 1) as u16,
            base: idt as u64,
        };
        asm!("lidt [{}]", in(reg) &dtr, options(readonly, nostack, preserves_flags));
    }
}

/// Markiert ein Tor als NICHT praesent. Wird vom Double-Fault-Selbsttest
/// benutzt, um eine Ausnahme waehrend der Ausnahmezustellung zu erzwingen.
///
/// # Safety
/// Danach fuehrt die zugehoerige Ausnahme unweigerlich zum Double Fault.
pub unsafe fn clear_gate(vector: u8) {
    unsafe {
        let idt = addr_of_mut!(IDT);
        (*idt)[vector as usize] = Gate::empty();
    }
}
