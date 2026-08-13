//! `karst-mem` — architekturunabhaengige Speicherlogik.
//!
//! Diese Crate enthaelt AUSSCHLIESSLICH reine Logik ohne Hardwarezugriff, damit sie
//! auf dem Host mit `cargo test -p karst-mem --target x86_64-unknown-linux-gnu`
//! getestet werden kann. Kein `asm!`, keine Ports, keine Register.
#![no_std]

#[cfg(test)]
extern crate std;

pub mod bitmap;
pub mod heap;
pub mod region;

/// Deterministischer Zufallsgenerator fuer die Eigenschaftstests. Existiert nur
/// im Testbau (`#[cfg(test)]`) — im Kernel wird davon kein Byte uebersetzt.
#[cfg(test)]
pub mod testrand;

/// Groesse eines physischen Frames in Bytes (x86_64 4-KiB-Basisseite).
/// Wird beim Portieren pro Architektur ueber `KernelPaging::PAGE_SIZE` gespiegelt.
pub const PAGE_SIZE: u64 = 4096;

/// Rundet `addr` auf das naechste Vielfache von `align` auf. `align` muss 2^n sein.
#[inline]
pub const fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

/// Rundet `addr` auf das naechste Vielfache von `align` ab. `align` muss 2^n sein.
#[inline]
pub const fn align_down(addr: u64, align: u64) -> u64 {
    addr & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testrand::Lcg;

    #[test]
    fn align_works() {
        assert_eq!(align_up(1, 4096), 4096);
        assert_eq!(align_up(4096, 4096), 4096);
        assert_eq!(align_down(4097, 4096), 4096);
    }

    #[test]
    fn align_edge_cases() {
        // Ausrichtung 1 laesst jeden Wert unveraendert.
        for v in [0u64, 1, 4095, u64::MAX / 2] {
            assert_eq!(align_up(v, 1), v);
            assert_eq!(align_down(v, 1), v);
        }
        // Null bleibt Null.
        assert_eq!(align_up(0, 4096), 0);
        assert_eq!(align_down(0, 4096), 0);
        // Genau eine Grenze darunter/darueber.
        assert_eq!(align_up(4095, 4096), 4096);
        assert_eq!(align_up(4097, 4096), 8192);
        assert_eq!(align_down(4096, 4096), 4096);
        assert_eq!(align_down(4095, 4096), 0);
        // Grosse Ausrichtung (1 GiB, wie eine Huge-Page der obersten Stufe).
        const GIB: u64 = 1 << 30;
        assert_eq!(align_up(GIB + 1, GIB), 2 * GIB);
        assert_eq!(align_down(2 * GIB - 1, GIB), GIB);
    }

    #[test]
    fn align_is_idempotent() {
        for align in [1u64, 2, 8, 4096, 2 << 20] {
            for v in [0u64, 1, 7, 4095, 123_456_789] {
                let up = align_up(v, align);
                let down = align_down(v, align);
                assert_eq!(align_up(up, align), up, "align_up nicht idempotent");
                assert_eq!(align_down(down, align), down, "align_down nicht idempotent");
            }
        }
    }

    /// Eigenschaftstest: fuer zufaellige Werte muss immer
    /// `down <= v <= up`, beide ausgerichtet, und `up - v < align` gelten.
    #[test]
    fn align_properties_hold_for_random_values() {
        let mut r = Lcg::new(0x4c0f_fee);
        for _ in 0..20_000 {
            let shift = r.range(0, 21) as u32; // Ausrichtung 1 .. 2 MiB
            let align = 1u64 << shift;
            let v = r.next_u64() >> 8; // Ueberlauf in align_up vermeiden
            let up = align_up(v, align);
            let down = align_down(v, align);
            assert!(down <= v && v <= up, "Reihenfolge verletzt bei v={v} align={align}");
            assert_eq!(up % align, 0);
            assert_eq!(down % align, 0);
            assert!(up - v < align, "align_up rundet zu weit: {v} -> {up}");
            assert!(v - down < align, "align_down rundet zu weit: {v} -> {down}");
            if v % align == 0 {
                assert_eq!(up, v);
                assert_eq!(down, v);
            } else {
                assert_eq!(up, down + align);
            }
        }
    }

    #[test]
    fn page_size_is_a_power_of_two() {
        assert!(PAGE_SIZE.is_power_of_two());
        assert_eq!(PAGE_SIZE, 4096);
    }
}
