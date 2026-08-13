//! Port-IO. **Nur hier** stehen `in`/`out`-Instruktionen.

use core::arch::asm;

/// Byte auf einen IO-Port schreiben.
///
/// # Safety
/// Portzugriffe wirken direkt auf Hardware.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

/// Byte von einem IO-Port lesen.
///
/// # Safety
/// Siehe [`outb`].
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}

/// 32-Bit-Wort auf einen IO-Port schreiben (fuer `isa-debug-exit`).
///
/// # Safety
/// Siehe [`outb`].
#[inline]
pub unsafe fn outl(port: u16, value: u32) {
    unsafe {
        asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags));
    }
}

/// Kurze IO-Verzoegerung fuer betagte Controller (8259, 8253).
///
/// # Safety
/// Schreibt auf den unbenutzten Diagnoseport 0x80.
#[inline]
pub unsafe fn io_wait() {
    unsafe { outb(0x80, 0) }
}
