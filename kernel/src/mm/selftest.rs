//! Selbsttests der Adressraum-Abstraktion — **im laufenden Kernel**, nicht auf
//! dem Host.
//!
//! **Schicht: mm.** Hier wird nachgewiesen, dass die Fehlerfaelle von
//! [`MapError`] nicht nur definiert, sondern auch wirklich erreichbar sind —
//! inklusive [`MapError::ParentHugePage`] — und dass `map`/`unmap`/`translate`
//! sich an den Raendern so verhalten, wie die Doku behauptet.
//! Der Test benutzt ausschliesslich [`AddressSpaceOps`] — kein x86-Detail.
//!
//! Jeder Fall arbeitet auf einer eigenen, sonst unbenutzten virtuellen Adresse
//! und raeumt hinter sich auf, damit der Boot danach unveraendert weiterlaeuft.
//! Angefasst wird nur der Teilbaum unter [`PROBE_BASE`]; die einzige Seite, die
//! wirklich beschrieben werden koennte, ist ein regulaer allozierter Frame, der
//! am Ende wieder zurueckgegeben wird.

use crate::arch::AddressSpace;
use crate::kcore::arch_iface::AddressSpaceOps;
use crate::kcore::mem::{FrameAllocator, MapError, MapFlags, PhysAddr, VirtAddr, PAGE_SIZE};
use crate::klog;

/// Basisadresse der Testseiten. Eigener, sonst unbenutzter PML4-Eintrag (509),
/// damit weder Heap (510), Kernelabbild (511) noch HHDM (256) beruehrt werden.
pub const PROBE_BASE: u64 = 0xffff_fe80_0000_0000;

/// 1 GiB — Schrittweite, mit der ein garantiert leerer Teilbaum getroffen wird.
const GIB: u64 = 1 << 30;

/// Adresse in einem nie benutzten Teilbaum (fuer `NotMapped`).
const EMPTY_SUBTREE: u64 = PROBE_BASE + 100 * GIB;
/// Adressen, an denen nur mit neuen Zwischentabellen abgebildet werden koennte.
const STARVE_SMALL: u64 = PROBE_BASE + 200 * GIB;
/// Wie [`STARVE_SMALL`], aber fuer eine grosse Seite (1-GiB-Vielfache sind
/// automatisch 2-MiB-ausgerichtet).
const STARVE_LARGE: u64 = PROBE_BASE + 201 * GIB;

/// Nicht kanonische Adresse (Bit 47 gesetzt, obere Bits null). Die MMU koennte
/// sie nie aufloesen; die Abstraktion muss sie abweisen statt umzurechnen.
const NONCANONICAL: u64 = 0x0000_8000_0000_0000;

/// Physische Adresse jenseits der 52-Bit-Grenze eines Eintrags.
const PHYS_TOO_BIG: u64 = 1 << 52;

/// Physische Basis fuer den Grosse-Seiten-Test. Wird **nur abgebildet und
/// uebersetzt, nie gelesen oder geschrieben** — deshalb ist es unerheblich,
/// wem dieser Bereich gehoert.
const HUGE_PHYS: u64 = 0x0020_0000;

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

/// Ergebnis einer Grenzfall-Zusage (erwartetes *Gelingen*, nicht Scheitern).
#[derive(Clone, Copy)]
pub struct Edge {
    /// Kurzname fuer das Boot-Log.
    pub name: &'static str,
    /// Zusage erfuellt?
    pub hit: bool,
}

/// Anzahl der geprueften Fehlerfaelle.
const N_ERR: usize = 17;
/// Anzahl der geprueften Grenzfall-Zusagen.
const N_EDGE: usize = 7;

