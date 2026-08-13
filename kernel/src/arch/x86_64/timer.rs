//! Systemzeitgeber auf Basis des 8253/8254-PIT.
//!
//! Der PIT ist auf jeder x86-Maschine vorhanden und braucht keine Kalibrierung —
//! ideal fuer den ersten nachweisbaren periodischen Interrupt. Der spaetere
//! Umstieg auf LAPIC-Timer/TSC-Deadline (siehe ROADMAP.md) tauscht nur diese
//! Datei aus.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::pic::Pic8259;
use super::port::outb;
use crate::kcore::arch_iface::{InterruptCtlOps, TimerOps};

/// Eingangsfrequenz des PIT in Hertz.
const PIT_BASE_HZ: u32 = 1_193_182;
const PIT_CH0: u16 = 0x40;
const PIT_CMD: u16 = 0x43;

static TICKS: AtomicU64 = AtomicU64::new(0);
static HZ: AtomicU32 = AtomicU32::new(0);

/// Der PIT als Systemzeitgeber.
pub struct Pit;

impl TimerOps for Pit {
    unsafe fn start(hz: u32) {
        let hz = hz.clamp(19, 1000);
        let divisor = (PIT_BASE_HZ / hz) as u16;
        HZ.store(PIT_BASE_HZ / divisor as u32, Ordering::Relaxed);
        unsafe {
            // Kanal 0, Zugriff lo+hi, Modus 3 (Rechteck), binaer.
            outb(PIT_CMD, 0x36);
            outb(PIT_CH0, divisor as u8);
            outb(PIT_CH0, (divisor >> 8) as u8);
            // IRQ0 freigeben.
            Pic8259::set_masked(0, false);
        }
    }

    fn ticks() -> u64 {
        TICKS.load(Ordering::Relaxed)
    }

    fn hz() -> u32 {
        HZ.load(Ordering::Relaxed)
    }
}

/// Wird vom IRQ0-Handler aufgerufen.
#[inline]
pub fn on_tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// Millisekunden seit Timerstart (0, solange der Timer nicht laeuft).
pub fn uptime_ms() -> u64 {
    let hz = HZ.load(Ordering::Relaxed) as u64;
    if hz == 0 {
        0
    } else {
        TICKS.load(Ordering::Relaxed) * 1000 / hz
    }
}
