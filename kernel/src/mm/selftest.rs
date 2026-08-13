//! Selbsttests der Adressraum-Abstraktion — **im laufenden Kernel**, nicht auf
//! dem Host.
//!
//! **Schicht: mm.** Hier wird nachgewiesen, dass die Fehlerfaelle von
//! [`MapError`] nicht nur definiert, sondern auch wirklich erreichbar sind.
//! Der Test benutzt ausschliesslich [`AddressSpaceOps`] — kein x86-Detail.
//!
//! Jeder Fall arbeitet auf einer eigenen, sonst unbenutzten virtuellen Adresse
//! und raeumt hinter sich auf, damit der Boot danach unveraendert weiterlaeuft.

use crate::klog;
use crate::arch::AddressSpace;
use crate::kcore::arch_iface::AddressSpaceOps;
use crate::kcore::mem::{FrameAllocator, MapError, MapFlags, PhysAddr, VirtAddr, PAGE_SIZE};

/// Basisadresse der Testseiten. Eigener, sonst unbenutzter PML4-Eintrag (509),
/// damit weder Heap (510), Kernelabbild (511) noch HHDM (256) beruehrt werden.
pub const PROBE_BASE: u64 = 0xffff_fe80_0000_0000;

/// Frame-Quelle, die grundsaetzlich nichts liefert. Damit ist
/// [`MapError::OutOfFrames`] ohne echten Speichermangel reproduzierbar.
struct StarvedAlloc;

impl FrameAllocator for StarvedAlloc {
    fn alloc_frame(&mut self) -> Option<PhysAddr> {
        None
    }
    fn free_frame(&mut self, _frame: PhysAddr) {}
}

/// Ergebnis eines Einzelfalls.
#[derive(Clone, Copy)]
pub struct Case {
    /// Kurzname des Falls fuer das Boot-Log.
    pub name: &'static str,
    /// Erwarteter Fehler.
    pub want: MapError,
    /// Wurde er tatsaechlich gemeldet?
    pub hit: bool,
}

/// Fuehrt die Fehlerfall-Tests aus und meldet sie ins Boot-Log.
///
/// Rueckgabe: `(bestanden, gesamt)`.
///
/// # Safety
/// `space` muss der AKTIVE Adressraum sein; die benutzten Probe-Adressen
/// duerfen sonst nirgends abgebildet sein.
pub unsafe fn map_errors(space: &mut AddressSpace) -> (usize, usize) {
    let mut cases: [Case; 4] = [
        Case { name: "misaligned", want: MapError::Misaligned, hit: false },
        Case { name: "already-mapped", want: MapError::AlreadyMapped, hit: false },
        Case { name: "not-mapped", want: MapError::NotMapped, hit: false },
        Case { name: "out-of-frames", want: MapError::OutOfFrames, hit: false },
    ];

    let mut guard = crate::mm::frame::lock();
    let fa = guard.as_mut().expect("Frame-Allocator");

    // 1) Nicht seitenausgerichtete virtuelle Adresse.
    let unaligned = VirtAddr(PROBE_BASE + 0x123);
    cases[0].hit = unsafe { space.map(unaligned, PhysAddr(0x1000), MapFlags::KERNEL_DATA, &mut *fa) }
        == Err(MapError::Misaligned);

    // 2) Doppeltes Mapping derselben Seite.
    let v = VirtAddr(PROBE_BASE);
    if let Some(frame) = fa.alloc_frame() {
        if unsafe { space.map(v, frame, MapFlags::KERNEL_DATA, &mut *fa) }.is_ok() {
            cases[1].hit = unsafe { space.map(v, frame, MapFlags::KERNEL_DATA, &mut *fa) }
                == Err(MapError::AlreadyMapped);
            if let Ok(f) = unsafe { space.unmap(v) } {
                // 3) Jetzt ist sie weg: erneutes unmap muss NotMapped melden.
                cases[2].hit = unsafe { space.unmap(v) } == Err(MapError::NotMapped);
                fa.free_frame(f);
            }
        } else {
            fa.free_frame(frame);
        }
    }

    // 4) Zwischentabelle noetig, aber keine Frames zu bekommen.
    //    Adresse in einem noch voellig leeren Teilbaum waehlen.
    let starving = VirtAddr(PROBE_BASE + 64 * (PAGE_SIZE as u64) * 512 * 512);
    let mut starved = StarvedAlloc;
    cases[3].hit =
        unsafe { space.map(starving, PhysAddr(0x1000), MapFlags::KERNEL_DATA, &mut starved) }
            == Err(MapError::OutOfFrames);

    let ok = cases.iter().filter(|c| c.hit).count();
    for c in cases.iter() {
        if !c.hit {
            klog!("mm", "Selbsttest MapError: FEHLER — {} ({:?}) nicht ausgeloest", c.name, c.want);
        }
    }
    klog!("mm", "Selbsttest MapError: {}/{} Fehlerfaelle nachweisbar ausgeloest", ok, cases.len());
    (ok, cases.len())
}
