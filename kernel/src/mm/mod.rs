//! Schicht **mm** — Speicherverwaltung.
//!
//! Entscheidet, WAS abgebildet wird und wem welcher Frame gehoert. WIE eine
//! Abbildung in Hardware aussieht, weiss ausschliesslich `arch`. In dieser
//! Datei steht deshalb kein einziges PTE-Bit.

pub mod frame;
pub mod heap;
pub mod selftest;

use karst_mem::region::{summarize, MemoryRegion};

use crate::arch::AddressSpace;
use crate::kcore::arch_iface::AddressSpaceOps;
use crate::kcore::mem::{MapError, MapFlags, PhysAddr, VirtAddr, PAGE_SIZE};

unsafe extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __data_end: u8;
}

/// Die drei Rechtebereiche des Kernelabbilds, wie das Linkerskript sie markiert.
#[derive(Clone, Copy)]
pub struct KernelSections {
    /// Ausfuehrbarer Code (R+X).
    pub text: (u64, u64),
    /// Konstanten (nur lesbar, NX).
    pub rodata: (u64, u64),
    /// Daten und BSS (R+W, NX).
    pub data: (u64, u64),
    /// Gesamtbereich des Abbilds.
    pub image: (u64, u64),
}

/// Liest die Sektionsgrenzen aus den Linkersymbolen.
pub fn kernel_sections() -> KernelSections {
    let a = |r: &u8| r as *const u8 as u64;
    unsafe {
        KernelSections {
            text: (a(&__text_start), a(&__text_end)),
            rodata: (a(&__rodata_start), a(&__rodata_end)),
            data: (a(&__data_start), a(&__data_end)),
            image: (a(&__kernel_start), a(&__kernel_end)),
        }
    }
}

/// Ergebnis des Adressraumaufbaus — reine Zahlen fuer das Boot-Log.
#[derive(Clone, Copy)]
pub struct SpaceStats {
    /// Mit grossen Seiten abgebildetes HHDM-Fenster in Bytes.
    pub hhdm_bytes: u64,
    /// Anzahl grosser Seiten im HHDM-Fenster.
    pub hhdm_large_pages: u64,
    /// Abgebildete Basisseiten des Kernelabbilds.
    pub image_pages: u64,
    /// Zusaetzlich uebernommene Stapelseiten des Bootloaders.
    pub stack_pages: u64,
}

fn map_range(
    space: &mut AddressSpace,
    fa: &mut dyn crate::kcore::mem::FrameAllocator,
    virt_start: u64,
    virt_end: u64,
    virt_base: u64,
    phys_base: u64,
    flags: MapFlags,
) -> Result<u64, MapError> {
    let mut n = 0;
    let mut v = virt_start & !(PAGE_SIZE as u64 - 1);
    let end = (virt_end + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);
    while v < end {
        let p = v - virt_base + phys_base;
        unsafe { space.map(VirtAddr(v), PhysAddr(p), flags, &mut *fa)? };
        n += 1;
        v += PAGE_SIZE as u64;
    }
    Ok(n)
}

/// Baut den EIGENEN Kernel-Adressraum auf und schaltet darauf um.
///
/// Reihenfolge und Begruendung:
/// 1. **HHDM-Fenster** mit grossen Seiten — ohne das kann der Kernel keine
///    physische Adresse mehr anfassen (auch keine Seitentabelle).
/// 2. **Kernelabbild** seitengenau mit echten Rechten: Text R+X, rodata R,
///    data/bss R+W+NX. Der Bootloader mappt alles beschreibbar UND ausfuehrbar;
///    genau das raeumen wir hier auf.
/// 3. **Aktueller Stapel** — er stammt vom Bootloader. Statt zu raten, wo er
///    liegt, wird seine Umgebung aus dem alten Adressraum uebersetzt und
///    uebernommen. Das macht den Umschaltvorgang unabhaengig vom Bootloader.
///
/// # Safety
/// Schaltet CR3 um. Danach gilt ausschliesslich der neue Adressraum.
pub unsafe fn build_kernel_space(
    regions: &[MemoryRegion],
    hhdm: u64,
    kernel_phys: u64,
    kernel_virt: u64,
) -> Result<(AddressSpace, SpaceStats), MapError> {
    let sections = kernel_sections();
    let summary = summarize(regions);
    let large = AddressSpace::LARGE_PAGE_SIZE as u64;
    let hhdm_len = (summary.mapped_top + large - 1) & !(large - 1);

    let old = unsafe { AddressSpace::current() };

    let mut guard = frame::lock();
    let fa = guard.as_mut().expect("Frame-Allocator nicht initialisiert");

    let mut space = unsafe { AddressSpace::new_empty(fa)? };

    // 1) HHDM-Fenster.
    let mut off = 0u64;
    let mut large_pages = 0u64;
    while off < hhdm_len {
        unsafe {
            space.map_large(
                VirtAddr(hhdm + off),
                PhysAddr(off),
                MapFlags::KERNEL_DATA,
                &mut *fa,
            )?
        };
        large_pages += 1;
        off += large;
    }

    // 2) Kernelabbild mit echten Rechten.
    let mut image_pages = 0;
    image_pages += map_range(
        &mut space, fa, sections.text.0, sections.text.1, kernel_virt, kernel_phys,
        MapFlags::KERNEL_CODE,
    )?;
    image_pages += map_range(
        &mut space, fa, sections.rodata.0, sections.rodata.1, kernel_virt, kernel_phys,
        MapFlags::READ,
    )?;
    image_pages += map_range(
        &mut space, fa, sections.data.0, sections.data.1, kernel_virt, kernel_phys,
        MapFlags::KERNEL_DATA,
    )?;

    // 3) Stapelumgebung uebernehmen. Die Adresse einer lokalen Variablen ist der
    //    portable Weg, den aktuellen Stapel zu finden — kein Registerzugriff noetig.
    let probe: u64 = 0;
    let sp = &probe as *const u64 as u64;
    let lo = (sp - 256 * 1024) & !(PAGE_SIZE as u64 - 1);
    let hi = (sp + 64 * 1024 + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);
    let mut stack_pages = 0;
    let mut v = lo;
    while v < hi {
        if space.translate(VirtAddr(v)).is_none() {
            if let Some(p) = old.translate(VirtAddr(v)) {
                let phys = PhysAddr(p.as_u64() & !(PAGE_SIZE as u64 - 1));
                if unsafe { space.map(VirtAddr(v), phys, MapFlags::KERNEL_DATA, &mut *fa) }.is_ok() {
                    stack_pages += 1;
                }
            }
        }
        v += PAGE_SIZE as u64;
    }

    // Umschalten. Ab hier gelten nur noch unsere Tabellen.
    unsafe { space.activate() };

    Ok((
        space,
        SpaceStats {
            hhdm_bytes: hhdm_len,
            hhdm_large_pages: large_pages,
            image_pages,
            stack_pages,
        },
    ))
}
