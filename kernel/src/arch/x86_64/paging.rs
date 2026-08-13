//! 4-Ebenen-Seitentabellen von x86_64 (PML4 -> PDPT -> PD -> PT).
//!
//! **Die einzige Datei im Baum, die PTE-Bits, CR3 und `invlpg` kennt.** Nach
//! aussen zeigt sie ausschliesslich [`AddressSpaceOps`] mit den neutralen Typen
//! aus [`crate::kcore::mem`]. `mm` entscheidet, WAS abgebildet wird.
//!
//! Zugriff auf fremde Tabellen laeuft ueber das HHDM-Fenster des Bootloaders
//! (`phys + hhdm_offset`) — deshalb muss jeder selbst gebaute Adressraum dieses
//! Fenster ebenfalls enthalten, sonst waere er nach dem Umschalten blind.

use super::cpu;
use crate::kcore::arch_iface::AddressSpaceOps;
use crate::kcore::mem::{
    phys_to_virt, FrameAllocator, MapError, MapFlags, PhysAddr, VirtAddr, PAGE_SIZE,
};

/// Seite ist gueltig.
pub const PRESENT: u64 = 1 << 0;
/// Seite ist beschreibbar.
pub const WRITABLE: u64 = 1 << 1;
/// Auch aus Ring 3 erreichbar.
pub const USER: u64 = 1 << 2;
/// Write-through-Caching.
pub const WRITE_THROUGH: u64 = 1 << 3;
/// Caching aus (MMIO).
pub const CACHE_DISABLE: u64 = 1 << 4;
/// Grosse Seite (2 MiB auf PD-Ebene, 1 GiB auf PDPT-Ebene).
pub const HUGE: u64 = 1 << 7;
/// Bleibt beim CR3-Wechsel im TLB.
pub const GLOBAL: u64 = 1 << 8;
/// Kein Instruktionsabruf (braucht EFER.NXE, siehe [`super::msr::enable_nx`]).
pub const NO_EXECUTE: u64 = 1 << 63;

/// Adressbits eines Eintrags (Bit 12..51).
const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;
/// Groesse einer 2-MiB-Seite.
pub const LARGE_PAGE: usize = 2 * 1024 * 1024;

/// Erste physische Adresse, die nicht mehr in einen Eintrag passt (52 Bit).
const PHYS_LIMIT: u64 = 1 << 52;

/// Ist `v` eine kanonische 48-Bit-Adresse (Bit 47 in Bit 48..63 fortgesetzt)?
///
/// Nicht kanonische Adressen wuerde die Indexrechnung stillschweigend auf eine
/// voellig andere Seite umbiegen — deshalb werden sie abgewiesen. `MapError`
/// hat dafuer keine eigene Variante (der Fehlerkatalog in `kcore` bleibt
/// unveraendert); gemeldet wird [`MapError::Misaligned`] als "diese Adresse ist
/// so nicht abbildbar".
#[inline]
const fn is_canonical(v: u64) -> bool {
    let top = v >> 47;
    top == 0 || top == 0x1_ffff
}

/// Prueft die virtuelle Adresse auf Ausrichtung UND Kanonizitaet.
#[inline]
const fn check_virt(v: u64, align: u64) -> Result<(), MapError> {
    if v & (align - 1) != 0 || !is_canonical(v) {
        return Err(MapError::Misaligned);
    }
    Ok(())
}

/// Prueft die physische Adresse auf Ausrichtung UND Wertebereich.
///
/// Ein Wert oberhalb von 2^52 wuerde beim Zusammenbau des Eintrags in die
/// Flag-Bits (bis hin zu NX) hineinlaufen — dann waere die Seite ausfuehrbar,
/// obwohl das Gegenteil verlangt wurde. Solche Adressen werden abgewiesen.
#[inline]
const fn check_phys(p: u64, align: u64) -> Result<(), MapError> {
    if p & (align - 1) != 0 || p >= PHYS_LIMIT {
        return Err(MapError::Misaligned);
    }
    Ok(())
}

