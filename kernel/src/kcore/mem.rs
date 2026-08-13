//! Architekturneutrale Speicher-Grundtypen und -Traits.
//!
//! **Schicht: core.** Diese Datei ist die Schnittstelle zwischen `mm` und `arch`
//! und enthaelt deshalb KEIN einziges x86-Detail: kein Steuerregister, kein
//! Hardware-Tabellenbit, kein Assembler.
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
    /// Aus einer rohen Zahl.
    #[inline]
    pub const fn new(a: u64) -> Self {
        Self(a)
    }
    /// Rohe Zahl.
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    /// Auf ein Vielfaches von `align` abrunden (`align` ist eine Zweierpotenz).
    #[inline]
    pub const fn align_down(self, align: u64) -> Self {
        Self(self.0 & !(align - 1))
    }
    /// Auf ein Vielfaches von `align` aufrunden (ungeprueft, siehe
    /// [`PhysAddr::checked_align_up`]).
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        Self((self.0 + align - 1) & !(align - 1))
    }
    /// Ist die Adresse ein Vielfaches von `align`?
    #[inline]
    pub const fn is_aligned(self, align: u64) -> bool {
        self.0 & (align - 1) == 0
    }
    /// Addition ohne stillen Ueberlauf. `None` heisst: die Rechnung waere am
    /// oberen Ende des physischen Adressraums umgeschlagen — der Aufrufer haette
    /// danach mit einer viel zu kleinen Adresse weitergearbeitet.
    #[inline]
    pub const fn checked_add(self, n: u64) -> Option<Self> {
        match self.0.checked_add(n) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }
    /// Aufrunden ohne Ueberlauf — die ungepruefte Variante [`PhysAddr::align_up`]
    /// liefert am Adressraumende 0 zurueck.
    #[inline]
    pub const fn checked_align_up(self, align: u64) -> Option<Self> {
        match self.0.checked_add(align - 1) {
            Some(v) => Some(Self(v & !(align - 1))),
            None => None,
        }
    }
}

