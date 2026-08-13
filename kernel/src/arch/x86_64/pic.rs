//! 8259A-PIC-Paar.
//!
//! Warum PIC und nicht gleich APIC: Der PIC braucht keine ACPI-Tabellen, keine
//! MMIO-Abbildung und keinen Timer-Kalibrierlauf — er funktioniert in der Sekunde,
//! in der der Kernel laeuft, und liefert damit die frueheste moegliche
//! Interrupt-Nachweisbarkeit. Der Umstieg auf LAPIC/IOAPIC steht in ROADMAP.md
//! (Phase SMP) und ersetzt nur diese eine Datei — [`crate::kcore::arch_iface::InterruptCtlOps`]
//! bleibt gleich.

use super::interrupts::IRQ_BASE;
use super::port::{inb, io_wait, outb};
use crate::kcore::arch_iface::InterruptCtlOps;

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x11; // Initialisierung + ICW4 folgt
const ICW4_8086: u8 = 0x01;
const EOI: u8 = 0x20;

/// Der klassische Interruptcontroller.
pub struct Pic8259;

impl InterruptCtlOps for Pic8259 {
    unsafe fn init() {
        unsafe {
            // Vorhandene Masken sichern ist unnoetig: wir maskieren am Ende alles.
            outb(PIC1_CMD, ICW1_INIT);
            io_wait();
            outb(PIC2_CMD, ICW1_INIT);
            io_wait();
            // ICW2: Vektorbasis. Ohne Umleitung kollidierte IRQ0 mit #DE.
            outb(PIC1_DATA, IRQ_BASE);
            io_wait();
            outb(PIC2_DATA, IRQ_BASE + 8);
            io_wait();
            // ICW3: Verkettung ueber IRQ2.
            outb(PIC1_DATA, 4);
            io_wait();
            outb(PIC2_DATA, 2);
            io_wait();
            // ICW4: 8086-Modus.
            outb(PIC1_DATA, ICW4_8086);
            io_wait();
            outb(PIC2_DATA, ICW4_8086);
            io_wait();
            // Alles maskieren — Treiber demaskieren gezielt.
            outb(PIC1_DATA, 0xFF);
            outb(PIC2_DATA, 0xFF);
        }
    }

    unsafe fn eoi(irq: u8) {
        unsafe {
            if irq >= 8 {
                outb(PIC2_CMD, EOI);
            }
            outb(PIC1_CMD, EOI);
        }
    }

    unsafe fn set_masked(irq: u8, masked: bool) {
        let (port, bit) = if irq < 8 { (PIC1_DATA, irq) } else { (PIC2_DATA, irq - 8) };
        unsafe {
            let mut mask = inb(port);
            if masked {
                mask |= 1 << bit;
            } else {
                mask &= !(1 << bit);
            }
            outb(port, mask);
        }
    }

    fn name() -> &'static str {
        "8259A-PIC (umgeleitet auf 0x20)"
    }
}