/// Eine Seitentabelle: 512 Eintraege, seitenausgerichtet.
#[repr(C, align(4096))]
pub struct PageTable {
    /// Rohe Eintraege.
    pub entries: [u64; 512],
}

#[inline]
fn table_ptr(p: PhysAddr) -> *mut PageTable {
    phys_to_virt(p).as_ptr::<PageTable>()
}

#[inline]
fn idx(v: u64, level: u32) -> usize {
    // level 4 = PML4 ... level 1 = PT
    ((v >> (12 + 9 * (level - 1))) & 0x1ff) as usize
}

/// Uebersetzt neutrale [`MapFlags`] in x86-PTE-Bits.
fn pte_flags(f: MapFlags, huge: bool) -> u64 {
    let mut b = PRESENT;
    if f.contains(MapFlags::WRITE) {
        b |= WRITABLE;
    }
    if !f.contains(MapFlags::EXEC) {
        b |= NO_EXECUTE;
    }
    if f.contains(MapFlags::USER) {
        b |= USER;
    }
    if f.contains(MapFlags::NO_CACHE) {
        b |= CACHE_DISABLE | WRITE_THROUGH;
    }
    if f.contains(MapFlags::GLOBAL) {
        b |= GLOBAL;
    }
    if huge {
        b |= HUGE;
    }
    b
}

/// Ein Adressraum von x86_64, identifiziert durch seine PML4.
pub struct X86AddressSpace {
    root: PhysAddr,
}

impl X86AddressSpace {
    /// Sorgt dafuer, dass unterhalb von `entry` eine Tabelle existiert.
    unsafe fn ensure_child(
        entry: *mut u64,
        alloc: &mut dyn FrameAllocator,
        user: bool,
    ) -> Result<PhysAddr, MapError> {
        let e = unsafe { *entry };
        if e & PRESENT == 0 {
            let frame = alloc.alloc_frame().ok_or(MapError::OutOfFrames)?;
            unsafe { core::ptr::write_bytes(phys_to_virt(frame).as_ptr::<u8>(), 0, PAGE_SIZE) };
            let mut bits = frame.as_u64() | PRESENT | WRITABLE;
            if user {
                bits |= USER;
            }
            unsafe { *entry = bits };
            Ok(frame)
        } else if e & HUGE != 0 {
            Err(MapError::ParentHugePage)
        } else {
            if user && e & USER == 0 {
                unsafe { *entry = e | USER };
            }
            Ok(PhysAddr(e & ADDR_MASK))
        }
    }

    /// Laeuft bis zur Tabelle der angegebenen Ebene hinab und legt fehlende an.
    unsafe fn walk_to(
        &mut self,
        v: u64,
        stop_level: u32,
        alloc: &mut dyn FrameAllocator,
        user: bool,
    ) -> Result<*mut PageTable, MapError> {
        let mut table = self.root;
        let mut level = 4u32;
        while level > stop_level {
            let e = unsafe { core::ptr::addr_of_mut!((*table_ptr(table)).entries[idx(v, level)]) };
            table = unsafe { Self::ensure_child(e, alloc, user)? };
            level -= 1;
        }
        Ok(table_ptr(table))
    }
}

impl AddressSpaceOps for X86AddressSpace {
    const LARGE_PAGE_SIZE: usize = LARGE_PAGE;

    unsafe fn new_empty(alloc: &mut dyn FrameAllocator) -> Result<Self, MapError> {
        let root = alloc.alloc_frame().ok_or(MapError::OutOfFrames)?;
        unsafe { core::ptr::write_bytes(phys_to_virt(root).as_ptr::<u8>(), 0, PAGE_SIZE) };
        Ok(X86AddressSpace { root })
    }

    unsafe fn current() -> Self {
        X86AddressSpace { root: PhysAddr(cpu::read_cr3() & ADDR_MASK) }
    }

    fn root(&self) -> PhysAddr {
        self.root
    }