impl VirtAddr {
    /// Aus einer rohen Zahl.
    #[inline]
    pub const fn new(a: u64) -> Self {
        Self(a)
    }
    /// Rohe Zahl.
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    /// Als roher Zeiger — der einzige Uebergang von Adresse zu Speicherzugriff.
    #[inline]
    pub const fn as_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }
    /// Auf ein Vielfaches von `align` abrunden (`align` ist eine Zweierpotenz).
    #[inline]
    pub const fn align_down(self, align: u64) -> Self {
        Self(self.0 & !(align - 1))
    }
    /// Auf ein Vielfaches von `align` aufrunden (ungeprueft, siehe
    /// [`PhysAddr::checked_align_up`]).
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        Self((self.0 + align - 1) & !(align - 1))
    }
    /// Ist die Adresse ein Vielfaches von `align`?
    #[inline]
    pub const fn is_aligned(self, align: u64) -> bool {
        self.0 & (align - 1) == 0
    }
    /// Ist die Adresse **kanonisch**, also von einer 64-Bit-MMU ueberhaupt
    /// aufloesbar? Gefordert ist, dass die oberen 16 Bit die Kopie von Bit 47
    /// sind (48-Bit-Adressraum, wie ihn x86_64 ohne LA57 und aarch64 mit
    /// 48-Bit-VA verwenden). Eine nicht kanonische Adresse ist kein
    /// "unbelegtes Mapping", sondern ein Programmfehler und muss von den
    /// Adressraumroutinen abgewiesen statt umgerechnet werden.
    #[inline]
    pub const fn is_canonical(self) -> bool {
        let top = self.0 >> 47;
        top == 0 || top == 0x1_ffff
    }
    /// Addition ohne stillen Ueberlauf (siehe [`PhysAddr::checked_add`]).
    #[inline]
    pub const fn checked_add(self, n: u64) -> Option<Self> {
        match self.0.checked_add(n) {
            Some(v) => Some(Self(v)),
            None => None,
        }
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
/// Die Uebersetzung in Hardware-Tabellenbits passiert ausschliesslich in `arch`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapFlags(pub u32);

impl MapFlags {
    /// Lesbar.
    pub const READ: MapFlags = MapFlags(1 << 0);
    /// Beschreibbar.
    pub const WRITE: MapFlags = MapFlags(1 << 1);
    /// Ausfuehrbar (ohne dieses Bit setzt `arch` NX).
    pub const EXEC: MapFlags = MapFlags(1 << 2);
    /// Auch aus dem Userspace erreichbar.
    pub const USER: MapFlags = MapFlags(1 << 3);
    /// Nicht cachebar (MMIO).
    pub const NO_CACHE: MapFlags = MapFlags(1 << 4);
    /// Global (bleibt ueber Adressraumwechsel im TLB) — Hinweis, darf ignoriert werden.
    pub const GLOBAL: MapFlags = MapFlags(1 << 5);

    /// Kerneldaten: lesen und schreiben, nicht ausfuehrbar.
    pub const KERNEL_DATA: MapFlags = MapFlags(0b0000_0011);
    /// Kernelcode: lesen und ausfuehren, nicht beschreibbar.
    pub const KERNEL_CODE: MapFlags = MapFlags(0b0000_0101);

    /// Sind alle Bits aus `o` gesetzt?
    #[inline]
    pub const fn contains(self, o: MapFlags) -> bool {
        self.0 & o.0 == o.0
    }
    /// Vereinigung zweier Rechtesaetze.
    #[inline]
    pub const fn union(self, o: MapFlags) -> MapFlags {
        MapFlags(self.0 | o.0)
    }
    /// Rohe Bits — nur fuer `arch`, das sie in Hardwarebits uebersetzt.
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Kurzschreibweise der Zugriffsrechte fuer Log-Zeilen (`R`, `RW`, `RX`, ...).
    pub const fn rwx(self) -> &'static str {
        match (
            self.contains(MapFlags::READ),
            self.contains(MapFlags::WRITE),
            self.contains(MapFlags::EXEC),
        ) {
            (true, true, true) => "RWX",
            (true, true, false) => "RW",
            (true, false, true) => "RX",
            (true, false, false) => "R",
            (false, true, true) => "WX",
            (false, true, false) => "W",
            (false, false, true) => "X",
            (false, false, false) => "-",
        }
    }

    /// Sind diese Rechte in sich sinnvoll?
    ///
    /// Zwei Bedingungen, die jede Abbildung des Kernels erfuellen muss:
    /// * lesbar muss sie sein — eine nur beschreibbare oder nur ausfuehrbare
    ///   Seite gibt es auf keiner der Zielarchitekturen,
    /// * **W^X**: schreibbar UND ausfuehrbar zugleich ist eine Einladung, Daten
    ///   als Code auszufuehren. Genau das raeumt [`crate::mm::build_kernel_space`]
    ///   nach dem Bootloader auf; ein Aufrufer, der es wieder einfuehrt, soll
    ///   auffallen statt still durchzukommen.
    #[inline]
    pub const fn is_sane(self) -> bool {
        self.contains(MapFlags::READ)
            && !(self.contains(MapFlags::WRITE) && self.contains(MapFlags::EXEC))
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

impl MapError {
    /// Klartextbegruendung fuer das Boot-Log: `Debug` liefert nur den
    /// Variantennamen, hier steht, was der Fehler bedeutet.
    pub const fn describe(self) -> &'static str {
        match self {
            MapError::OutOfFrames => "kein physischer Frame mehr verfuegbar",
            MapError::AlreadyMapped => "Adresse ist bereits abgebildet",
            MapError::NotMapped => "Adresse ist nicht abgebildet",
            MapError::Misaligned => "Adresse nicht seitenausgerichtet",
            MapError::ParentHugePage => "eine grosse Seite versperrt den Weg",
        }
    }
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} ({})", self, self.describe())
    }
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
///
/// Der Offset darf nur ein einziges Mal von 0 auf einen echten Wert wechseln:
/// jede spaetere Aenderung wuerde alle bereits berechneten Zeiger auf
/// Seitentabellen und die Frame-Bitmap ungueltig machen. Ein zweiter Aufruf
/// mit demselben Wert ist harmlos (`true`), einer mit einem anderen Wert wird
/// abgewiesen (`false`) — statt stillschweigend den Kernel zu zerlegen.
pub fn set_hhdm_offset(off: u64) -> bool {
    HHDM_OFFSET
        .compare_exchange(0, off, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
        || HHDM_OFFSET.load(Ordering::SeqCst) == off
}

/// Aktueller HHDM-Offset.
#[inline]
pub fn hhdm_offset() -> u64 {
    HHDM_OFFSET.load(Ordering::Relaxed)
}

/// Ist das Direct-Map-Fenster schon bekannt? Vor diesem Zeitpunkt darf kein
/// physischer Speicher ueber [`phys_to_virt`] angefasst werden.
#[inline]
pub fn hhdm_ready() -> bool {
    hhdm_offset() != 0
}

/// Physische Adresse ueber das Direct-Map-Fenster ansprechbar machen.
#[inline]
pub fn phys_to_virt(p: PhysAddr) -> VirtAddr {
    VirtAddr(p.0 + hhdm_offset())
}

/// Wie [`phys_to_virt`], aber ohne blindes Vertrauen: `None`, solange das
/// Fenster nicht eingerichtet ist oder die Rechnung ueberlaufen wuerde.
#[inline]
pub fn phys_to_virt_checked(p: PhysAddr) -> Option<VirtAddr> {
    let off = hhdm_offset();
    if off == 0 {
        return None;
    }
    p.0.checked_add(off).map(VirtAddr)
}

/// Umkehrung von [`phys_to_virt`] — nur fuer Adressen im HHDM-Fenster gueltig.
#[inline]
pub fn virt_to_phys_hhdm(v: VirtAddr) -> PhysAddr {
    PhysAddr(v.0 - hhdm_offset())
}

/// Wie [`virt_to_phys_hhdm`], aber ohne blindes Vertrauen: `None`, solange das
/// Fenster nicht eingerichtet ist oder `v` gar nicht darin liegt. Die
/// ungepruefte Variante wuerde in beiden Faellen eine unsinnige, aber
/// harmlos aussehende physische Adresse liefern.
#[inline]
pub fn virt_to_phys_hhdm_checked(v: VirtAddr) -> Option<PhysAddr> {
    let off = hhdm_offset();
    if off == 0 || v.0 < off {
        return None;
    }
    Some(PhysAddr(v.0 - off))
}

/// Liegt `v` im Direct-Map-Fenster fuer physischen Speicher bis `phys_limit`?
#[inline]
pub fn hhdm_contains(v: VirtAddr, phys_limit: u64) -> bool {
    match virt_to_phys_hhdm_checked(v) {
        Some(p) => p.0 < phys_limit,
        None => false,
    }
}

/// Selbsttest der Adressrechnung im laufenden Kernel.
///
/// Diese Datei ist Grundlage jeder Speicheroperation, hat aber keine
/// Host-Tests: der Kernel ist ein `no_std`-Binary und wird nicht mit `cargo
/// test` uebersetzt. Damit die **gepruefte** Rechnerei nicht bloss behauptet
/// ist, laeuft sie hier im Boot einmal gegen ihre Randfaelle. Kein Zugriff auf
/// Speicher, nur Arithmetik — der Test ist damit auch dann gefahrlos, wenn
/// noch gar nichts abgebildet ist.
///
/// Geprueft werden:
/// 1. Ausrichtung: `align_down`/`align_up`/`is_aligned` passen zueinander,
/// 2. [`PhysAddr::checked_add`] und [`PhysAddr::checked_align_up`] melden den
///    Ueberlauf am Adressraumende, statt still auf 0 umzuschlagen,
/// 3. [`VirtAddr::is_canonical`] trennt die beiden gueltigen Haelften von der
///    nicht abbildbaren Mitte,
/// 4. der HHDM-Offset laesst sich nicht auf einen ANDEREN Wert umbiegen
///    ([`set_hhdm_offset`] weist das ab; derselbe Wert bleibt erlaubt),
/// 5. [`phys_to_virt_checked`] meldet einen Ueberlauf statt einer falschen
///    Adresse, und hin/zurueck ergibt wieder den Ausgangswert,
/// 6. [`virt_to_phys_hhdm_checked`] und [`hhdm_contains`] weisen Adressen
///    unterhalb bzw. jenseits des Fensters ab,
/// 7. [`MapFlags::is_sane`] erkennt W^X-Verstoesse und nicht lesbare Rechte,
/// 8. jede [`MapError`]-Variante hat eine nichtleere Klartextbegruendung.
///
/// Rueckgabe: `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    let total = 8usize;
    let mut ok = 0usize;

    // 1 — Ausrichtung.
    let a = PhysAddr(0x1234_5678);
    let v = VirtAddr(0xffff_8000_0000_1001);
    if a.align_down(PAGE_SIZE as u64).is_aligned(PAGE_SIZE as u64)
        && a.align_up(PAGE_SIZE as u64).is_aligned(PAGE_SIZE as u64)
        && a.align_up(PAGE_SIZE as u64).as_u64() - a.align_down(PAGE_SIZE as u64).as_u64()
            == PAGE_SIZE as u64
        && v.align_down(PAGE_SIZE as u64).as_u64() == 0xffff_8000_0000_1000
        && v.align_up(PAGE_SIZE as u64).as_u64() == 0xffff_8000_0000_2000
        && PhysAddr(0x2000).align_up(PAGE_SIZE as u64).as_u64() == 0x2000
        && !PhysAddr(1).is_aligned(PAGE_SIZE as u64)
    {
        ok += 1;
    }

    // 2 — Ueberlauf am oberen Ende.
    if PhysAddr(u64::MAX).checked_add(1).is_none()
        && PhysAddr(u64::MAX).checked_align_up(PAGE_SIZE as u64).is_none()
        && PhysAddr(u64::MAX - PAGE_SIZE as u64).checked_add(1).is_some()
        && VirtAddr(u64::MAX).checked_add(1).is_none()
    {
        ok += 1;
    }

    // 3 — Kanonizitaet.
    if VirtAddr(0x0000_7fff_ffff_ffff).is_canonical()
        && VirtAddr(0xffff_8000_0000_0000).is_canonical()
        && !VirtAddr(0x0000_8000_0000_0000).is_canonical()
        && !VirtAddr(0xffff_7fff_ffff_ffff).is_canonical()
    {
        ok += 1;
    }

    // 4 — der HHDM-Offset ist unveraenderlich, sobald er steht.
    let off = hhdm_offset();
    if off != 0 && !set_hhdm_offset(off ^ 0x1000) && set_hhdm_offset(off) && hhdm_offset() == off {
        ok += 1;
    }

    // 5 — Hin- und Rueckrechnung, Ueberlauf gemeldet.
    let p = PhysAddr(0x10_0000);
    if phys_to_virt_checked(p).map(virt_to_phys_hhdm) == Some(p)
        && phys_to_virt_checked(PhysAddr(u64::MAX)).is_none()
        && phys_to_virt(p).as_u64() == p.as_u64() + off
    {
        ok += 1;
    }

    // 6 — ausserhalb des Fensters.
    if virt_to_phys_hhdm_checked(VirtAddr(off - 1)).is_none()
        && virt_to_phys_hhdm_checked(VirtAddr(off)) == Some(PhysAddr(0))
        && hhdm_contains(VirtAddr(off + 0x1000), 0x1_0000)
        && !hhdm_contains(VirtAddr(off + 0x1_0000), 0x1_0000)
        && !hhdm_contains(VirtAddr(0x1000), 0x1_0000)
    {
        ok += 1;
    }

    // 7 — Rechte.
    if MapFlags::KERNEL_DATA.is_sane()
        && MapFlags::KERNEL_CODE.is_sane()
        && !MapFlags::KERNEL_DATA.union(MapFlags::EXEC).is_sane()
        && !MapFlags::WRITE.is_sane()
        && MapFlags::KERNEL_CODE.rwx().len() == 2
    {
        ok += 1;
    }

    // 8 — jede Fehlervariante erklaert sich.
    let alle = [
        MapError::OutOfFrames,
        MapError::AlreadyMapped,
        MapError::NotMapped,
        MapError::Misaligned,
        MapError::ParentHugePage,
    ];
    if alle.iter().all(|e| !e.describe().is_empty()) {
        ok += 1;
    }

    (ok, total)
}
