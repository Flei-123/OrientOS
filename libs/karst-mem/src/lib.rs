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

    #[test]
    fn align_works() {
        assert_eq!(align_up(1, 4096), 4096);
        assert_eq!(align_up(4096, 4096), 4096);
        assert_eq!(align_down(4097, 4096), 4096);
    }
}
