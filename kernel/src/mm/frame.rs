//! Physischer Frame-Allocator (Bitmap).
//!
//! **Schicht: mm.** Kennt keine Seitentabellen und keine x86-Details — nur
//! "welcher 4-KiB-Frame ist frei". Die eigentliche Bitmap-Logik liegt seit dem
//! 23.08.2026 in **Firn** (`kernel/firn/bitmap.fi`), angebunden über
//! [`crate::mm::firn_bitmap`]; hier steht nur die Auswertung der echten
//! Memory-Map und der globale Zugriff.
//!
//! Die frühere Rust-Fassung (`libs/osum-mem/src/bitmap.rs`) ist ausgebaut. Ihre
//! 23 Testfälle sind nicht verloren gegangen: sie stehen als C-Prüfstand in
//! `tests/firn-bitmap/` und werden gegen **dasselbe** Firn-Objekt gefahren, das
//! im Kernelabbild landet.
//!
//! Was der Wechsel bringt: die Rechnungen sind **geprüft**. `used -= 1` lief in
//! Rusts Release-Bau still um; ab da hätte der Verwalter Milliarden freier
//! Rahmen gemeldet und Speicher vergeben, den es nicht gibt.
//!
//! **Warum Bitmap und nicht Buddy** (ausfuehrlich in ARCHITECTURE.md): 1 Bit je
//! 4 KiB sind 32 KiB Metadaten pro GiB RAM, konstant und ohne Zeiger. Ein
//! Buddy-Allocator braucht Freilisten, die man erst anlegen kann, wenn es einen
//! Heap gibt — der Heap braucht aber Frames. Die Bitmap loest dieses
//! Henne-Ei-Problem, indem sie sich selbst in den erstbesten freien Bereich legt.

use crate::klog;
use crate::mm::firn_bitmap::{AllocError, Bitmap};
use osum_mem::region::{frames_for, summarize, MemoryRegion, MemorySummary};
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

    /// Buchhaltungsinvariante: belegt + frei ergibt genau die verwaltete Menge.
    ///
    /// Sie ist der einzige Weg, ein still verlorenes oder doppelt gezaehltes
    /// Bit ueberhaupt zu bemerken; der Selbsttest rechnet sie nach jedem
    /// Fehlerpfad nach.
    pub fn invariant_holds(&self) -> bool {
        self.used().checked_add(self.free()) == Some(self.total())
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

/// Ergebnis der Vorpruefung einer Memory-Map: so viel muss feststehen, BEVOR
/// irgendein Byte geschrieben wird.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BitmapPlan {
    /// Zahl der zu verwaltenden Frames (aus `ram_top`).
    pub total_frames: usize,
    /// Groesse der Bitmap in 64-Bit-Woertern.
    pub words: usize,
    /// Physische Adresse, an der die Bitmap liegen soll.
    pub storage: u64,
}

/// Prueft eine Memory-Map und legt Groesse und Ort der Bitmap fest —
/// **ohne Nebenwirkung**. Genau deshalb ist diese Funktion mit erfundenen,
/// absichtlich kaputten Karten pruefbar (siehe [`map_selftest`]), waehrend
/// [`init`] selbst nur einmal im Boot laufen kann.
///
/// Abgewiesen werden: leere Liste, `ram_top == 0`, zu wenig nutzbarer
/// Speicher, unglaubwuerdig grosse Bitmap, fehlender Platz fuer die Bitmap und
/// eine Bitmap, die ausserhalb des verwalteten Bereichs zu liegen kaeme.
pub fn plan_bitmap(regions: &[MemoryRegion]) -> Result<BitmapPlan, &'static str> {
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
    let Some(storage) = storage else {
        return Err("kein zusammenhaengender Platz fuer die Frame-Bitmap");
    };

    // Die Bitmap muss sich selbst als belegt fuehren koennen: liegt ihr
    // Speicher jenseits von `ram_top`, wuerde sie ihre eigenen Frames spaeter
    // weitervergeben.
    let storage_end = PhysAddr(storage)
        .checked_add(bytes as u64)
        .and_then(|e| e.checked_align_up(PAGE_SIZE as u64))
        .ok_or("Platz fuer die Frame-Bitmap laeuft am Adressraumende ueber")?;
    if (storage_end.as_u64() / PAGE_SIZE as u64) as usize > total_frames {
        return Err("Platz fuer die Frame-Bitmap liegt ausserhalb des verwalteten Bereichs");
    }
    Ok(BitmapPlan { total_frames, words, storage })
}

