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
    /// Wie oft konnte eine Anforderung nicht bedient werden (Speicher leer).
    starved: usize,
    /// Wie oft wurde eine unsinnige Rueckgabe abgewiesen (nicht ausgerichtet,
    /// ausserhalb der Bitmap oder doppelt freigegeben).
    bad_frees: usize,
    /// Kleinster je erreichter Stand freier Frames — Wasserstandsmarke.
    low_water: usize,
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
    /// Abgewiesene Anforderungen mangels freier Frames.
    pub fn starved(&self) -> usize {
        self.starved
    }
    /// Abgewiesene, unsinnige Rueckgaben.
    pub fn bad_frees(&self) -> usize {
        self.bad_frees
    }
    /// Wenigste je gleichzeitig freien Frames.
    pub fn low_water(&self) -> usize {
        self.low_water
    }

    /// Reserviert `count` zusammenhaengende Frames — **ohne** sie zu nullen.
    /// Fuer grosse Bloecke (Bitmap-Selbsttest, spaeter DMA-Puffer), bei denen
    /// das Nullen den Aufrufer nichts angeht.
    pub fn alloc_contiguous(&mut self, count: usize) -> Result<PhysAddr, AllocError> {
        match self.bm.alloc_contiguous(count) {
            Ok(i) => {
                self.note_low_water();
                Ok(PhysAddr((i as u64) * PAGE_SIZE as u64))
            }
            Err(e) => {
                self.starved += 1;
                Err(e)
            }
        }
    }

    /// Gibt `count` ab `base` zusammenhaengend belegte Frames zurueck.
    /// Prueft Ausrichtung und Bereich, bevor irgendein Bit angefasst wird.
    pub fn free_contiguous(&mut self, base: PhysAddr, count: usize) -> Result<(), AllocError> {
        if !base.is_aligned(PAGE_SIZE as u64) || count == 0 {
            self.bad_frees += 1;
            return Err(AllocError::OutOfRange);
        }
        let idx = (base.as_u64() / PAGE_SIZE as u64) as usize;
        if idx.checked_add(count).map_or(true, |e| e > self.bm.frames()) {
            self.bad_frees += 1;
            return Err(AllocError::OutOfRange);
        }
        self.bm.free(idx, count)
    }

    #[inline]
    fn note_low_water(&mut self) {
        let f = self.bm.free_frames();
        if f < self.low_water {
            self.low_water = f;
        }
    }
}

impl FrameAllocator for BitmapFrameAllocator {
    fn alloc_frame(&mut self) -> Option<PhysAddr> {
        let i = match self.bm.alloc() {
            Ok(i) => i,
            Err(_) => {
                // Erschoepfter Speicher ist ein regulaerer Zustand, kein Absturz:
                // der Aufrufer bekommt `None` und entscheidet selbst.
                self.starved += 1;
                return None;
            }
        };
        let p = PhysAddr((i as u64) * PAGE_SIZE as u64);
        // Ohne Direct-Map-Fenster kann der Frame nicht genullt werden — dann
        // waere die Zusage "genullt" gebrochen. Lieber nichts vergeben.
        let Some(v) = crate::kcore::mem::phys_to_virt_checked(p) else {
            let _ = self.bm.free(i, 1);
            self.starved += 1;
            return None;
        };
        // Zusage des Traits: Frames kommen genullt heraus. Zwingend fuer
        // Seitentabellen — ein Rest alter Bits waere ein zufaelliges Mapping.
        unsafe { core::ptr::write_bytes(v.as_ptr::<u8>(), 0, PAGE_SIZE) };
        self.note_low_water();
        Some(p)
    }

