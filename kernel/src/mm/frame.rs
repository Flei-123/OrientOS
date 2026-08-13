//! Physischer Frame-Allocator (Bitmap).
//!
//! **Schicht: mm.** Kennt keine Seitentabellen und keine x86-Details — nur
//! "welcher 4-KiB-Frame ist frei". Die eigentliche Bitmap-Logik liegt in der
//! host-getesteten Crate [`karst_mem::bitmap`]; hier steht nur die Anbindung an
//! die echte Memory-Map und der globale Zugriff.
//!
//! **Warum Bitmap und nicht Buddy** (ausfuehrlich in ARCHITECTURE.md): 1 Bit je
//! 4 KiB sind 32 KiB Metadaten pro GiB RAM, konstant und ohne Zeiger. Ein
//! Buddy-Allocator braucht Freilisten, die man erst anlegen kann, wenn es einen
//! Heap gibt — der Heap braucht aber Frames. Die Bitmap loest dieses
//! Henne-Ei-Problem, indem sie sich selbst in den erstbesten freien Bereich legt.

use crate::klog;
use karst_mem::bitmap::{AllocError, Bitmap};
use karst_mem::region::{frames_for, summarize, MemoryRegion, MemorySummary};
use spin::{Mutex, MutexGuard};

use crate::kcore::mem::{phys_to_virt, FrameAllocator, PhysAddr, PAGE_SIZE};

/// Bitmap-Allocator ueber den gesamten physischen Adressraum.
pub struct BitmapFrameAllocator {
    bm: Bitmap<'static>,
}

impl BitmapFrameAllocator {
    /// Gesamtzahl verwalteter Frames.
    pub fn total(&self) -> usize {
        self.bm.frames()
    }
    /// Freie Frames.
    pub fn free(&self) -> usize {
        self.bm.free_frames()
    }
    /// Belegte Frames.
    pub fn used(&self) -> usize {
        self.bm.used_frames()
    }
    /// Reserviert `count` zusammenhaengende Frames.
    pub fn alloc_contiguous(&mut self, count: usize) -> Result<PhysAddr, AllocError> {
        self.bm
            .alloc_contiguous(count)
            .map(|i| PhysAddr((i as u64) * PAGE_SIZE as u64))
    }
}

impl FrameAllocator for BitmapFrameAllocator {
    fn alloc_frame(&mut self) -> Option<PhysAddr> {
        let i = self.bm.alloc().ok()?;
        let p = PhysAddr((i as u64) * PAGE_SIZE as u64);
        // Zusage des Traits: Frames kommen genullt heraus. Zwingend fuer
        // Seitentabellen — ein Rest alter Bits waere ein zufaelliges Mapping.
        unsafe { core::ptr::write_bytes(phys_to_virt(p).as_ptr::<u8>(), 0, PAGE_SIZE) };
        Some(p)
    }

    fn free_frame(&mut self, frame: PhysAddr) {
        let _ = self.bm.free((frame.as_u64() / PAGE_SIZE as u64) as usize, 1);
    }
}

static ALLOCATOR: Mutex<Option<BitmapFrameAllocator>> = Mutex::new(None);

/// Kennzahlen nach der Initialisierung — fuer das Boot-Log.
#[derive(Clone, Copy)]
pub struct FrameStats {
    /// Verwaltete Frames insgesamt.
    pub total_frames: usize,
    /// Davon frei.
    pub free_frames: usize,
    /// Groesse der Bitmap in Bytes.
    pub bitmap_bytes: usize,
    /// Physische Lage der Bitmap.
    pub bitmap_at: PhysAddr,
    /// Kennzahlen der Memory-Map.
    pub summary: MemorySummary,
}

