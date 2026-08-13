//! Architekturunabhaengige Beschreibung physischer Speicherregionen.
//!
//! Der Bootloader-spezifische Code (z. B. Limine) uebersetzt seine Antwort in
//! diese Typen; alles darueber kennt nur noch `MemoryRegion`.

use crate::{align_down, align_up, PAGE_SIZE};

/// Art einer physischen Speicherregion — bootloaderunabhaengig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// Frei benutzbarer RAM.
    Usable,
    /// Vom Bootloader belegt, nach Umschalten auf eigene Page-Tables frei.
    BootloaderReclaimable,
    /// Kernel- und Modul-Image.
    KernelAndModules,
    /// Firmware/ACPI/MMIO/defekt — niemals als RAM benutzen.
    Reserved,
    /// ACPI-Tabellen, nach dem Parsen wiederverwendbar.
    AcpiReclaimable,
    /// ACPI NVS.
    AcpiNvs,
    /// Framebuffer.
    Framebuffer,
    /// Defekter Speicher.
    BadMemory,
}

impl RegionKind {
    /// Regionen, aus denen der Frame-Allocator Frames vergeben darf.
    #[inline]
    pub const fn is_allocatable(self) -> bool {
        matches!(self, RegionKind::Usable)
    }

    /// Ist das echter Arbeitsspeicher (jetzt oder nach Rueckgewinnung)?
    /// Bestimmt die Groesse der Frame-Bitmap.
    #[inline]
    pub const fn is_real_memory(self) -> bool {
        matches!(
            self,
            RegionKind::Usable
                | RegionKind::BootloaderReclaimable
                | RegionKind::KernelAndModules
                | RegionKind::AcpiReclaimable
                | RegionKind::AcpiNvs
        )
    }

    /// Muss diese Region im Direct-Map-Fenster erreichbar sein?
    /// Alles ausser weit entfernten MMIO-Loechern und defektem Speicher.
    #[inline]
    pub const fn needs_direct_map(self) -> bool {
        !matches!(self, RegionKind::Reserved | RegionKind::BadMemory)
    }

    pub const fn name(self) -> &'static str {
        match self {
            RegionKind::Usable => "usable",
            RegionKind::BootloaderReclaimable => "bootloader-reclaimable",
            RegionKind::KernelAndModules => "kernel+modules",
            RegionKind::Reserved => "reserved",
            RegionKind::AcpiReclaimable => "acpi-reclaimable",
            RegionKind::AcpiNvs => "acpi-nvs",
            RegionKind::Framebuffer => "framebuffer",
            RegionKind::BadMemory => "bad-memory",
        }
    }
}

/// Eine physische Speicherregion [start, start+len).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRegion {
    pub start: u64,
    pub len: u64,
    pub kind: RegionKind,
}

impl MemoryRegion {
    pub const fn new(start: u64, len: u64, kind: RegionKind) -> Self {
        Self { start, len, kind }
    }

    #[inline]
    pub const fn end(&self) -> u64 {
        self.start + self.len
    }

    /// Auf Seitengrenzen eingeschraenkte Region (start aufrunden, end abrunden).
    /// Gibt `None` zurueck, wenn danach nichts uebrig bleibt.
    pub fn page_aligned(&self) -> Option<(u64, u64)> {
        let s = align_up(self.start, PAGE_SIZE);
        let e = align_down(self.end(), PAGE_SIZE);
        if e > s {
            Some((s, e))
        } else {
            None
        }
    }
}