/// Traegt eine Memory-Map in eine (anfangs vollstaendig belegte) Bitmap ein.
///
/// Reihenfolge ist hier die ganze Aussage: erst werden die nutzbaren Regionen
/// freigegeben, DANACH wird jede nicht nutzbare Region nach aussen ausgerichtet
/// nachreserviert. Ueberlappen sich beide (zerrissene Map), gewinnt die
/// strengere Aussage; ein angebrochener Frame wird nie halb vergeben.
///
/// Rueckgabe: Zahl der Frames, die wegen einer solchen Ueberschneidung wieder
/// gesperrt wurden.
fn apply_map(bm: &mut Bitmap<'_>, regions: &[MemoryRegion]) -> usize {
    for r in regions {
        if !r.kind.is_allocatable() {
            continue;
        }
        if let Some((s, e)) = r.page_aligned() {
            bm.free_range((s / PAGE_SIZE as u64) as usize, (e / PAGE_SIZE as u64) as usize);
        }
    }
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
    free_after_usable - bm.free_frames()
}

/// Zaehlt Frames, die laut Memory-Map NICHT vergeben werden duerfen, in der
/// Bitmap aber frei sind. Muss immer 0 sein — sonst widersprechen sich Bitmap
/// und Bootloaderangaben.
fn map_violations(bm: &Bitmap<'_>, regions: &[MemoryRegion]) -> usize {
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
    violations
}

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
    // Ein zweiter Aufruf wuerde eine frische, "alles frei"-Bitmap ueber die
    // laufende legen: jeder bereits vergebene Frame waere danach erneut
    // vergebbar. Das ist der teuerste denkbare Fehler in dieser Datei und
    // deshalb ausdruecklich abgefangen statt nur dokumentiert.
    if ALLOCATOR.lock().is_some() {
        return Err("Frame-Allocator ist bereits eingerichtet");
    }
    let summary = summarize(regions);
    // Groesse und Ort der Bitmap festlegen — nebenwirkungsfrei und mit
    // denselben Pruefungen, die [`map_selftest`] an erfundenen Karten vorfuehrt.
    let BitmapPlan { total_frames, words, storage: storage_phys } = plan_bitmap(regions)?;
    let bytes = words * 8;

    // Der Zugriff laeuft ueber das Direct-Map-Fenster — gepruefte Umrechnung,
    // damit ein absurder HHDM-Offset hier auffaellt und nicht beim ersten
    // Schreibzugriff als Dreifachfehler.
    let bitmap_virt = crate::kcore::mem::phys_to_virt_checked(PhysAddr(storage_phys))
        .ok_or("Bitmap-Adresse laesst sich nicht ins HHDM-Fenster umrechnen")?;
    if !crate::kcore::mem::hhdm_contains(bitmap_virt, summary.mapped_top.max(summary.ram_top)) {
        return Err("Bitmap liegt ausserhalb des vom Bootloader abgebildeten Fensters");
    }
    let bits: &'static mut [u64] =
        unsafe { core::slice::from_raw_parts_mut(bitmap_virt.as_ptr::<u64>(), words) };
    let mut bm = Bitmap::new_all_used(bits, total_frames);

    // Nutzbare Regionen freigeben, nicht nutzbare danach nachreservieren.
    let overlap_frames = apply_map(&mut bm, regions);

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
    if map_violations(&bm, regions) > 0 {
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

/// Sperrt den Frame-Allocator **nur, wenn er gerade frei ist**.
///
/// Fuer Aufrufer, die mitten in einem anderen Vorgang stecken und nicht warten
/// duerfen — allen voran das Nachwachsen des Heaps ([`crate::mm::heap::grow`]):
/// dort haelt der Aufrufer bereits die Heap-Sperre. Waere die Frame-Sperre
/// genau dann von einem Kontrollfluss gehalten, der seinerseits Heap-Speicher
/// anfordert, wuerde ein blockierendes [`lock`] den Kernel stehen lassen.
/// `None` heisst schlicht: jetzt nicht — der Aufrufer meldet einen sauberen
/// Misserfolg, statt zu haengen.
pub fn try_lock() -> Option<MutexGuard<'static, Option<BitmapFrameAllocator>>> {
    ALLOCATOR.try_lock()
}