/// Baut den Frame-Allocator aus der Memory-Map auf.
///
/// # Safety
/// Genau einmal im Boot, mit einer gueltigen Memory-Map und gesetztem
/// HHDM-Offset (die Bitmap wird ueber das HHDM-Fenster beschrieben).
pub unsafe fn init(regions: &[MemoryRegion]) -> Result<FrameStats, &'static str> {
    let summary = summarize(regions);
    // Nur echter RAM — nicht die MMIO-Loecher im oberen Adressraum.
    let total_frames = frames_for(summary.ram_top);
    if total_frames == 0 {
        return Err("Memory-Map leer");
    }
    let words = Bitmap::words_needed(total_frames);
    let bytes = words * 8;

    // Platz fuer die Bitmap suchen: erste nutzbare Region, die gross genug ist
    // und oberhalb des ersten Megabytes liegt (darunter wohnt Altlast-Firmware).
    let mut storage = None;
    for r in regions {
        if !r.kind.is_allocatable() {
            continue;
        }
        let Some((s, e)) = r.page_aligned() else { continue };
        let s = if s < 0x10_0000 { 0x10_0000 } else { s };
        if e > s && (e - s) as usize >= bytes {
            storage = Some(s);
            break;
        }
    }
    let Some(storage_phys) = storage else {
        return Err("kein zusammenhaengender Platz fuer die Frame-Bitmap");
    };

    let bits: &'static mut [u64] = unsafe {
        core::slice::from_raw_parts_mut(
            phys_to_virt(PhysAddr(storage_phys)).as_ptr::<u64>(),
            words,
        )
    };
    let mut bm = Bitmap::new_all_used(bits, total_frames);

    // Nutzbare Regionen freigeben.
    for r in regions {
        if !r.kind.is_allocatable() {
            continue;
        }
        if let Some((s, e)) = r.page_aligned() {
            bm.free_range((s / PAGE_SIZE as u64) as usize, (e / PAGE_SIZE as u64) as usize);
        }
    }
    // Erstes Megabyte nie vergeben (BIOS-Datenbereich, Realmode-IVT, VGA).
    bm.reserve_range(0, (0x10_0000 / PAGE_SIZE) as usize);
    // Die Bitmap selbst gehoert nicht in den freien Pool.
    let bstart = (storage_phys / PAGE_SIZE as u64) as usize;
    let bend = bstart + (bytes + PAGE_SIZE - 1) / PAGE_SIZE;
    bm.reserve_range(bstart, bend);

    let stats = FrameStats {
        total_frames: bm.frames(),
        free_frames: bm.free_frames(),
        bitmap_bytes: bytes,
        bitmap_at: PhysAddr(storage_phys),
        summary,
    };
    *ALLOCATOR.lock() = Some(BitmapFrameAllocator { bm });
    Ok(stats)
}

/// Sperrt den globalen Frame-Allocator.
pub fn lock() -> MutexGuard<'static, Option<BitmapFrameAllocator>> {
    ALLOCATOR.lock()
}

/// Freie Frames (0, solange nicht initialisiert).
pub fn free_frames() -> usize {
    ALLOCATOR.lock().as_ref().map_or(0, |a| a.free())
}

/// Belegte Frames.
pub fn used_frames() -> usize {
    ALLOCATOR.lock().as_ref().map_or(0, |a| a.used())
}

/// Selbsttest des Frame-Allocators im laufenden Kernel.
///
/// Prueft die Zusagen, auf die sich alles andere verlaesst: ein frisch
/// gelieferter Frame ist seitenausgerichtet und genullt, er verschwindet aus
/// dem freien Pool, und nach der Rueckgabe stimmt der Zaehler wieder.
/// Rueckgabe: `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    let total = 4usize;
    let mut ok = 0usize;
    let mut g = ALLOCATOR.lock();
    let Some(a) = g.as_mut() else {
        klog!("mm", "Selbsttest Frames: FEHLER — Allocator nicht initialisiert");
        return (0, total);
    };
    let before = a.free();
    if let Some(f) = a.alloc_frame() {
        if f.is_aligned(PAGE_SIZE as u64) {
            ok += 1;
        }
        if a.free() == before - 1 {
            ok += 1;
        }
        let p = phys_to_virt(f).as_ptr::<u64>();
        let mut zero = true;
        for i in 0..(PAGE_SIZE / 8) {
            if unsafe { core::ptr::read_volatile(p.add(i)) } != 0 {
                zero = false;
                break;
            }
        }
        if zero {
            ok += 1;
        }
        a.free_frame(f);
        if a.free() == before {
            ok += 1;
        }
    }
    klog!("mm", "Selbsttest Frames: {}/{} Zusagen erfuellt, {} frei", ok, total, a.free());
    (ok, total)
}
