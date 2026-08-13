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
}
