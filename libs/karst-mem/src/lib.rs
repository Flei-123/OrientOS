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

/// Rundet `addr` auf das naechste Vielfache von `align` auf und meldet einen
/// Ueberlauf, statt still auf 0 zu springen.
///
/// Noetig, weil eine defekte Firmware-Memory-Map Adressen dicht unter
/// `u64::MAX` melden kann: `align_up` wuerde dann 0 liefern und eine Region
/// scheinbar an den Anfang des Adressraums legen. Alle Stellen, die mit
/// fremden Zahlen rechnen (siehe [`region::MemoryRegion::page_aligned`]),
/// benutzen diese Variante.
#[inline]
pub const fn align_up_checked(addr: u64, align: u64) -> Option<u64> {
    match addr.checked_add(align - 1) {
        Some(v) => Some(v & !(align - 1)),
        None => None,
    }
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
    fn align_up_checked_reports_overflow_instead_of_wrapping() {
        // Der Regelfall verhaelt sich wie `align_up`.
        for (v, a) in [(0u64, 4096u64), (1, 4096), (4096, 4096), (4097, 4096), (7, 8)] {
            assert_eq!(align_up_checked(v, a), Some(align_up(v, a)), "v={v} a={a}");
        }
        // Der Ueberlaufbereich: `align_up` wuerde hier umlaufen (im Debugbau
        // sogar in Panik geraten), die gepruefte Variante meldet None.
        for v in [u64::MAX, u64::MAX - 1, u64::MAX - 4094] {
            assert_eq!(align_up_checked(v, PAGE_SIZE), None, "v={v:#x}");
            assert_eq!(v.wrapping_add(PAGE_SIZE - 1) & !(PAGE_SIZE - 1), 0);
        }
        // Genau die letzte noch darstellbare Seitengrenze geht noch.
        let last = u64::MAX - 4095;
        assert_eq!(align_up_checked(last, PAGE_SIZE), Some(last));
        assert_eq!(align_up_checked(last - 1, PAGE_SIZE), Some(last));
        // Ausrichtung 1 kann nie ueberlaufen.
        assert_eq!(align_up_checked(u64::MAX, 1), Some(u64::MAX));
    }

    /// Eigenschaftstest: die gepruefte Variante stimmt ueberall dort mit
    /// `align_up` ueberein, wo diese nicht umlaeuft — und nur dort.
    #[test]
    fn align_up_checked_agrees_with_align_up_where_defined() {
        let mut r = Lcg::new(0xc0de);
        for _ in 0..20_000 {
            let align = 1u64 << r.range(0, 21) as u32;
            let v = r.next_u64();
            match align_up_checked(v, align) {
                Some(up) => {
                    assert_eq!(up % align, 0);
                    assert!(up >= v);
                    assert!(up - v < align);
                    assert_eq!(up, align_up(v, align));
                }
                None => assert!(v > u64::MAX - (align - 1), "None trotz Spielraum"),
            }
        }
    }

    #[test]
    fn page_size_is_a_power_of_two() {
        assert!(PAGE_SIZE.is_power_of_two());
        assert_eq!(PAGE_SIZE, 4096);
    }

    // --------------------------------------------------------------------
    // Zusammenspiel Memory-Map -> Frame-Bitmap.
    //
    // Die beiden Bausteine werden im Kernel IMMER zusammen benutzt: aus der
    // Regionsliste entsteht die Bitmap. Genau dieser Uebergang ist der Ort,
    // an dem eine zerrissene, unsortierte oder ueberlappende Map gefaehrlich
    // wird — hier wird er ohne Hardware durchgespielt.
    // --------------------------------------------------------------------

    use crate::bitmap::Bitmap;
    use crate::region::{frames_for, summarize, MemoryRegion, RegionKind};

    /// Baut die Bitmap so auf, wie es der Kernel tut: erst nutzbare Regionen
    /// freigeben, danach alles Nicht-Nutzbare konservativ (nach aussen
    /// gerundet) wieder reservieren.
    fn bitmap_from_map<'a>(
        regions: &[MemoryRegion],
        bits: &'a mut [u64],
        frames: usize,
    ) -> Bitmap<'a> {
        let mut b = Bitmap::new_all_used(bits, frames);
        for r in regions.iter().filter(|r| r.kind.is_allocatable()) {
            if let Some((s, e)) = r.page_aligned() {
                b.free_range((s / PAGE_SIZE) as usize, (e / PAGE_SIZE) as usize);
            }
        }
        for r in regions.iter().filter(|r| !r.kind.is_allocatable() && r.len > 0) {
            let s = align_down(r.start, PAGE_SIZE) / PAGE_SIZE;
            let e = align_up(r.end(), PAGE_SIZE) / PAGE_SIZE;
            b.reserve_range(s as usize, e as usize);
        }
        b
    }

    #[test]
    fn bitmap_from_map_only_hands_out_usable_frames() {
        let regions = [
            MemoryRegion::new(0, 0x1000, RegionKind::Reserved),
            MemoryRegion::new(0x1000, 0x9f000, RegionKind::Usable),
            MemoryRegion::new(0xa0000, 0x60000, RegionKind::Reserved), // Loch bei 640 KiB
            MemoryRegion::new(0x10_0000, 0x40_0000, RegionKind::Usable),
            MemoryRegion::new(0x50_0000, 0x10_0000, RegionKind::KernelAndModules),
            MemoryRegion::new(0x60_0000, 0x20_0000, RegionKind::BootloaderReclaimable),
        ];
        let s = summarize(&regions);
        let frames = frames_for(s.ram_top);
        assert_eq!(frames as u64, 0x80_0000 / PAGE_SIZE);
        let mut bits = std::vec![0u64; Bitmap::words_needed(frames)];
        let mut b = bitmap_from_map(&regions, &mut bits, frames);

        // Frei sind genau die nutzbaren Regionen.
        assert_eq!(b.free_frames() as u64, (0x9f000 + 0x40_0000) / PAGE_SIZE);

        // Alles abraeumen und jeden Frame gegen die Map pruefen.
        let mut n = 0u64;
        while let Ok(f) = b.alloc() {
            let addr = f as u64 * PAGE_SIZE;
            let ok = regions
                .iter()
                .any(|r| r.kind.is_allocatable() && addr >= r.start && addr + PAGE_SIZE <= r.end());
            assert!(ok, "Frame {f} ({addr:#x}) stammt nicht aus nutzbarem RAM");
            n += 1;
        }
        assert_eq!(n, (0x9f000 + 0x40_0000) / PAGE_SIZE);
        assert_eq!(b.free_frames(), 0);
    }

    #[test]
    fn overlapping_and_unsorted_map_never_yields_reserved_frames() {
        // Firmware liefert die Regionen unsortiert, und eine Reserved-Region
        // ueberlappt eine als nutzbar gemeldete. Der Allocator muss die
        // Ueberschneidung dem Reservierten zuschlagen.
        let regions = [
            MemoryRegion::new(0x8000, 0x2000, RegionKind::Reserved),
            MemoryRegion::new(0x0, 0x1_0000, RegionKind::Usable),
            MemoryRegion::new(0x1_0000, 0x1000, RegionKind::BadMemory),
        ];
        let frames = frames_for(summarize(&regions).ram_top);
        let mut bits = std::vec![0u64; Bitmap::words_needed(frames)];
        let mut b = bitmap_from_map(&regions, &mut bits, frames);
        assert_eq!(b.free_frames() as u64, (0x1_0000 - 0x2000) / PAGE_SIZE);
        while let Ok(f) = b.alloc() {
            let addr = f as u64 * PAGE_SIZE;
            assert!(
                !(0x8000..0xa000).contains(&addr),
                "reservierter Frame {addr:#x} wurde vergeben"
            );
            assert!(addr < 0x1_0000, "Frame {addr:#x} liegt ausserhalb des RAM");
        }
    }

    #[test]
    fn torn_map_with_unaligned_edges_is_trimmed_inward() {
        // Nicht seitenausgerichtete Raender: nutzbar wird nach INNEN gerundet,
        // reserviert nach AUSSEN. Ein halber Frame darf nie vergeben werden.
        let regions = [
            MemoryRegion::new(0x1001, 0x2fff, RegionKind::Usable), // nutzbar: 0x2000..0x4000
            MemoryRegion::new(0x4000, 0x1000, RegionKind::Usable),
            MemoryRegion::new(0x5800, 0x800, RegionKind::Reserved), // frisst Frame 5
        ];
        let frames = frames_for(summarize(&regions).ram_top);
        let mut bits = std::vec![0u64; Bitmap::words_needed(frames)];
        let b = bitmap_from_map(&regions, &mut bits, frames);
        assert!(b.is_used(0) && b.is_used(1), "angeschnittener Frame vergeben");
        assert!(!b.is_used(2) && !b.is_used(3) && !b.is_used(4));
        assert!(b.is_used(5), "teilweise reservierter Frame muss belegt bleiben");
        assert_eq!(b.free_frames(), 3);
    }

    /// Eigenschaftstest: fuer zufaellig zerrissene Maps darf der Allocator nie
    /// einen Frame ausgeben, der nicht vollstaendig in einer nutzbaren Region
    /// liegt — und nie mehr Frames, als die Map hergibt.
    #[test]
    fn random_torn_maps_never_leak_frames_outside_usable_memory() {
        let mut r = Lcg::new(0xf00d);
        for _ in 0..200 {
            let mut regions = std::vec::Vec::new();
            let mut cursor = 0u64;
            for _ in 0..r.range(1, 12) {
                cursor += (r.below(4) as u64) * PAGE_SIZE + r.below(4096) as u64;
                let len = (r.below(20) as u64) * PAGE_SIZE + r.below(4096) as u64;
                let kind = match r.below(4) {
                    0 => RegionKind::Reserved,
                    1 => RegionKind::BootloaderReclaimable,
                    _ => RegionKind::Usable,
                };
                regions.push(MemoryRegion::new(cursor, len, kind));
                cursor += len;
            }
            let frames = frames_for(summarize(&regions).ram_top);
            if frames == 0 {
                continue;
            }
            let mut bits = std::vec![0u64; Bitmap::words_needed(frames)];
            let mut b = bitmap_from_map(&regions, &mut bits, frames);
            let frei = b.free_frames();
            let mut n = 0;
            while let Ok(f) = b.alloc() {
                let addr = f as u64 * PAGE_SIZE;
                assert!(f < frames, "Frame {f} ausserhalb der Bitmap");
                let in_usable = regions.iter().any(|reg| {
                    reg.kind.is_allocatable() && addr >= reg.start && addr + PAGE_SIZE <= reg.end()
                });
                let in_other = regions.iter().any(|reg| {
                    !reg.kind.is_allocatable()
                        && reg.len > 0
                        && addr < reg.end()
                        && addr + PAGE_SIZE > reg.start
                });
                assert!(in_usable, "Frame {addr:#x} liegt nicht in nutzbarem RAM");
                assert!(!in_other, "Frame {addr:#x} ueberlappt eine belegte Region");
                n += 1;
            }
            assert_eq!(n, frei, "Zaehler und tatsaechliche Ausgabe weichen ab");
            assert_eq!(b.free_frames(), 0);
        }
    }
}