    fn free_frame(&mut self, frame: PhysAddr) {
        // Eine Rueckgabe, die nicht ausgerichtet ist, ausserhalb der Bitmap
        // liegt oder einen ohnehin freien Frame betrifft, ist ein Fehler des
        // Aufrufers. Sie wird gezaehlt statt still ausgefuehrt — sonst wuerde
        // ein doppeltes `free_frame` denselben Frame zweimal vergeben.
        if !frame.is_aligned(PAGE_SIZE as u64) {
            self.bad_frees += 1;
            return;
        }
        let idx = (frame.as_u64() / PAGE_SIZE as u64) as usize;
        if idx >= self.bm.frames() || !self.bm.is_used(idx) {
            self.bad_frees += 1;
            return;
        }
        let _ = self.bm.free(idx, 1);
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
    /// Frames, die in einer nutzbaren Region lagen, aber wegen Ueberschneidung
    /// mit einer nicht nutzbaren Region nachtraeglich gesperrt wurden.
    pub overlap_frames: usize,
    /// Frames unterhalb des ersten Megabytes, die gesperrt bleiben.
    pub low_frames: usize,
}

/// Groesste Bitmap, die dieser Kernel akzeptiert: 8 MiB beschreiben 256 GiB
/// RAM. Eine groessere Forderung kommt nicht von echtem Speicher, sondern von
/// einer kaputten Memory-Map (Muell im `ram_top`) — dann lieber sauber
/// abbrechen als 8 MiB in einen Bereich schreiben, den es nicht gibt.
const MAX_BITMAP_BYTES: usize = 8 * 1024 * 1024;

/// Baut den Frame-Allocator aus der Memory-Map auf.
///
/// Haertung gegen unglaubwuerdige Bootloader-Daten:
/// * ohne Direct-Map-Fenster ist die Bitmap nicht beschreibbar → Fehler,
/// * leere Regionsliste oder `ram_top == 0` → Fehler,
/// * absurd grosse Bitmap (> [`MAX_BITMAP_BYTES`]) → Fehler,
/// * ueberlappende Regionen: nach dem Freigeben der nutzbaren Bereiche wird
///   jede NICHT nutzbare Region nach aussen ausgerichtet nachreserviert. Eine
///   zerrissene Map kann so keinen reservierten Frame in den freien Pool
///   schmuggeln.
///
/// # Safety
/// Genau einmal im Boot, mit einer gueltigen Memory-Map und gesetztem
/// HHDM-Offset (die Bitmap wird ueber das HHDM-Fenster beschrieben).
pub unsafe fn init(regions: &[MemoryRegion]) -> Result<FrameStats, &'static str> {
    if !crate::kcore::mem::hhdm_ready() {
        return Err("HHDM-Fenster unbekannt — Bitmap nicht beschreibbar");
    }
    if regions.is_empty() {
        return Err("Memory-Map leer");
    }
    let summary = summarize(regions);
    // Nur echter RAM — nicht die MMIO-Loecher im oberen Adressraum.
    let total_frames = frames_for(summary.ram_top);
    if total_frames == 0 {
        return Err("Memory-Map leer");
    }
    if summary.usable_bytes < (64 * PAGE_SIZE) as u64 {
        return Err("zu wenig nutzbarer Speicher fuer den Kernel");
    }
    let words = Bitmap::words_needed(total_frames);
    let bytes = words * 8;
    if bytes > MAX_BITMAP_BYTES {
        return Err("Memory-Map unglaubwuerdig gross fuer die Frame-Bitmap");
    }

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
    // Nicht nutzbare Regionen NACH dem Freigeben nachreservieren. Ueberlappt
    // eine reservierte Region eine nutzbare (zerrissene Map), gewinnt die
    // strengere Aussage. Die Grenzen werden dabei nach aussen ausgerichtet,
    // damit ein angebrochener Frame nicht halb vergeben wird.
    let free_after_usable = bm.free_frames();
    for r in regions {
        if r.kind.is_allocatable() || r.len == 0 {
            continue;
        }
        let s = (r.start / PAGE_SIZE as u64) as usize;
        let e = ((r.end() + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64) as usize;
        if e > s {
            bm.reserve_range(s, e);
        }
    }
    let overlap_frames = free_after_usable - bm.free_frames();

    // Erstes Megabyte nie vergeben (BIOS-Datenbereich, Realmode-IVT, VGA).
    let before_low = bm.free_frames();
    bm.reserve_range(0, (0x10_0000 / PAGE_SIZE) as usize);
    let low_frames = before_low - bm.free_frames();
    // Die Bitmap selbst gehoert nicht in den freien Pool.
    let bstart = (storage_phys / PAGE_SIZE as u64) as usize;
    let bend = bstart + (bytes + PAGE_SIZE - 1) / PAGE_SIZE;
    bm.reserve_range(bstart, bend);

    if bm.free_frames() == 0 {
        return Err("nach Auswertung der Memory-Map ist kein Frame frei");
    }

    // Nachpruefung der Zusage "kein Frame einer nicht nutzbaren Region ist
    // frei". Sie kostet einen Durchlauf ueber die Bitmap und faengt jede
    // kuenftige Umsortierung der Schritte oben ab.
    let mut violations = 0usize;
    for r in regions {
        if r.kind.is_allocatable() || r.len == 0 {
            continue;
        }
        let s = (r.start / PAGE_SIZE as u64) as usize;
        let e = (((r.end() + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64) as usize).min(bm.frames());
        for i in s..e {
            if !bm.is_used(i) {
                violations += 1;
            }
        }
    }
    if violations > 0 {
        return Err("Frame-Bitmap widerspricht der Memory-Map");
    }
    // Die Bitmap muss sich selbst als belegt fuehren, sonst vergibt sie ihren
    // eigenen Speicher weiter.
    if !bm.is_used(bstart) || !bm.is_used(bend - 1) {
        return Err("Frame-Bitmap schuetzt ihren eigenen Speicher nicht");
    }

    let stats = FrameStats {
        total_frames: bm.frames(),
        free_frames: bm.free_frames(),
        bitmap_bytes: bytes,
        bitmap_at: PhysAddr(storage_phys),
        summary,
        overlap_frames,
        low_frames,
    };
    let low_water = bm.free_frames();
    *ALLOCATOR.lock() =
        Some(BitmapFrameAllocator { bm, starved: 0, bad_frees: 0, low_water });
    klog!(
        "mm",
        "Memory-Map  : {} Regionen, {} Frames unter 1 MiB gesperrt, {} Frames wegen Ueberschneidung nachreserviert",
        regions.len(),
        low_frames,
        overlap_frames
    );
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

/// Hoechstzahl der Bruchstuecke, die der Erschoepfungstest zwischenspeichert.
const DRAIN_CHUNKS: usize = 32;

/// Selbsttest des Frame-Allocators im laufenden Kernel.
///
/// Geprueft werden die Zusagen, auf die sich alles andere verlaesst, **und**
/// die Fehlerpfade:
/// 1. gelieferter Frame ist seitenausgerichtet,
/// 2. er verschwindet aus dem freien Pool,
/// 3. er ist vollstaendig genullt,
/// 4. nach der Rueckgabe stimmt der Zaehler wieder,
/// 5. doppelte Rueckgabe wird abgewiesen und gezaehlt,
/// 6. nicht ausgerichtete Rueckgabe wird abgewiesen,
/// 7. Rueckgabe ausserhalb der Bitmap wird abgewiesen,
/// 8. Anforderung groesser als der gesamte Speicher scheitert sauber,
/// 9. zusammenhaengende Anforderung liefert wirklich zusammenhaengende Frames,
/// 10. bei voellig erschoepftem Speicher liefert `alloc_frame` `None` — und
///     nach Rueckgabe aller Bruchstuecke steht der Zaehler exakt wie vorher.
///
/// Rueckgabe: `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    let total = 10usize;
    let mut ok = 0usize;
    let mut g = ALLOCATOR.lock();
    let Some(a) = g.as_mut() else {
        klog!("mm", "Selbsttest Frames: FEHLER — Allocator nicht initialisiert");
        return (0, total);
    };
    let before = a.free();

    // 1..4 — Grundzusagen eines einzelnen Frames.
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

        // 5 — doppelte Rueckgabe darf den Frame NICHT ein zweites Mal in den
        //     Pool legen, sonst vergibt der Allocator ihn spaeter zweimal.
        let bad_before = a.bad_frees();
        a.free_frame(f);
        if a.free() == before && a.bad_frees() == bad_before + 1 {
            ok += 1;
        }
    }

    // 6 — krumme Adresse.
    let bad_before = a.bad_frees();
    a.free_frame(PhysAddr(0x1234));
    if a.bad_frees() == bad_before + 1 && a.free() == before {
        ok += 1;
    }

    // 7 — jenseits der Bitmap.
    let bad_before = a.bad_frees();
    a.free_frame(PhysAddr((a.total() as u64 + 4) * PAGE_SIZE as u64));
    if a.bad_frees() == bad_before + 1 && a.free() == before {
        ok += 1;
    }

    // 8 — Anforderung groesser als der verwaltete Speicher.
    let starved_before = a.starved();
    if a.alloc_contiguous(a.total() + 1).is_err()
        && a.starved() == starved_before + 1
        && a.free() == before
    {
        ok += 1;
    }

    // 9 — zusammenhaengender Block, wirklich zusammenhaengend belegt.
    if let Ok(base) = a.alloc_contiguous(8) {
        let idx = (base.as_u64() / PAGE_SIZE as u64) as usize;
        let contiguous = (idx..idx + 8).all(|i| a.bm.is_used(i));
        let counted = a.free() == before - 8;
        let released = a.free_contiguous(base, 8).is_ok() && a.free() == before;
        if base.is_aligned(PAGE_SIZE as u64) && contiguous && counted && released {
            ok += 1;
        }
    }

    // 10 — vollstaendige Erschoepfung. Die Frames werden in wenigen grossen
    //      Bruchstuecken belegt (ohne Nullen, das kostete hier 400+ MiB
    //      Schreibarbeit), danach muss `alloc_frame` `None` liefern.
    let mut chunks: [(PhysAddr, usize); DRAIN_CHUNKS] = [(PhysAddr(0), 0); DRAIN_CHUNKS];
    let mut n_chunks = 0usize;
    let mut want = a.free();
    while want > 0 && n_chunks < DRAIN_CHUNKS {
        match a.alloc_contiguous(want) {
            Ok(base) => {
                chunks[n_chunks] = (base, want);
                n_chunks += 1;
                want = a.free();
            }
            Err(_) => want /= 2,
        }
    }
    let drained = a.free() == 0;
    let starved_before = a.starved();
    let none_when_empty = a.alloc_frame().is_none() && a.starved() == starved_before + 1;
    for &(base, count) in chunks.iter().take(n_chunks) {
        let _ = a.free_contiguous(base, count);
    }
    if drained && none_when_empty && a.free() == before {
        ok += 1;
    }

    klog!(
        "mm",
        "Selbsttest Frames: {}/{} Zusagen erfuellt, {} frei",
        ok,
        total,
        a.free()
    );
    klog!(
        "mm",
        "  Frame-Fehlerpfade: {} abgewiesene Rueckgaben, {} erschoepfte Anforderungen, Tiefstand {} frei",
        a.bad_frees(),
        a.starved(),
        a.low_water()
    );
    (ok, total)
}
