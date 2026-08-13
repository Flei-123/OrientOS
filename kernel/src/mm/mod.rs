//! Schicht **mm** — Speicherverwaltung.
//!
//! Entscheidet, WAS abgebildet wird und wem welcher Frame gehoert. WIE eine
//! Abbildung in Hardware aussieht, weiss ausschliesslich `arch`. In dieser
//! Datei steht deshalb kein einziges Hardware-Tabellenbit.

pub mod frame;
pub mod heap;
pub mod selftest;

use karst_mem::region::{summarize, MemoryRegion};

use crate::klog;

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
    // W^X und Lesbarkeit gelten fuer JEDEN Bereich des Kernelabbilds. Ein
    // Aufrufer, der hier versehentlich WRITE|EXEC uebergibt, wuerde genau das
    // wieder einbauen, was diese Funktion beim Bootloader abraeumt.
    assert!(
        flags.is_sane(),
        "Rechte {} verletzen W^X oder sind nicht lesbar",
        flags.rwx()
    );
    let mut n = 0;
    let mut v = virt_start & !(PAGE_SIZE as u64 - 1);
    let end = (virt_end + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);
    while v < end {
        // Nicht kanonische Adressen kann keine MMU aufloesen; sie kaemen aus
        // kaputten Linkersymbolen und wuerden sonst still auf eine ganz andere
        // Seite umgerechnet.
        if !VirtAddr(v).is_canonical() {
            return Err(MapError::Misaligned);
        }
        let p = v - virt_base + phys_base;
        unsafe { space.map(VirtAddr(v), PhysAddr(p), flags, &mut *fa)? };
        n += 1;
        // Am oberen Ende des Adressraums darf der Zaehler nicht auf 0
        // umschlagen — das waere eine Endlosschleife, die den halben
        // Adressraum neu abbildet.
        match VirtAddr(v).checked_add(PAGE_SIZE as u64) {
            Some(next) => v = next.as_u64(),
            None => break,
        }
    }
    Ok(n)
}

/// Prueft stichprobenfrei (jede Seite!), dass ein virtueller Bereich genau auf
/// den erwarteten physischen Bereich zeigt.
///
/// Gibt `(geprueft, falsch)` zurueck. Damit laesst sich nach dem Umschalten
/// belegen, dass das Abbild nicht nur *irgendwie*, sondern **richtig** liegt —
/// ein um eine Seite verschobenes Mapping faellt sonst erst als sinnloser
/// Absturz irgendwo im Code auf.
fn verify_range(
    space: &AddressSpace,
    virt_start: u64,
    virt_end: u64,
    virt_base: u64,
    phys_base: u64,
) -> (u64, u64) {
    let mut checked = 0u64;
    let mut wrong = 0u64;
    let mut v = virt_start & !(PAGE_SIZE as u64 - 1);
    let end = (virt_end + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);
    while v < end {
        let want = PhysAddr(v - virt_base + phys_base);
        match space.translate(VirtAddr(v)) {
            Some(got) if got == want => checked += 1,
            _ => wrong += 1,
        }
        v += PAGE_SIZE as u64;
    }
    (checked, wrong)
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

    // Haertung gegen widerspruechliche Bootloader-Angaben: ein falscher
    // HHDM-Offset wuerde beim Umschalten zu einem Dreifachfehler fuehren,
    // dessen Ursache im Nachhinein nicht mehr feststellbar ist.
    assert!(
        hhdm != 0 && hhdm == crate::kcore::mem::hhdm_offset(),
        "HHDM-Offset des Bootloaders passt nicht zum eingetragenen Wert"
    );
    assert!(
        hhdm_len >= large,
        "Memory-Map ergibt ein leeres Direct-Map-Fenster"
    );
    assert!(
        sections.text.1 > sections.text.0 && sections.image.1 > sections.image.0,
        "Linkersymbole des Kernelabbilds sind unbrauchbar"
    );

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

    // Vor der Umschaltung: die drei Dinge pruefen, ohne die der naechste Befehl
    // nach `activate()` einen nicht diagnostizierbaren Dreifachfehler ausloest —
    // die laufende Funktion selbst, der aktuelle Stapel und das HHDM-Fenster
    // (ueber das die Seitentabellen erreicht werden). Fehlt eines, wird NICHT
    // umgeschaltet, sondern ein Fehler gemeldet; der alte Adressraum bleibt
    // gueltig, und der Panic-Handler kann noch berichten.
    let here = build_kernel_space as *const () as u64;
    let checks = [
        (here, "Kernelcode"),
        (sp, "Stapel"),
        (hhdm, "HHDM-Fenster"),
        (sections.rodata.0, "rodata"),
        (sections.data.0, "Daten"),
    ];
    let mut checked = 0u32;
    for (addr, what) in checks {
        if space.translate(VirtAddr(addr)).is_none() {
            klog!(
                "mm",
                "Umschaltung abgebrochen: {} bei {:#018x} fehlt im neuen Adressraum",
                what,
                addr
            );
            return Err(MapError::NotMapped);
        }
        checked += 1;
    }
    klog!(
        "mm",
        "Vor Umschaltung geprueft: {}/{} Pflichtbereiche im neuen Adressraum erreichbar",
        checked,
        checks.len()
    );

    // Nicht nur "irgendwo abgebildet", sondern RICHTIG abgebildet: jede Seite
    // des Kernelabbilds muss auf genau den physischen Ort zeigen, an den der
    // Bootloader sie geladen hat. Eine um eine Seite verschobene Abbildung
    // besteht die Erreichbarkeitspruefung oben, stuerzt aber nach dem
    // Umschalten an unvorhersehbarer Stelle ab.
    let mut img_ok = 0u64;
    let mut img_bad = 0u64;
    for (start, end) in [sections.text, sections.rodata, sections.data] {
        let (c, w) = verify_range(&space, start, end, kernel_virt, kernel_phys);
        img_ok += c;
        img_bad += w;
    }
    // Dasselbe fuer die Raender des Direct-Map-Fensters: erste und letzte
    // grosse Seite muessen auf phys 0 bzw. hhdm_len - LARGE zeigen.
    let hhdm_edges = [0u64, hhdm_len - large];
    let mut hhdm_bad = 0u64;
    for off in hhdm_edges {
        if space.translate(VirtAddr(hhdm + off)) != Some(PhysAddr(off)) {
            hhdm_bad += 1;
        }
    }
    if img_bad > 0 || hhdm_bad > 0 {
        klog!(
            "mm",
            "Umschaltung abgebrochen: {} Abbildseiten und {} HHDM-Raender zeigen woanders hin",
            img_bad,
            hhdm_bad
        );
        return Err(MapError::NotMapped);
    }
    klog!(
        "mm",
        "Abbild nachgeprueft: {} Seiten virt->phys korrekt (.text {}, .rodata {}, .data {}), HHDM-Raender ok",
        img_ok,
        MapFlags::KERNEL_CODE.rwx(),
        MapFlags::READ.rwx(),
        MapFlags::KERNEL_DATA.rwx()
    );

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