/// Freie Frames (0, solange nicht initialisiert).
pub fn free_frames() -> usize {
    ALLOCATOR.lock().as_ref().map_or(0, |a| a.free())
}

/// Belegte Frames.
pub fn used_frames() -> usize {
    ALLOCATOR.lock().as_ref().map_or(0, |a| a.used())
}

/// Selbsttest der Memory-Map-Auswertung mit **erfundenen** Karten.
///
/// [`init`] laeuft im Boot genau einmal und bekommt genau eine (gesunde)
/// Karte zu sehen — seine Fehlerpfade waeren damit reine Behauptung. Hier
/// werden sie an nebenwirkungsfreien Kopien der Logik ([`plan_bitmap`],
/// [`apply_map`], [`map_violations`]) wirklich durchlaufen:
///
/// 1. leere Regionsliste wird abgewiesen,
/// 2. Karte ohne echten RAM (nur MMIO) wird abgewiesen,
/// 3. Karte mit zu wenig nutzbarem Speicher wird abgewiesen,
/// 4. Karte mit unglaubwuerdiger Obergrenze (Bitmap > 8 MiB) wird abgewiesen,
/// 5. gesunde Karte ergibt einen plausiblen Plan (Bitmap oberhalb 1 MiB, in
///    einer nutzbaren Region, gross genug fuer alle Frames),
/// 6. **zerrissene Karte**: eine reservierte Region liegt mitten in einer als
///    nutzbar gemeldeten. Nach [`apply_map`] muss jeder ihrer Frames belegt
///    sein, kein Frame darf der Memory-Map widersprechen, und die Zahl der
///    freien Frames muss exakt der Erwartung entsprechen,
/// 7. **unsortierte, ueberlappende Karte**: die Firmware liefert die Regionen
///    in beliebiger Reihenfolge, und eine reservierte Region ueberlappt eine
///    als nutzbar gemeldete. Kein Frame der Ueberschneidung darf vergeben
///    werden,
/// 8. **nicht ausgerichtete Raender**: nutzbar wird nach INNEN gerundet,
///    reserviert nach AUSSEN. Ein angebrochener Frame wird nie halb vergeben.
///
/// Die Faelle 7 und 8 standen bis zum 23.08.2026 als Host-Tests in
/// `libs/osum-mem/src/lib.rs`. Mit dem Umzug der Bitmap nach Firn sind sie
/// hierher gewandert — und laufen damit in jedem Boot gegen das echte Objekt
/// statt gegen eine Host-Kopie.
///
/// Rueckgabe: `(bestanden, gesamt)`.
pub fn map_selftest() -> (usize, usize) {
    use osum_mem::region::RegionKind::{BadMemory, Reserved, Usable};
    let total = 8usize;
    let mut ok = 0usize;
    const MIB: u64 = 1 << 20;

    // 1 — leere Liste.
    if plan_bitmap(&[]).is_err() {
        ok += 1;
    }
    // 2 — kein echter RAM, nur ein MMIO-Loch weit oben.
    let mmio_only = [MemoryRegion::new(0xf000_0000, 16 * MIB, Reserved)];
    if plan_bitmap(&mmio_only).is_err() {
        ok += 1;
    }
    // 3 — nutzbarer Speicher unterhalb der Mindestmenge (64 Seiten).
    let tiny = [
        MemoryRegion::new(0, MIB, Reserved),
        MemoryRegion::new(MIB, 16 * PAGE_SIZE as u64, Usable),
    ];
    if plan_bitmap(&tiny).is_err() {
        ok += 1;
    }
    // 4 — Muell in der Obergrenze: 256 GiB RAM braeuchten mehr als 8 MiB Bitmap.
    let absurd = [
        MemoryRegion::new(MIB, 64 * MIB, Usable),
        MemoryRegion::new(1 << 48, 4 * MIB, Usable),
    ];
    if plan_bitmap(&absurd).is_err() {
        ok += 1;
    }
    // 5 — gesunde Karte: 16 MiB RAM, das erste Megabyte reserviert.
    let healthy = [
        MemoryRegion::new(0, MIB, Reserved),
        MemoryRegion::new(MIB, 15 * MIB, Usable),
    ];
    match plan_bitmap(&healthy) {
        Ok(p) => {
            let frames = (16 * MIB / PAGE_SIZE as u64) as usize;
            if p.total_frames == frames
                && p.words == Bitmap::words_needed(frames)
                && p.storage >= MIB
                && p.storage + (p.words * 8) as u64 <= 16 * MIB
            {
                ok += 1;
            }
        }
        Err(_) => {}
    }

    // 6 — zerrissene Karte auf einer echten, aber nur 16 MiB grossen Bitmap
    //     im eigenen Stapelrahmen (64 Woerter = 4096 Frames).
    let torn = [
        MemoryRegion::new(0, 16 * MIB, Usable),
        // mitten in der nutzbaren Region: 4 MiB .. 5 MiB gehoeren der Firmware
        MemoryRegion::new(4 * MIB, MIB, Reserved),
        // krumme Grenzen: der angebrochene Frame muss ganz gesperrt werden
        MemoryRegion::new(8 * MIB + 100, 8, Reserved),
    ];
    let mut scratch = [0u64; 64];
    let frames = (16 * MIB / PAGE_SIZE as u64) as usize;
    let mut bm = Bitmap::new_all_used(&mut scratch, frames);
    let overlap = apply_map(&mut bm, &torn);
    let reserved_start = (4 * MIB / PAGE_SIZE as u64) as usize;
    let reserved_end = (5 * MIB / PAGE_SIZE as u64) as usize;
    let all_reserved = (reserved_start..reserved_end).all(|i| bm.is_used(i));
    let odd_frame = ((8 * MIB) / PAGE_SIZE as u64) as usize;
    let expected_free = frames - (reserved_end - reserved_start) - 1;
    if all_reserved
        && bm.is_used(odd_frame)
        && map_violations(&bm, &torn) == 0
        && overlap == (reserved_end - reserved_start) + 1
        && bm.free_frames() == expected_free
    {
        ok += 1;
    }

    // 7 — unsortiert UND ueberlappend: eine reservierte Region mitten in einer
    //     nutzbaren, und die Liste kommt nicht in Adressreihenfolge. Genau so
    //     liefert echte Firmware sie gelegentlich.
    let unsortiert = [
        MemoryRegion::new(0x8000, 0x2000, Reserved),
        MemoryRegion::new(0x0, 0x1_0000, Usable),
        MemoryRegion::new(0x1_0000, 0x1000, BadMemory),
    ];
    let mut s7 = [0u64; 4];
    let f7 = (0x1_1000u64 / PAGE_SIZE as u64) as usize;
    let mut bm7 = Bitmap::new_all_used(&mut s7, f7);
    apply_map(&mut bm7, &unsortiert);
    let erwartet7 = ((0x1_0000 - 0x2000) / PAGE_SIZE as u64) as usize;
    let mut ueberschuss7 = false;
    for i in 0..f7 {
        let addr = i as u64 * PAGE_SIZE as u64;
        if !bm7.is_used(i) && ((0x8000..0xa000).contains(&addr) || addr >= 0x1_0000) {
            ueberschuss7 = true;
        }
    }
    if bm7.free_frames() == erwartet7 && !ueberschuss7 && map_violations(&bm7, &unsortiert) == 0 {
        ok += 1;
    }

    // 8 — krumme Raender: nutzbar nach innen, reserviert nach aussen gerundet.
    let krumm = [
        MemoryRegion::new(0x1001, 0x2fff, Usable), // nutzbar erst ab 0x2000
        MemoryRegion::new(0x4000, 0x1000, Usable),
        MemoryRegion::new(0x5800, 0x800, Reserved), // frisst Frame 5 ganz
    ];
    let mut s8 = [0u64; 2];
    let mut bm8 = Bitmap::new_all_used(&mut s8, 6);
    apply_map(&mut bm8, &krumm);
    if bm8.is_used(0)
        && bm8.is_used(1)
        && !bm8.is_used(2)
        && !bm8.is_used(3)
        && !bm8.is_used(4)
        && bm8.is_used(5)
        && bm8.free_frames() == 3
    {
        ok += 1;
    }

    klog!(
        "mm",
        "Selbsttest Memory-Map: {}/{} Zusagen erfuellt (4 kaputte Karten abgewiesen, zerrissene Karte: {} Frames nachreserviert, unsortiert+ueberlappend und krumme Raender geprueft)",
        ok,
        total,
        overlap
    );
    (ok, total)
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
///     nach Rueckgabe aller Bruchstuecke steht der Zaehler exakt wie vorher,
/// 11. Anforderungen und Rueckgaben der Laenge 0 werden abgewiesen und
///     gezaehlt, statt einen Zeiger auf nichts zu liefern,
/// 12. die Buchhaltungsinvariante `belegt + frei == verwaltet` gilt nach jedem
///     der obigen Faelle — auch nach den Fehlerpfaden.
///
/// Vorangestellt laufen [`crate::kcore::mem::selftest`] (die Adressrechnung,
/// auf der jede Zeile hier steht) und [`map_selftest`]; ihre Faelle zaehlen in
/// dasselbe Ergebnis, weil sie dieselbe Zusage pruefen: kein Frame wird
/// vergeben, der nicht vergeben werden darf.
///
/// Rueckgabe: `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    // Zuerst die reine Adressarithmetik der core-Schicht (kein Speicherzugriff).
    let (addr_ok, addr_total) = crate::kcore::mem::selftest();
    klog!(
        "mm",
        "Selbsttest Adressrechnung: {}/{} Zusagen erfuellt (Ausrichtung, Ueberlauf, Kanonizitaet, HHDM-Fenster, Rechte, Fehlertexte)",
        addr_ok,
        addr_total
    );
    // Dann die Auswertung erfundener Memory-Maps (ohne Sperre, ohne Hardware),
    // zuletzt der laufende Allocator.
    let (map_ok, map_total) = map_selftest();
    let total = 12usize + map_total + addr_total;
    let mut ok = map_ok + addr_ok;
    let mut g = ALLOCATOR.lock();
    let Some(a) = g.as_mut() else {
        klog!("mm", "Selbsttest Frames: FEHLER — Allocator nicht initialisiert");
        return (0, total);
    };
    let before = a.free();
    // Fuer Fall 12: die Invariante wird nach jedem Abschnitt nachgerechnet,
    // nicht nur einmal am Ende — sonst koennten sich zwei Fehler aufheben.
    let mut invariant = a.invariant_holds();

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

    invariant &= a.invariant_holds();

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

    invariant &= a.invariant_holds();

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
    invariant &= a.invariant_holds();

    // 11 — Laenge 0. Weder darf ein "Block" ohne Frames entstehen (der
    //      Aufrufer haette einen Zeiger auf fremden Speicher), noch darf eine
    //      Rueckgabe der Laenge 0 die Buchhaltung anfassen.
    let starved_before = a.starved();
    let bad_before = a.bad_frees();
    let zero_alloc = a.alloc_contiguous(0).is_err() && a.starved() == starved_before + 1;
    let zero_free = a.free_contiguous(PhysAddr(0x20_0000), 0).is_err()
        && a.bad_frees() == bad_before + 1;
    if zero_alloc && zero_free && a.free() == before {
        ok += 1;
    }
    invariant &= a.invariant_holds();

    // 12 — Buchhaltung ueber alle Faelle hinweg schluessig.
    if invariant && a.free() == before {
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
    klog!(
        "mm",
        "  Frame-Buchhaltung: {} belegt + {} frei = {} verwaltet, stimmig: {}",
        a.used(),
        a.free(),
        a.total(),
        if invariant && a.invariant_holds() { "ja" } else { "nein" }
    );
    (ok, total)
}