/// Fuehrt Fehlerfall- und Grenzfalltests aus und meldet sie ins Boot-Log.
///
/// Rueckgabe: `(bestanden, gesamt)` ueber **beide** Gruppen zusammen, damit die
/// Startbilanz in `kmain` jeden einzelnen Fall mitzaehlt.
///
/// # Safety
/// `space` muss der AKTIVE Adressraum sein; die benutzten Probe-Adressen
/// duerfen sonst nirgends abgebildet sein.
pub unsafe fn map_errors(space: &mut AddressSpace) -> (usize, usize) {
    let mut errs: [Case; N_ERR] = [
        Case { name: "map-virt-unausgerichtet", want: MapError::Misaligned, hit: false },
        Case { name: "map-phys-unausgerichtet", want: MapError::Misaligned, hit: false },
        Case { name: "map-virt-nicht-kanonisch", want: MapError::Misaligned, hit: false },
        Case { name: "map-phys-zu-gross", want: MapError::Misaligned, hit: false },
        Case { name: "maplarge-virt-unausgerichtet", want: MapError::Misaligned, hit: false },
        Case { name: "unmap-unausgerichtet", want: MapError::Misaligned, hit: false },
        Case { name: "map-doppelt", want: MapError::AlreadyMapped, hit: false },
        Case { name: "unmap-nicht-gemappt", want: MapError::NotMapped, hit: false },
        Case { name: "unmap-leerer-teilbaum", want: MapError::NotMapped, hit: false },
        Case { name: "map-ohne-frames", want: MapError::OutOfFrames, hit: false },
        Case { name: "maplarge-ohne-frames", want: MapError::OutOfFrames, hit: false },
        Case { name: "neuer-raum-ohne-frames", want: MapError::OutOfFrames, hit: false },
        Case { name: "maplarge-doppelt", want: MapError::AlreadyMapped, hit: false },
        Case { name: "map-in-grosse-seite", want: MapError::ParentHugePage, hit: false },
        Case { name: "unmap-in-grosse-seite", want: MapError::ParentHugePage, hit: false },
        Case { name: "unmaplarge-doppelt", want: MapError::NotMapped, hit: false },
        Case { name: "maplarge-ueber-4k-seite", want: MapError::AlreadyMapped, hit: false },
    ];
    let mut edges: [Edge; N_EDGE] = [
        Edge { name: "translate-mit-offset", hit: false },
        Edge { name: "translate-nach-unmap-leer", hit: false },
        Edge { name: "maplarge-abbilden", hit: false },
        Edge { name: "translate-in-grosser-seite", hit: false },
        Edge { name: "unmaplarge-gibt-frame", hit: false },
        Edge { name: "translate-nach-unmaplarge-leer", hit: false },
        Edge { name: "translate-nicht-kanonisch-leer", hit: false },
    ];

    let mut guard = crate::mm::frame::lock();
    let fa = guard.as_mut().expect("Frame-Allocator");
    let mut starved = StarvedAlloc;

    let a = VirtAddr(PROBE_BASE);
    let large = VirtAddr(PROBE_BASE + 4 * 1024 * 1024);
    let huge_phys = PhysAddr(HUGE_PHYS);
    let flags = MapFlags::KERNEL_DATA;

    // --------------------------------------------------- Ausrichtung / Bereich
    errs[0].hit = unsafe {
        space.map(VirtAddr(PROBE_BASE + 0x123), PhysAddr(0x1000), flags, &mut *fa)
    } == Err(MapError::Misaligned);
    errs[1].hit = unsafe { space.map(a, PhysAddr(0x1001), flags, &mut *fa) }
        == Err(MapError::Misaligned);
    errs[2].hit = unsafe { space.map(VirtAddr(NONCANONICAL), PhysAddr(0x1000), flags, &mut *fa) }
        == Err(MapError::Misaligned);
    errs[3].hit = unsafe { space.map(a, PhysAddr(PHYS_TOO_BIG), flags, &mut *fa) }
        == Err(MapError::Misaligned);
    errs[4].hit = unsafe {
        space.map_large(
            VirtAddr(large.as_u64() + PAGE_SIZE as u64),
            huge_phys,
            flags,
            &mut *fa,
        )
    } == Err(MapError::Misaligned);
    errs[5].hit = unsafe { space.unmap(VirtAddr(a.as_u64() + 8)) } == Err(MapError::Misaligned);

    // ------------------------------------------- doppeltes Mapping / translate
    if let Some(frame) = fa.alloc_frame() {
        if unsafe { space.map(a, frame, flags, &mut *fa) }.is_ok() {
            errs[6].hit = unsafe { space.map(a, frame, flags, &mut *fa) }
                == Err(MapError::AlreadyMapped);
            // translate muss den Versatz innerhalb der Seite mitfuehren.
            edges[0].hit = space.translate(VirtAddr(a.as_u64() + 0x321))
                == Some(PhysAddr(frame.as_u64() + 0x321));
            if let Ok(f) = unsafe { space.unmap(a) } {
                edges[1].hit = f == frame && space.translate(a).is_none();
                errs[7].hit = unsafe { space.unmap(a) } == Err(MapError::NotMapped);
            }
        }

        // --------------------------------------------------- leerer Teilbaum
        errs[8].hit =
            unsafe { space.unmap(VirtAddr(EMPTY_SUBTREE)) } == Err(MapError::NotMapped);

        // ------------------------------------------------- keine Frames mehr
        errs[9].hit = unsafe {
            space.map(VirtAddr(STARVE_SMALL), PhysAddr(0x1000), flags, &mut starved)
        } == Err(MapError::OutOfFrames);
        errs[10].hit = unsafe {
            space.map_large(VirtAddr(STARVE_LARGE), huge_phys, flags, &mut starved)
        } == Err(MapError::OutOfFrames);
        errs[11].hit = match unsafe { AddressSpace::new_empty(&mut starved) } {
            Err(e) => e == MapError::OutOfFrames,
            Ok(_) => false,
        };

        // --------------------------------------------------- grosse Seite
        // Nur abbilden und uebersetzen — auf HUGE_PHYS wird nie zugegriffen.
        if unsafe { space.map_large(large, huge_phys, MapFlags::READ, &mut *fa) }.is_ok() {
            edges[2].hit = true;
            errs[12].hit = unsafe { space.map_large(large, huge_phys, MapFlags::READ, &mut *fa) }
                == Err(MapError::AlreadyMapped);
            let inner = VirtAddr(large.as_u64() + PAGE_SIZE as u64);
            errs[13].hit = unsafe { space.map(inner, frame, flags, &mut *fa) }
                == Err(MapError::ParentHugePage);
            errs[14].hit = unsafe { space.unmap(inner) } == Err(MapError::ParentHugePage);
            edges[3].hit = space.translate(VirtAddr(large.as_u64() + 0x1_2345))
                == Some(PhysAddr(HUGE_PHYS + 0x1_2345));
            edges[4].hit = unsafe { space.unmap(large) } == Ok(huge_phys);
            edges[5].hit = space.translate(large).is_none();
            errs[15].hit = unsafe { space.unmap(large) } == Err(MapError::NotMapped);
        }

        // ------------------------- 4-KiB-Seite versperrt den Weg fuer 2 MiB
        if unsafe { space.map(large, frame, flags, &mut *fa) }.is_ok() {
            errs[16].hit = unsafe { space.map_large(large, huge_phys, MapFlags::READ, &mut *fa) }
                == Err(MapError::AlreadyMapped);
            let _ = unsafe { space.unmap(large) };
        }

        fa.free_frame(frame);
    }

    // translate darf fuer eine nicht kanonische Adresse nichts erfinden.
    edges[6].hit = space.translate(VirtAddr(NONCANONICAL)).is_none();

    let ok_err = errs.iter().filter(|c| c.hit).count();
    let ok_edge = edges.iter().filter(|e| e.hit).count();
    for c in errs.iter() {
        if !c.hit {
            klog!("mm", "Selbsttest MapError: FEHLER — {} ({:?}) nicht ausgeloest", c.name, c.want);
        }
    }
    for e in edges.iter() {
        if !e.hit {
            klog!("mm", "Selbsttest Grenzfaelle: FEHLER — {} nicht erfuellt", e.name);
        }
    }
    klog!(
        "mm",
        "Selbsttest MapError: {}/{} Fehlerfaelle nachweisbar ausgeloest \
         (Misaligned 6, AlreadyMapped 3, NotMapped 4, OutOfFrames 3, ParentHugePage 2)",
        ok_err,
        errs.len()
    );
    klog!(
        "mm",
        "Selbsttest Grenzfaelle: {}/{} Zusagen erfuellt \
         (translate mit Versatz, 2-MiB-Seite abbilden/uebersetzen/aufloesen)",
        ok_edge,
        edges.len()
    );
    (ok_err + ok_edge, errs.len() + edges.len())
}