    unsafe fn map(
        &mut self,
        virt: VirtAddr,
        phys: PhysAddr,
        flags: MapFlags,
        alloc: &mut dyn FrameAllocator,
    ) -> Result<(), MapError> {
        check_virt(virt.as_u64(), PAGE_SIZE as u64)?;
        check_phys(phys.as_u64(), PAGE_SIZE as u64)?;
        let user = flags.contains(MapFlags::USER);
        let v = virt.as_u64();
        let pt = unsafe { self.walk_to(v, 1, alloc, user)? };
        let leaf = unsafe { &mut (*pt).entries[idx(v, 1)] };
        if *leaf & PRESENT != 0 {
            return Err(MapError::AlreadyMapped);
        }
        *leaf = phys.as_u64() | pte_flags(flags, false);
        unsafe { cpu::invlpg(v) };
        Ok(())
    }

    unsafe fn map_large(
        &mut self,
        virt: VirtAddr,
        phys: PhysAddr,
        flags: MapFlags,
        alloc: &mut dyn FrameAllocator,
    ) -> Result<(), MapError> {
        check_virt(virt.as_u64(), LARGE_PAGE as u64)?;
        check_phys(phys.as_u64(), LARGE_PAGE as u64)?;
        let user = flags.contains(MapFlags::USER);
        let v = virt.as_u64();
        let pd = unsafe { self.walk_to(v, 2, alloc, user)? };
        let leaf = unsafe { &mut (*pd).entries[idx(v, 2)] };
        if *leaf & PRESENT != 0 {
            return Err(MapError::AlreadyMapped);
        }
        *leaf = phys.as_u64() | pte_flags(flags, true);
        unsafe { cpu::invlpg(v) };
        Ok(())
    }

    unsafe fn unmap(&mut self, virt: VirtAddr) -> Result<PhysAddr, MapError> {
        check_virt(virt.as_u64(), PAGE_SIZE as u64)?;
        let v = virt.as_u64();
        let mut table = self.root;
        let mut level = 4u32;
        while level > 1 {
            let ent =
                unsafe { core::ptr::addr_of_mut!((*table_ptr(table)).entries[idx(v, level)]) };
            let e = unsafe { *ent };
            if e & PRESENT == 0 {
                return Err(MapError::NotMapped);
            }
            if e & HUGE != 0 {
                // Grosse Seite. Nur ihr eigener Anfang darf sie aufloesen —
                // eine Adresse mittendrin ist ein Aufrufsfehler und bleibt
                // ParentHugePage, sonst wuerde man mehr freigeben als gedacht.
                let page = 1u64 << (12 + 9 * (level - 1));
                if v & (page - 1) != 0 {
                    return Err(MapError::ParentHugePage);
                }
                unsafe { *ent = 0 };
                unsafe { cpu::invlpg(v) };
                return Ok(PhysAddr(e & ADDR_MASK));
            }
            table = PhysAddr(e & ADDR_MASK);
            level -= 1;
        }
        let leaf = unsafe { &mut (*table_ptr(table)).entries[idx(v, 1)] };
        if *leaf & PRESENT == 0 {
            return Err(MapError::NotMapped);
        }
        let phys = PhysAddr(*leaf & ADDR_MASK);
        *leaf = 0;
        unsafe { cpu::invlpg(v) };
        Ok(phys)
    }

    fn translate(&self, virt: VirtAddr) -> Option<PhysAddr> {
        let v = virt.as_u64();
        // Nicht kanonisch = von der MMU nie aufloesbar; niemals ein Ergebnis
        // erfinden, das die Indexrechnung zufaellig liefern wuerde.
        if !is_canonical(v) {
            return None;
        }
        let mut table = self.root;
        let mut level = 4u32;
        loop {
            let e = unsafe { (*table_ptr(table)).entries[idx(v, level)] };
            if e & PRESENT == 0 {
                return None;
            }
            if level == 1 {
                return Some(PhysAddr((e & ADDR_MASK) + (v & 0xfff)));
            }
            if e & HUGE != 0 {
                let page = 1u64 << (12 + 9 * (level - 1));
                return Some(PhysAddr((e & ADDR_MASK) + (v & (page - 1))));
            }
            table = PhysAddr(e & ADDR_MASK);
            level -= 1;
        }
    }

    unsafe fn activate(&self) {
        unsafe { cpu::write_cr3(self.root.as_u64()) };
    }
}
