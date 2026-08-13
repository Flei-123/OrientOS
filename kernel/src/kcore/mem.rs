//! Architekturneutrale Speicher-Grundtypen und -Traits.
//!
//! **Schicht: core.** Diese Datei ist die Schnittstelle zwischen `mm` und `arch`
//! und enthaelt deshalb KEIN einziges x86-Detail (kein CR3, kein PTE-Bit, kein asm).
//! `mm` beschreibt *was* gemappt werden soll, `arch` weiss *wie*.

use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

/// Groesse einer Standard-Seite. Architekturen mit anderer Basisgroesse
/// definieren sie spaeter ueber `arch::PAGE_SIZE`; 4 KiB gilt fuer x86_64 und
/// aarch64 (4K-Granule) gleichermassen.
pub const PAGE_SIZE: usize = 4096;

/// Physische Adresse (Bus-/RAM-Adresse).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(pub u64);

/// Virtuelle Adresse.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(pub u64);

impl PhysAddr {
    #[inline]
    pub const fn new(a: u64) -> Self {
        Self(a)
    }
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    #[inline]
    pub const fn align_down(self, align: u64) -> Self {
        Self(self.0 & !(align - 1))
    }
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        Self((self.0 + align - 1) & !(align - 1))
    }
    #[inline]
    pub const fn is_aligned(self, align: u64) -> bool {
        self.0 & (align - 1) == 0
    }
}

impl VirtAddr {
    #[inline]
    pub const fn new(a: u64) -> Self {
        Self(a)
    }
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    #[inline]
    pub const fn as_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }
    #[inline]
    pub const fn align_down(self, align: u64) -> Self {
        Self(self.0 & !(align - 1))
    }
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        Self((self.0 + align - 1) & !(align - 1))
    }
    #[inline]
    pub const fn is_aligned(self, align: u64) -> bool {
        self.0 & (align - 1) == 0
    }
}

impl fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "phys:{:#018x}", self.0)
    }
}
impl fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#018x}", self.0)
    }
}
impl fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "virt:{:#018x}", self.0)
    }
}
impl fmt::Display for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#018x}", self.0)
    }
}

/// Zugriffsattribute fuer ein Mapping — bewusst generisch gehalten.
/// Die Uebersetzung in PTE-Bits passiert ausschliesslich in `arch`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapFlags(pub u32);

impl MapFlags {
    pub const READ: MapFlags = MapFlags(1 << 0);
    pub const WRITE: MapFlags = MapFlags(1 << 1);
    pub const EXEC: MapFlags = MapFlags(1 << 2);
    /// Auch aus dem Userspace erreichbar.
    pub const USER: MapFlags = MapFlags(1 << 3);
    /// Nicht cachebar (MMIO).
    pub const NO_CACHE: MapFlags = MapFlags(1 << 4);
    /// Global (bleibt ueber Adressraumwechsel im TLB) — Hinweis, darf ignoriert werden.
    pub const GLOBAL: MapFlags = MapFlags(1 << 5);

    pub const KERNEL_DATA: MapFlags = MapFlags(0b0000_0011);
    pub const KERNEL_CODE: MapFlags = MapFlags(0b0000_0101);

    #[inline]
    pub const fn contains(self, o: MapFlags) -> bool {
        self.0 & o.0 == o.0
    }
    #[inline]
    pub const fn union(self, o: MapFlags) -> MapFlags {
        MapFlags(self.0 | o.0)
    }
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for MapFlags {
    type Output = MapFlags;
    #[inline]
    fn bitor(self, o: MapFlags) -> MapFlags {
        MapFlags(self.0 | o.0)
    }
}

/// Fehler beim Manipulieren von Adressraeumen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapError {
    /// Es konnte kein Frame fuer eine Zwischentabelle beschafft werden.
    OutOfFrames,
    /// Die Seite ist bereits gemappt.
    AlreadyMapped,
    /// Die Seite ist nicht gemappt.
    NotMapped,
    /// Adresse nicht seitenausgerichtet.
    Misaligned,
    /// Ein Huge-Page-Eintrag versperrt den Weg.
    ParentHugePage,
}

/// Quelle physischer Frames. Wird von `mm` implementiert und von `arch`
/// verwendet (fuer Zwischen-Page-Tables) — deshalb steht der Trait hier
/// in der neutralen core-Schicht.
pub trait FrameAllocator {
    /// Liefert einen genullten, 4-KiB-ausgerichteten physischen Frame.
    fn alloc_frame(&mut self) -> Option<PhysAddr>;
    /// Gibt einen zuvor allozierten Frame zurueck.
    fn free_frame(&mut self, frame: PhysAddr);
}

/// Offset des "Higher Half Direct Map"-Fensters: `virt = phys + HHDM`.
/// Wird einmalig im Boot aus den Bootloader-Daten gesetzt.
static HHDM_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Setzt den HHDM-Offset (genau einmal im fruehen Boot).
pub fn set_hhdm_offset(off: u64) {
    HHDM_OFFSET.store(off, Ordering::SeqCst);
}

/// Aktueller HHDM-Offset.
#[inline]
pub fn hhdm_offset() -> u64 {
    HHDM_OFFSET.load(Ordering::Relaxed)
}

/// Physische Adresse ueber das Direct-Map-Fenster ansprechbar machen.
#[inline]
pub fn phys_to_virt(p: PhysAddr) -> VirtAddr {
    VirtAddr(p.0 + hhdm_offset())
}

/// Umkehrung von [`phys_to_virt`] — nur fuer Adressen im HHDM-Fenster gueltig.
#[inline]
pub fn virt_to_phys_hhdm(v: VirtAddr) -> PhysAddr {
    PhysAddr(v.0 - hhdm_offset())
}