/// Zusammenfassung einer Memory-Map.
///
/// Die drei Obergrenzen sind bewusst getrennt — sie unterscheiden sich auf
/// echten Maschinen um Groessenordnungen:
/// * [`MemorySummary::ram_top`] begrenzt die Frame-Bitmap. Wuerde man dafuer
///   [`MemorySummary::highest_address`] nehmen, kostete ein PCI-Loch bei
///   1 TiB eine 32-MiB-Bitmap fuer Speicher, den es gar nicht gibt.
/// * [`MemorySummary::mapped_top`] begrenzt das HHDM-Fenster: es muss auch den
///   Framebuffer erreichen, aber keine weit entfernten MMIO-Loecher.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemorySummary {
    /// Summe aller Regionen.
    pub total_bytes: u64,
    /// Summe der frei benutzbaren Regionen.
    pub usable_bytes: u64,
    /// Summe der spaeter zurueckgewinnbaren Bootloader-Regionen.
    pub reclaimable_bytes: u64,
    /// Hoechste Adresse ueberhaupt (inklusive MMIO-Loecher).
    pub highest_address: u64,
    /// Hoechste Adresse, aus der je ein Frame vergeben werden kann.
    pub ram_top: u64,
    /// Hoechste Adresse, die das Direct-Map-Fenster abdecken muss.
    pub mapped_top: u64,
}

/// Berechnet Kennzahlen ueber eine Regionsliste.
pub fn summarize(regions: &[MemoryRegion]) -> MemorySummary {
    let mut s = MemorySummary::default();
    for r in regions {
        s.total_bytes += r.len;
        if r.kind.is_allocatable() {
            s.usable_bytes += r.len;
        }
        if r.kind == RegionKind::BootloaderReclaimable {
            s.reclaimable_bytes += r.len;
        }
        if r.end() > s.highest_address {
            s.highest_address = r.end();
        }
        if r.kind.is_real_memory() && r.end() > s.ram_top {
            s.ram_top = r.end();
        }
        if r.kind.needs_direct_map() && r.end() > s.mapped_top {
            s.mapped_top = r.end();
        }
    }
    s
}

