//! 16550-UART auf COM1 — die Debugkonsole des fruehen Boots.
//!
//! Bewusst **ohne Sperre**: Der Panic-Handler muss auch dann noch schreiben
//! koennen, wenn ein Interrupt mitten in einer Ausgabe zugeschlagen hat. Ein
//! Spinlock wuerde in genau dieser Lage verklemmen. Solange nur eine CPU laeuft,
//! ist das gefahrlos; mit SMP kommt ein Per-CPU-Puffer (siehe ROADMAP.md).

use super::port::{inb, outb};

/// Basisport von COM1.
const COM1: u16 = 0x3F8;

// Registerabstaende relativ zur Basis.
const DATA: u16 = 0;
const INT_EN: u16 = 1;
const FIFO_CTL: u16 = 2;
const LINE_CTL: u16 = 3;
const MODEM_CTL: u16 = 4;
const LINE_STATUS: u16 = 5;

/// Initialisiert COM1 auf 115200 8N1 mit aktivem FIFO.
///
/// # Safety
/// Programmiert Hardware; genau einmal im Boot.
pub unsafe fn init() {
    unsafe {
        outb(COM1 + INT_EN, 0x00); // Interrupts aus — wir pollen.
        outb(COM1 + LINE_CTL, 0x80); // DLAB an: Teiler sichtbar.
        outb(COM1 + DATA, 0x01); // Teiler 1 = 115200 Baud.
        outb(COM1 + INT_EN, 0x00);
        outb(COM1 + LINE_CTL, 0x03); // 8 Bit, keine Paritaet, 1 Stopbit.
        outb(COM1 + FIFO_CTL, 0xC7); // FIFO an, leeren, Schwelle 14 Byte.
        outb(COM1 + MODEM_CTL, 0x0B); // DTR + RTS + OUT2.
    }
}

#[inline]
fn can_send() -> bool {
    // Bit 5 = Transmitter Holding Register Empty.
    unsafe { inb(COM1 + LINE_STATUS) & 0x20 != 0 }
}

/// Einzelnes Byte senden (blockierend).
#[inline]
pub fn write_byte(b: u8) {
    let mut spins = 0u32;
    while !can_send() {
        spins += 1;
        // Notbremse: wenn keine UART da ist, darf der Kernel nicht haengen.
        if spins > 1_000_000 {
            return;
        }
        core::hint::spin_loop();
    }
    unsafe { outb(COM1 + DATA, b) }
}

/// Zeichenkette senden. `\n` wird zu `\r\n`, damit Terminals sauber umbrechen.
pub fn write_str(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            write_byte(b'\r');
        }
        write_byte(b);
    }
}