/// Anzahl Frames, die eine Bitmap fuer `highest_address` beschreiben muss.
#[inline]
pub const fn frames_for(highest_address: u64) -> usize {
    (highest_address / PAGE_SIZE) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testrand::Lcg;

    #[test]
    fn summary_counts_only_usable() {
        let regions = [
            MemoryRegion::new(0, 0x1000, RegionKind::Reserved),
            MemoryRegion::new(0x1000, 0x10000, RegionKind::Usable),
            MemoryRegion::new(0x11000, 0x1000, RegionKind::BootloaderReclaimable),
        ];
        let s = summarize(&regions);
        assert_eq!(s.usable_bytes, 0x10000);
        assert_eq!(s.reclaimable_bytes, 0x1000);
        assert_eq!(s.highest_address, 0x12000);
        assert_eq!(s.total_bytes, 0x12000);
    }

    #[test]
    fn tops_ignore_far_mmio_holes() {
        let regions = [
            MemoryRegion::new(0x1000, 0x10000, RegionKind::Usable),
            MemoryRegion::new(0xfd00_0000, 0x40_0000, RegionKind::Framebuffer),
            // PCI-Loch bei 1 TiB — darf weder Bitmap noch HHDM aufblaehen.
            MemoryRegion::new(0xfd_0000_0000, 0x3_0000_0000, RegionKind::Reserved),
        ];
        let s = summarize(&regions);
        assert_eq!(s.highest_address, 0x100_0000_0000);
        assert_eq!(s.ram_top, 0x11000, "Bitmap darf nur echten RAM abdecken");
        assert_eq!(s.mapped_top, 0xfd40_0000, "HHDM muss den Framebuffer erreichen");
    }

    #[test]
    fn page_alignment_shrinks_region() {
        let r = MemoryRegion::new(0x1001, 0x2000, RegionKind::Usable);
        assert_eq!(r.page_aligned(), Some((0x2000, 0x3000)));
        let tiny = MemoryRegion::new(0x1001, 0x10, RegionKind::Usable);
        assert_eq!(tiny.page_aligned(), None);
    }

    /// Alle Kinds an einem Ort, damit eine neue Variante hier auffaellt.
    const ALL_KINDS: [RegionKind; 8] = [
        RegionKind::Usable,
        RegionKind::BootloaderReclaimable,
        RegionKind::KernelAndModules,
        RegionKind::Reserved,
        RegionKind::AcpiReclaimable,
        RegionKind::AcpiNvs,
        RegionKind::Framebuffer,
        RegionKind::BadMemory,
    ];

    #[test]
    fn every_kind_has_a_distinct_name() {
        let mut seen = std::vec::Vec::new();
        for k in ALL_KINDS {
            let n = k.name();
            assert!(!n.is_empty(), "{k:?} ohne Namen");
            assert!(!seen.contains(&n), "Name {n} doppelt vergeben");
            seen.push(n);
        }
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn only_usable_memory_may_be_allocated_from() {
        for k in ALL_KINDS {
            assert_eq!(k.is_allocatable(), k == RegionKind::Usable, "{k:?}");
            // Was vergeben werden darf, ist zwingend auch echter Speicher und
            // muss im Direktfenster erreichbar sein.
            if k.is_allocatable() {
                assert!(k.is_real_memory(), "{k:?}");
                assert!(k.needs_direct_map(), "{k:?}");
            }
        }
        assert!(!RegionKind::Framebuffer.is_real_memory());
        assert!(RegionKind::Framebuffer.needs_direct_map(), "MMIO-Fenster wird gemappt");
        assert!(!RegionKind::BadMemory.is_real_memory());
        assert!(!RegionKind::BadMemory.needs_direct_map());
        assert!(!RegionKind::Reserved.needs_direct_map());
        assert!(RegionKind::AcpiNvs.is_real_memory());
        assert!(RegionKind::KernelAndModules.is_real_memory());
    }

    #[test]
    fn empty_memory_map_summarizes_to_zero() {
        // Entarteter Fall: Bootloader liefert keine einzige Region.
        let s = summarize(&[]);
        assert_eq!(s, MemorySummary::default());
        assert_eq!(s.total_bytes, 0);
        assert_eq!(s.ram_top, 0);
        assert_eq!(s.mapped_top, 0);
        assert_eq!(frames_for(s.ram_top), 0);
    }

    #[test]
    fn map_without_usable_memory_yields_no_frames() {
        // Zerrissene Map, in der NUR Reserviertes steht: der Frame-Allocator
        // darf daraus keinen einzigen Frame ableiten.
        let regions = [
            MemoryRegion::new(0, 0x1000, RegionKind::Reserved),
            MemoryRegion::new(0xa0000, 0x60000, RegionKind::Reserved),
            MemoryRegion::new(0x1_0000_0000, 0x1000, RegionKind::BadMemory),
        ];
        let s = summarize(&regions);
        assert_eq!(s.usable_bytes, 0);
        assert_eq!(s.reclaimable_bytes, 0);
        assert_eq!(s.ram_top, 0, "kein echter RAM -> keine Bitmap");
        assert_eq!(s.mapped_top, 0, "nichts davon muss ins Direktfenster");
        assert_eq!(s.highest_address, 0x1_0000_1000);
        assert_eq!(frames_for(s.ram_top), 0);
    }

    #[test]
    fn fragmented_map_is_summed_completely() {
        // 64 winzige Loecher, wie sie eine reale Firmware liefert.
        let mut regions = std::vec::Vec::new();
        for i in 0..64u64 {
            let kind = if i % 4 == 0 { RegionKind::Reserved } else { RegionKind::Usable };
            regions.push(MemoryRegion::new(i * 0x2000, 0x1000, kind));
        }
        let s = summarize(&regions);
        assert_eq!(s.total_bytes, 64 * 0x1000);
        assert_eq!(s.usable_bytes, 48 * 0x1000);
        assert_eq!(s.highest_address, 63 * 0x2000 + 0x1000);
        assert_eq!(s.ram_top, s.highest_address, "letzte Region ist nutzbar");
    }

    #[test]
    fn zero_length_regions_are_harmless() {
        let regions = [
            MemoryRegion::new(0x1000, 0, RegionKind::Usable),
            MemoryRegion::new(0x2000, 0x1000, RegionKind::Usable),
        ];
        let s = summarize(&regions);
        assert_eq!(s.total_bytes, 0x1000);
        assert_eq!(s.usable_bytes, 0x1000);
        assert_eq!(s.ram_top, 0x3000);
        assert_eq!(MemoryRegion::new(0x1000, 0, RegionKind::Usable).page_aligned(), None);
    }

    #[test]
    fn region_end_and_alignment_edge_cases() {
        let exact = MemoryRegion::new(0x2000, 0x1000, RegionKind::Usable);
        assert_eq!(exact.end(), 0x3000);
        assert_eq!(exact.page_aligned(), Some((0x2000, 0x3000)));
        // Genau eine Seite, aber verschoben -> nichts bleibt uebrig.
        let shifted = MemoryRegion::new(0x2001, 0x1000, RegionKind::Usable);
        assert_eq!(shifted.page_aligned(), None);
        // Anderthalb Seiten verschoben -> genau eine Seite bleibt.
        let one = MemoryRegion::new(0x2001, 0x1fff, RegionKind::Usable);
        assert_eq!(one.page_aligned(), Some((0x3000, 0x4000)));
        // Unterhalb einer Seite.
        assert_eq!(MemoryRegion::new(1, 2, RegionKind::Usable).page_aligned(), None);
    }

    #[test]
    fn frames_for_rounds_down_to_whole_frames() {
        assert_eq!(frames_for(0), 0);
        assert_eq!(frames_for(PAGE_SIZE - 1), 0);
        assert_eq!(frames_for(PAGE_SIZE), 1);
        assert_eq!(frames_for(PAGE_SIZE + 1), 1);
        // 508 MiB wie in der QEMU-Standardkonfiguration dieses Projekts.
        assert_eq!(frames_for(508 * 1024 * 1024), 130_048);
    }

    #[test]
    fn reclaimable_counts_separately_from_usable() {
        let regions = [
            MemoryRegion::new(0x0, 0x4000, RegionKind::Usable),
            MemoryRegion::new(0x4000, 0x2000, RegionKind::BootloaderReclaimable),
            MemoryRegion::new(0x6000, 0x1000, RegionKind::KernelAndModules),
            MemoryRegion::new(0x7000, 0x1000, RegionKind::AcpiReclaimable),
        ];
        let s = summarize(&regions);
        assert_eq!(s.usable_bytes, 0x4000, "reclaimable ist noch nicht frei");
        assert_eq!(s.reclaimable_bytes, 0x2000);
        assert_eq!(s.total_bytes, 0x8000);
        assert_eq!(s.ram_top, 0x8000);
    }

    /// Eigenschaftstest: fuer zufaellige Maps muessen die Kennzahlen immer in
    /// derselben Beziehung stehen — sonst wuerde die Bitmap zu klein oder das
    /// Direktfenster zu kurz berechnet.
    #[test]
    fn summary_invariants_hold_for_random_maps() {
        let mut r = Lcg::new(0x5eed);
        for _ in 0..500 {
            let n = r.range(0, 30);
            let mut regions = std::vec::Vec::new();
            let mut cursor = 0u64;
            for _ in 0..n {
                cursor += (r.below(16) as u64) * PAGE_SIZE;
                let len = (r.below(64) as u64) * PAGE_SIZE;
                let kind = ALL_KINDS[r.below(ALL_KINDS.len())];
                regions.push(MemoryRegion::new(cursor, len, kind));
                cursor += len;
            }
            let s = summarize(&regions);
            let sum: u64 = regions.iter().map(|x| x.len).sum();
            assert_eq!(s.total_bytes, sum);
            assert!(s.usable_bytes <= s.total_bytes);
            assert!(s.reclaimable_bytes <= s.total_bytes);
            assert!(s.ram_top <= s.highest_address, "RAM kann nicht ueber allem liegen");
            assert!(s.mapped_top <= s.highest_address);
            assert!(s.usable_bytes + s.reclaimable_bytes <= s.total_bytes);
            for reg in &regions {
                // Jede nutzbare Region muss vollstaendig unter ram_top liegen.
                if reg.kind.is_allocatable() && reg.len > 0 {
                    assert!(reg.end() <= s.ram_top);
                    assert!(reg.end() <= s.mapped_top);
                }
                if let Some((a, b)) = reg.page_aligned() {
                    assert!(a % PAGE_SIZE == 0 && b % PAGE_SIZE == 0);
                    assert!(a >= reg.start && b <= reg.end() && a < b);
                }
            }
            assert!(frames_for(s.ram_top) as u64 * PAGE_SIZE <= s.ram_top);
        }
    }
}
