//! Eigener Heap-Allocator von Karstos — **keine externe Crate**.
//!
//! Bewusst selbst geschrieben (Kernziel *lightweight*): ein First-Fit-Free-List-
//! Allocator mit Adress-Sortierung und sofortiger Verschmelzung benachbarter
//! Loecher. Rund 150 Zeilen statt einer Fremdabhaengigkeit — und weil hier keine
//! Hardware angefasst wird, laeuft die komplette Logik in den Host-Tests unten
//! (`cargo test -p karst-mem --target x86_64-unknown-linux-gnu`), also **bevor**
//! sie jemals im Kernel Speicher vergibt.
//!
//! Layout eines belegten Blocks:
//! ```text
//! [ ggf. Ausricht-Fueller ][ AllocHeader (16 B) ][ Nutzdaten ... ]
//! ^ block_start                                  ^ Rueckgabezeiger
//! ```
//! Der Header speichert die *tatsaechliche* Blockgroesse und den Abstand zum
//! Blockanfang. Dadurch ist `dealloc` unabhaengig vom uebergebenen `Layout`
//! korrekt — auch bei uebergrossen Ausrichtungen, bei denen der Block vorne
//! Fueller enthaelt.

use core::alloc::Layout;
use core::ptr;

/// Grundausrichtung aller Bloecke. 16 Byte deckt `u128`/SSE-Typen ab.
pub const ALIGN: usize = 16;
/// Groesse des Verwaltungskopfes eines belegten Blocks.
pub const HEADER: usize = core::mem::size_of::<AllocHeader>();
/// Kleinster Block, der noch als freies Loch verwaltet werden kann
/// (muss den `FreeBlock`-Kopf aufnehmen und danach noch nutzbar sein).
pub const MIN_BLOCK: usize = 32;

/// Rundet `v` auf ein Vielfaches von `a` (2^n) auf und meldet einen Ueberlauf,
/// statt still auf 0 umzulaufen. Der Heap rechnet ausschliesslich damit — jede
/// der drei Stellen verarbeitet Zahlen, die von aussen kommen (Layout-Groesse,
/// Layout-Ausrichtung, Regionsgrenzen).
#[inline]
const fn align_up_checked(v: usize, a: usize) -> Option<usize> {
    match v.checked_add(a - 1) {
        Some(x) => Some(x & !(a - 1)),
        None => None,
    }
}

/// Kopf eines freien Loches — liegt IM Loch, kostet also keinen Extraspeicher.
#[repr(C)]
struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}

/// Kopf eines belegten Blocks, direkt vor den Nutzdaten.
#[repr(C)]
pub struct AllocHeader {
    /// Gesamtgroesse des Blocks inklusive Fueller und Header.
    size: usize,
    /// Abstand vom Blockanfang bis zum Nutzdatenzeiger.
    offset: usize,
}

/// Konsistente Momentaufnahme des Heapzustands (siehe [`Heap::stats`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct HeapStats {
    /// Gesamtgroesse in Bytes.
    pub size: usize,
    /// Belegte Bytes (inkl. Verwaltungskoepfe).
    pub used: usize,
    /// Freie Bytes.
    pub free: usize,
    /// Anzahl freier Loecher (Fragmentierungsmass).
    pub holes: usize,
    /// Groesstes zusammenhaengendes freies Loch in Bytes.
    pub largest_hole: usize,
}

/// Der Heap. Nicht selbst gesperrt — der Kernel legt ihn in einen Spinlock.
pub struct Heap {
    head: *mut FreeBlock,
    size: usize,
    used: usize,
}

// Sicherheit: `Heap` enthaelt nur Rohzeiger auf Speicher, den er selbst
// verwaltet. Der Zugriff wird ausserhalb durch einen Mutex serialisiert.
unsafe impl Send for Heap {}

impl Heap {
    /// Leerer Heap ohne Speicher — als `static` initialisierbar.
    pub const fn empty() -> Self {
        Heap { head: ptr::null_mut(), size: 0, used: 0 }
    }

    /// Fuegt dem Heap einen weiteren Speicherbereich hinzu.
    ///
    /// # Safety
    /// `start..start+size` muss gueltiger, beschreibbarer, exklusiv dem Heap
    /// gehoerender Speicher sein und fuer die Lebensdauer des Heaps gelten.
    pub unsafe fn add_region(&mut self, start: *mut u8, size: usize) {
        // Ueberlaufsicher: `start` kommt aus dem Frame-Allocator, `size` aus
        // der Memory-Map. Rechnet eine der beiden Zahlen um, entstuende ein
        // Loch mit absurder Groesse, aus dem der Heap Speicher vergaebe, den
        // es nicht gibt. Im Zweifel wird die Region verworfen.
        let Some(s) = align_up_checked(start as usize, ALIGN) else {
            return;
        };
        let Some(e) = (start as usize).checked_add(size).map(|v| v & !(ALIGN - 1)) else {
            return;
        };
        if e <= s || e - s < MIN_BLOCK {
            return;
        }
        self.size += e - s;
        unsafe { self.insert_free(s as *mut FreeBlock, e - s) };
    }

    /// Belegter Speicher in Bytes (inkl. Verwaltungskoepfe).
    #[inline]
    pub fn used(&self) -> usize {
        self.used
    }
    /// Gesamtgroesse des Heaps in Bytes.
    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }
    /// Freier Speicher in Bytes.
    #[inline]
    pub fn free(&self) -> usize {
        self.size - self.used
    }

    /// Reserviert Speicher. Gibt `null` zurueck, wenn nichts passt.
    ///
    /// # Safety
    /// Nur mit gueltigem `Layout` aufrufen.
    pub unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let align = if layout.align() > ALIGN { layout.align() } else { ALIGN };
        // Ueberlaufsicher aufrunden: eine Groesse dicht unter `usize::MAX`
        // wuerde beim Aufrunden auf 0 umlaufen und dann JEDEN freien Block als
        // passend erscheinen lassen — der Ruecklauf waere ein Zeiger auf
        // fremden Speicher. Solche Anfragen scheitern hier sofort.
        let want = if layout.size() == 0 { 1 } else { layout.size() };
        let Some(need) = align_up_checked(want, ALIGN) else {
            return ptr::null_mut();
        };

        let mut prev: *mut *mut FreeBlock = &mut self.head;
        let mut cur = self.head;
        while !cur.is_null() {
            let bstart = cur as usize;
            let bsize = unsafe { (*cur).size };
            let bend = bstart + bsize;
            // Auch die Ausrichtung wird ueberlaufsicher gerechnet: eine absurd
            // grosse Ausrichtung darf nur zum Fehlschlag fuehren, nie zu einem
            // umgelaufenen Zeiger.
            let payload = match align_up_checked(bstart + HEADER, align) {
                Some(v) => v,
                None => {
                    prev = unsafe { &mut (*cur).next };
                    cur = unsafe { (*cur).next };
                    continue;
                }
            };

            if payload.checked_add(need).map_or(false, |end| end <= bend) {
                let rest = bend - (payload + need);
                let split = rest >= MIN_BLOCK;
                let total = if split { payload + need - bstart } else { bsize };

                // Block aus der Liste haengen.
                unsafe { *prev = (*cur).next };
                if split {
                    unsafe { self.insert_free((bstart + total) as *mut FreeBlock, rest) };
                }

                let hdr = (payload - HEADER) as *mut AllocHeader;
                unsafe {
                    (*hdr).size = total;
                    (*hdr).offset = payload - bstart;
                }
                self.used += total;
                return payload as *mut u8;
            }

            prev = unsafe { &mut (*cur).next };
            cur = unsafe { (*cur).next };
        }
        ptr::null_mut()
    }

    /// Gibt einen zuvor von [`Heap::alloc`] gelieferten Zeiger frei.
    ///
    /// # Safety
    /// `p` muss von genau diesem Heap stammen und darf nur einmal freigegeben werden.
    pub unsafe fn dealloc(&mut self, p: *mut u8, _layout: Layout) {
        if p.is_null() {
            return;
        }
        let hdr = (p as usize - HEADER) as *mut AllocHeader;
        let (size, offset) = unsafe { ((*hdr).size, (*hdr).offset) };
        let bstart = p as usize - offset;
        self.used -= size;
        unsafe { self.insert_free(bstart as *mut FreeBlock, size) };
    }

    /// Haengt ein Loch adress-sortiert ein und verschmilzt es mit den Nachbarn.
    unsafe fn insert_free(&mut self, block: *mut FreeBlock, size: usize) {
        let baddr = block as usize;
        let mut prev: *mut FreeBlock = ptr::null_mut();
        let mut cur = self.head;
        while !cur.is_null() && (cur as usize) < baddr {
            prev = cur;
            cur = unsafe { (*cur).next };
        }
        unsafe {
            (*block).size = size;
            (*block).next = cur;
        }
        if prev.is_null() {
            self.head = block;
        } else {
            unsafe { (*prev).next = block };
        }
        // Mit dem Nachfolger verschmelzen.
        if !cur.is_null() && baddr + size == cur as usize {
            unsafe {
                (*block).size += (*cur).size;
                (*block).next = (*cur).next;
            }
        }
        // Mit dem Vorgaenger verschmelzen.
        if !prev.is_null() {
            let pend = prev as usize + unsafe { (*prev).size };
            if pend == baddr {
                unsafe {
                    (*prev).size += (*block).size;
                    (*prev).next = (*block).next;
                }
            }
        }
    }

    /// Momentaufnahme aller Kennzahlen in einem Rutsch — fuer Diagnosemeldungen
    /// (z. B. den OOM-Bericht des Kernels), die den Heap nur einmal sperren
    /// duerfen und dabei einen in sich konsistenten Satz Zahlen brauchen.
    pub fn stats(&self) -> HeapStats {
        let mut holes = 0usize;
        let mut largest = 0usize;
        let mut cur = self.head;
        while !cur.is_null() {
            let s = unsafe { (*cur).size };
            holes += 1;
            if s > largest {
                largest = s;
            }
            cur = unsafe { (*cur).next };
        }
        HeapStats { size: self.size, used: self.used, free: self.size - self.used, holes, largest_hole: largest }
    }

    /// Anzahl freier Loecher — nur fuer Tests/Diagnose.
    pub fn hole_count(&self) -> usize {
        let mut n = 0;
        let mut cur = self.head;
        while !cur.is_null() {
            n += 1;
            cur = unsafe { (*cur).next };
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testrand::Lcg;
    use std::alloc::{alloc as sys_alloc, Layout as SysLayout};

    /// Legt einen 64-KiB-Puffer an und baut daraus einen Heap.
    fn heap_with(bytes: usize) -> (Heap, *mut u8) {
        let l = SysLayout::from_size_align(bytes, 4096).unwrap();
        let p = unsafe { sys_alloc(l) };
        let mut h = Heap::empty();
        unsafe { h.add_region(p, bytes) };
        (h, p)
    }

    #[test]
    fn alloc_free_roundtrip_restores_space() {
        let (mut h, _) = heap_with(64 * 1024);
        let before = h.free();
        let l = Layout::from_size_align(100, 8).unwrap();
        let p = unsafe { h.alloc(l) };
        assert!(!p.is_null());
        assert!(h.free() < before);
        unsafe { h.dealloc(p, l) };
        assert_eq!(h.free(), before);
        assert_eq!(h.hole_count(), 1, "Loecher muessen wieder verschmelzen");
    }

    #[test]
    fn respects_large_alignment() {
        let (mut h, _) = heap_with(64 * 1024);
        for a in [16usize, 32, 64, 256, 4096] {
            let l = Layout::from_size_align(64, a).unwrap();
            let p = unsafe { h.alloc(l) };
            assert!(!p.is_null(), "keine Allokation mit align {a}");
            assert_eq!(p as usize % a, 0, "Ausrichtung {a} verletzt");
            unsafe { h.dealloc(p, l) };
        }
    }

    #[test]
    fn many_allocations_do_not_overlap() {
        let (mut h, _) = heap_with(64 * 1024);
        let l = Layout::from_size_align(48, 8).unwrap();
        let mut ptrs = std::vec::Vec::new();
        for _ in 0..200 {
            let p = unsafe { h.alloc(l) };
            assert!(!p.is_null());
            ptrs.push(p as usize);
        }
        ptrs.sort();
        for w in ptrs.windows(2) {
            assert!(w[1] - w[0] >= 48, "Bloecke ueberlappen: {:#x} {:#x}", w[0], w[1]);
        }
        for p in ptrs {
            unsafe { h.dealloc(p as *mut u8, l) };
        }
        assert_eq!(h.used(), 0);
        assert_eq!(h.hole_count(), 1);
    }

    #[test]
    fn writes_do_not_corrupt_neighbours() {
        let (mut h, _) = heap_with(64 * 1024);
        let l = Layout::from_size_align(64, 8).unwrap();
        let ps: std::vec::Vec<*mut u8> = (0..50).map(|_| unsafe { h.alloc(l) }).collect();
        for (i, p) in ps.iter().enumerate() {
            unsafe { ptr::write_bytes(*p, i as u8, 64) };
        }
        for (i, p) in ps.iter().enumerate() {
            for k in 0..64 {
                assert_eq!(unsafe { *p.add(k) }, i as u8, "Block {i} wurde ueberschrieben");
            }
        }
    }

    #[test]
    fn exhaustion_returns_null_and_recovers() {
        let (mut h, _) = heap_with(8192);
        let l = Layout::from_size_align(512, 8).unwrap();
        let mut ps = std::vec::Vec::new();
        loop {
            let p = unsafe { h.alloc(l) };
            if p.is_null() {
                break;
            }
            ps.push(p);
        }
        assert!(!ps.is_empty());
        assert!(unsafe { h.alloc(l) }.is_null());
        let n = ps.len();
        for p in ps {
            unsafe { h.dealloc(p, l) };
        }
        assert_eq!(h.used(), 0);
        // Nach dem Verschmelzen muss wieder mindestens genauso viel passen.
        let mut again = 0;
        loop {
            if unsafe { h.alloc(l) }.is_null() {
                break;
            }
            again += 1;
        }
        assert!(again >= n, "Fragmentierung nach free: {again} < {n}");
    }

    #[test]
    fn zero_sized_allocation_is_unique_and_freeable() {
        let (mut h, _) = heap_with(4096);
        let l = Layout::from_size_align(0, 1).unwrap();
        let a = unsafe { h.alloc(l) };
        let b = unsafe { h.alloc(l) };
        assert!(!a.is_null() && !b.is_null() && a != b);
        unsafe { h.dealloc(a, l) };
        unsafe { h.dealloc(b, l) };
        assert_eq!(h.used(), 0);
    }

    // ------------------------------------------------------- Zustand & Statistik

    #[test]
    fn empty_heap_serves_nothing() {
        let mut h = Heap::empty();
        assert_eq!(h.size(), 0);
        assert_eq!(h.used(), 0);
        assert_eq!(h.free(), 0);
        assert_eq!(h.hole_count(), 0);
        assert_eq!(h.stats(), HeapStats::default());
        let l = Layout::from_size_align(8, 8).unwrap();
        assert!(unsafe { h.alloc(l) }.is_null(), "leerer Heap darf nichts vergeben");
        // dealloc(null) muss folgenlos bleiben (Aufraeumpfade rufen das auf).
        unsafe { h.dealloc(ptr::null_mut(), l) };
        assert_eq!(h.used(), 0);
    }

    #[test]
    fn stats_are_internally_consistent() {
        let (mut h, _) = heap_with(64 * 1024);
        let s = h.stats();
        assert_eq!(s.size, h.size());
        assert_eq!(s.used + s.free, s.size);
        assert_eq!(s.holes, 1);
        assert_eq!(s.largest_hole, s.free);

        let l = Layout::from_size_align(1000, 16).unwrap();
        let a = unsafe { h.alloc(l) };
        let b = unsafe { h.alloc(l) };
        let c = unsafe { h.alloc(l) };
        unsafe { h.dealloc(b, l) }; // Loch in der Mitte -> 2 Loecher
        let s = h.stats();
        assert_eq!(s.holes, h.hole_count());
        assert_eq!(s.holes, 2, "mittleres Loch muss getrennt bleiben");
        assert_eq!(s.used + s.free, s.size);
        assert_eq!(s.used, h.used());
        assert!(s.largest_hole <= s.free);
        assert!(s.largest_hole >= 1000);

        unsafe { h.dealloc(a, l) };
        unsafe { h.dealloc(c, l) };
        let s = h.stats();
        assert_eq!(s.used, 0);
        assert_eq!(s.holes, 1, "am Ende muss alles wieder ein Loch sein");
        assert_eq!(s.largest_hole, s.size);
    }

    // ------------------------------------------------------------ add_region

    #[test]
    fn several_regions_are_all_usable() {
        let l4k = SysLayout::from_size_align(4096, 4096).unwrap();
        let p1 = unsafe { sys_alloc(l4k) };
        let p2 = unsafe { sys_alloc(l4k) };
        let mut h = Heap::empty();
        unsafe { h.add_region(p1, 4096) };
        unsafe { h.add_region(p2, 4096) };
        assert_eq!(h.size(), 8192);
        assert_eq!(h.hole_count(), 2, "getrennte Regionen duerfen nicht verschmelzen");

        // Beide Regionen muessen wirklich Speicher hergeben.
        let l = Layout::from_size_align(2048, 16).unwrap();
        let mut got = std::vec::Vec::new();
        loop {
            let p = unsafe { h.alloc(l) };
            if p.is_null() {
                break;
            }
            got.push(p);
        }
        assert!(got.len() >= 2, "nur {} Bloecke aus 2 Regionen", got.len());
        let in1 = got.iter().filter(|p| (**p as usize) < p2 as usize).count();
        assert!(in1 > 0 && in1 < got.len() || p2 < p1, "eine Region blieb ungenutzt");
        for p in got {
            unsafe { h.dealloc(p, l) };
        }
        assert_eq!(h.used(), 0);
        assert_eq!(h.hole_count(), 2);
    }

    #[test]
    fn useless_regions_are_ignored() {
        let l = SysLayout::from_size_align(4096, 4096).unwrap();
        let p = unsafe { sys_alloc(l) };
        let mut h = Heap::empty();
        unsafe { h.add_region(p, 0) };
        unsafe { h.add_region(p, MIN_BLOCK - 1) };
        unsafe { h.add_region(p.add(1), 8) };
        assert_eq!(h.size(), 0, "zu kleine Regionen duerfen nicht gezaehlt werden");
        assert_eq!(h.hole_count(), 0);
        assert!(unsafe { h.alloc(Layout::from_size_align(1, 1).unwrap()) }.is_null());
    }

    #[test]
    fn unaligned_region_is_trimmed_but_stays_correct() {
        let l = SysLayout::from_size_align(4096, 4096).unwrap();
        let p = unsafe { sys_alloc(l) };
        let mut h = Heap::empty();
        // Start um 1 Byte versetzt, Ende um 3 Byte zu frueh.
        unsafe { h.add_region(p.add(1), 4096 - 4) };
        assert!(h.size() <= 4096 - 4);
        assert_eq!(h.size() % ALIGN, 0, "Heapgroesse muss ausgerichtet sein");
        let la = Layout::from_size_align(64, 64).unwrap();
        let q = unsafe { h.alloc(la) };
        assert!(!q.is_null());
        assert_eq!(q as usize % 64, 0);
        assert!(q as usize >= p as usize && (q as usize) + 64 <= p as usize + 4096);
        unsafe { h.dealloc(q, la) };
        assert_eq!(h.used(), 0);
    }

    // ------------------------------------------------------------- Fehlerpfade

    #[test]
    fn oversized_requests_fail_without_overflowing() {
        let (mut h, _) = heap_with(8192);
        let too_big = Layout::from_size_align(1 << 30, 16).unwrap();
        assert!(unsafe { h.alloc(too_big) }.is_null());
        // Ausrichtung groesser als der ganze Heap: darf nur scheitern, nie rechnen-
        // ueberlaufen (die `checked_add`-Pruefung in `alloc`).
        let wild = Layout::from_size_align(64, 1 << 20).unwrap();
        assert!(unsafe { h.alloc(wild) }.is_null());
        assert_eq!(h.used(), 0);
        assert_eq!(h.free(), h.size(), "Fehlversuche duerfen nichts belegen");
        // Danach muss der Heap weiter normal arbeiten.
        let ok = Layout::from_size_align(64, 16).unwrap();
        let q = unsafe { h.alloc(ok) };
        assert!(!q.is_null());
        unsafe { h.dealloc(q, ok) };
        assert_eq!(h.hole_count(), 1);
    }

    #[test]
    fn exact_fit_leaves_no_hole() {
        // Ein Block, der den Rest bis unter MIN_BLOCK aufbraucht, wird NICHT
        // gesplittet — sonst entstuenden unbrauchbare Splitter.
        let (mut h, _) = heap_with(4096);
        let all = h.free();
        let l = Layout::from_size_align(all - HEADER, 16).unwrap();
        let p = unsafe { h.alloc(l) };
        assert!(!p.is_null(), "exakt passende Allokation muss gelingen");
        assert_eq!(h.hole_count(), 0);
        assert_eq!(h.free(), 0);
        assert_eq!(h.used(), all);
        unsafe { h.dealloc(p, l) };
        assert_eq!(h.free(), all);
        assert_eq!(h.hole_count(), 1);
    }

    #[test]
    fn free_order_does_not_matter_for_coalescing() {
        for reverse in [false, true] {
            let (mut h, _) = heap_with(16 * 1024);
            let l = Layout::from_size_align(128, 16).unwrap();
            let mut ps: std::vec::Vec<*mut u8> = std::vec::Vec::new();
            for _ in 0..40 {
                let p = unsafe { h.alloc(l) };
                assert!(!p.is_null());
                ps.push(p);
            }
            if reverse {
                ps.reverse();
            }
            for p in ps {
                unsafe { h.dealloc(p, l) };
            }
            assert_eq!(h.used(), 0);
            assert_eq!(h.hole_count(), 1, "reverse={reverse}: Loecher verschmelzen nicht");
            assert_eq!(h.free(), h.size());
        }
    }

    #[test]
    fn first_fit_reuses_the_freed_address() {
        let (mut h, _) = heap_with(8192);
        let l = Layout::from_size_align(64, 16).unwrap();
        let a = unsafe { h.alloc(l) };
        let b = unsafe { h.alloc(l) };
        unsafe { h.dealloc(a, l) };
        let c = unsafe { h.alloc(l) };
        assert_eq!(a, c, "First-Fit muss das vorderste Loch wiederverwenden");
        unsafe { h.dealloc(b, l) };
        unsafe { h.dealloc(c, l) };
        assert_eq!(h.hole_count(), 1);
    }

    #[test]
    fn very_large_alignments_are_honoured() {
        let (mut h, _) = heap_with(256 * 1024);
        for a in [1usize, 2, 4, 8, 8192, 16384] {
            let l = Layout::from_size_align(32, a).unwrap();
            let p = unsafe { h.alloc(l) };
            assert!(!p.is_null(), "keine Allokation mit align {a}");
            assert_eq!(p as usize % a.max(ALIGN), 0, "Ausrichtung {a} verletzt");
            unsafe { ptr::write_bytes(p, 0xa5, 32) };
            unsafe { h.dealloc(p, l) };
            assert_eq!(h.used(), 0, "Fueller vor dem Block wurde nicht zurueckgegeben");
        }
        assert_eq!(h.hole_count(), 1);
        assert_eq!(h.free(), h.size());
    }

    #[test]
    fn absurd_sizes_fail_without_wrapping() {
        let (mut h, _) = heap_with(64 * 1024);
        let voll = h.free();
        // Die groesstmoeglichen zulaessigen Layouts. Wichtig ist, dass die
        // Aufrundung auf 16 Byte NICHT umlaeuft — sonst waere `need` klein und
        // irgendein Block wuerde faelschlich passen.
        for size in [isize::MAX as usize - 15, isize::MAX as usize / 2, 1usize << 40] {
            let l = Layout::from_size_align(size, 16).unwrap();
            assert!(unsafe { h.alloc(l) }.is_null(), "size {size:#x} lieferte einen Zeiger");
            assert_eq!(align_up_checked(size, ALIGN).is_some(), true);
        }
        // Direkter Nachweis der Schutzrechnung: eine Groesse, die beim
        // Aufrunden ueberliefe, meldet None — und `alloc` gibt dann `null`.
        assert_eq!(align_up_checked(usize::MAX, ALIGN), None);
        assert_eq!(align_up_checked(usize::MAX - 14, ALIGN), None);
        assert_eq!(align_up_checked(usize::MAX - 15, ALIGN), Some(usize::MAX - 15));
        // Ausrichtung dicht unter dem Adressraumende: nur Fehlschlag erlaubt.
        let wild = Layout::from_size_align(16, 1usize << 62).unwrap();
        assert!(unsafe { h.alloc(wild) }.is_null());
        assert_eq!(h.used(), 0);
        assert_eq!(h.free(), voll, "Fehlversuche haben den Heap veraendert");
        // Der Heap muss danach ganz normal weiterarbeiten.
        let ok = Layout::from_size_align(128, 16).unwrap();
        let p = unsafe { h.alloc(ok) };
        assert!(!p.is_null());
        unsafe { h.dealloc(p, ok) };
        assert_eq!(h.hole_count(), 1);
    }

    #[test]
    fn regions_that_would_overflow_the_address_space_are_ignored() {
        let mut h = Heap::empty();
        // Laenge laeuft ueber das Adressraumende hinaus.
        unsafe { h.add_region(0xffff_ffff_ffff_f000usize as *mut u8, usize::MAX) };
        assert_eq!(h.size(), 0, "unmoegliche Region wurde uebernommen");
        // Startadresse so hoch, dass schon das Aufrunden ueberlaufen wuerde.
        unsafe { h.add_region(usize::MAX as *mut u8, 64) };
        assert_eq!(h.size(), 0);
        assert_eq!(h.hole_count(), 0);
        assert!(unsafe { h.alloc(Layout::from_size_align(16, 16).unwrap()) }.is_null());

        // Danach eine echte Region: der Heap muss davon unbeeindruckt sein.
        let (h2, _) = heap_with(4096);
        let mut h2 = h2;
        assert!(h2.size() > 0);
        let l = Layout::from_size_align(64, 16).unwrap();
        let p = unsafe { h2.alloc(l) };
        assert!(!p.is_null());
        unsafe { h2.dealloc(p, l) };
    }

    #[test]
    fn header_survives_alignment_padding() {
        // Bei grosser Ausrichtung liegt vor dem Header ein Fueller. `dealloc`
        // muss trotzdem den GANZEN Block zurueckgeben — sonst versickert bei
        // jeder ausgerichteten Allokation ein Stueck Heap.
        let (mut h, _) = heap_with(64 * 1024);
        let voll = h.free();
        for _ in 0..50 {
            let l = Layout::from_size_align(24, 512).unwrap();
            let p = unsafe { h.alloc(l) };
            assert!(!p.is_null());
            assert_eq!(p as usize % 512, 0);
            unsafe { h.dealloc(p, l) };
            assert_eq!(h.free(), voll, "Fueller wurde nicht zurueckgegeben");
            assert_eq!(h.hole_count(), 1);
        }
    }

    #[test]
    fn dealloc_accepts_a_different_but_compatible_layout() {
        // Der Header kennt die echte Blockgroesse; `dealloc` darf deshalb nicht
        // vom uebergebenen Layout abhaengen (Rust erlaubt hier z. B. `realloc`-
        // nahe Faelle nicht, aber der Kernel ruft mit dem Layout des Typs auf).
        let (mut h, _) = heap_with(8192);
        let voll = h.free();
        let a = Layout::from_size_align(100, 16).unwrap();
        let p = unsafe { h.alloc(a) };
        assert!(!p.is_null());
        // Gleiche Groesse, kleinere Ausrichtung -> gleicher Block.
        let b = Layout::from_size_align(100, 8).unwrap();
        unsafe { h.dealloc(p, b) };
        assert_eq!(h.free(), voll);
        assert_eq!(h.used(), 0);
        assert_eq!(h.hole_count(), 1);
    }

    #[test]
    fn stats_report_fragmentation_honestly() {
        // Nach dem Freigeben jedes zweiten Blocks muss `largest_hole` deutlich
        // kleiner sein als `free` — genau diese Zahl steht im OOM-Bericht des
        // Kernels und darf nicht schoengerechnet sein.
        let (mut h, _) = heap_with(32 * 1024);
        let l = Layout::from_size_align(256, 16).unwrap();
        let mut ps = std::vec::Vec::new();
        loop {
            let p = unsafe { h.alloc(l) };
            if p.is_null() {
                break;
            }
            ps.push(p);
        }
        assert!(ps.len() >= 16);
        for (i, p) in ps.iter().enumerate() {
            if i % 2 == 0 {
                unsafe { h.dealloc(*p, l) };
            }
        }
        let s = h.stats();
        assert!(s.holes >= ps.len() / 2 - 1, "Loecher werden falsch gezaehlt");
        assert!(s.largest_hole < s.free, "Fragmentierung wird verschwiegen");
        assert!(s.largest_hole >= 256);
        assert_eq!(s.used + s.free, s.size);
        // Eine Anfrage groesser als das groesste Loch muss scheitern.
        let zu_gross = Layout::from_size_align(s.largest_hole + 1, 16).unwrap();
        assert!(unsafe { h.alloc(zu_gross) }.is_null());
        for (i, p) in ps.iter().enumerate() {
            if i % 2 == 1 {
                unsafe { h.dealloc(*p, l) };
            }
        }
        assert_eq!(h.hole_count(), 1);
        assert_eq!(h.stats().largest_hole, h.size());
    }

    // -------------------------------------------------------- Eigenschaftstests

    /// Zufaellige alloc/free-Folge gegen ein Referenzmodell.
    ///
    /// Das Modell fuehrt Buch ueber alle lebenden Bloecke (Adresse, Groesse,
    /// Fuellmuster). Geprueft wird: kein Block ueberlappt einen anderen, jeder
    /// Block liegt im Heapfenster, der Inhalt bleibt unveraendert, die
    /// Buchhaltung (`used + free == size`) stimmt nach jedem Schritt, und am
    /// Ende gibt der Heap seinen kompletten Speicher wieder her.
    #[test]
    fn random_alloc_free_matches_reference_model() {
        const BYTES: usize = 128 * 1024;
        for seed in 0..8u64 {
            let (mut h, base) = heap_with(BYTES);
            let total = h.size();
            let mut r = Lcg::new(seed);
            // (Zeiger, Groesse, Ausrichtung, Fuellbyte)
            let mut live: std::vec::Vec<(usize, usize, usize, u8)> = std::vec::Vec::new();
            let mut fill: u8 = 1;

            for step in 0..1500 {
                if live.is_empty() || r.chance(55) {
                    let size = r.range(1, 900);
                    let align = 1usize << r.range(0, 9); // 1 .. 512
                    let l = Layout::from_size_align(size, align).unwrap();
                    let p = unsafe { h.alloc(l) };
                    if p.is_null() {
                        // OOM ist erlaubt — dann muss aber auch wirklich wenig
                        // zusammenhaengender Platz da sein.
                        assert!(
                            h.stats().largest_hole < size + HEADER + align,
                            "Schritt {step}: null trotz Loch von {} Bytes",
                            h.stats().largest_hole
                        );
                        continue;
                    }
                    let a = p as usize;
                    assert_eq!(a % align.max(ALIGN), 0, "Schritt {step}: Ausrichtung verletzt");
                    assert!(
                        a >= base as usize && a + size <= base as usize + BYTES,
                        "Schritt {step}: Block liegt ausserhalb des Heapfensters"
                    );
                    for (o, s, _, _) in &live {
                        assert!(
                            a + size <= *o || *o + *s <= a,
                            "Schritt {step}: Block {a:#x}+{size} ueberlappt {o:#x}+{s}"
                        );
                    }
                    fill = fill.wrapping_add(1).max(1);
                    unsafe { ptr::write_bytes(p, fill, size) };
                    live.push((a, size, align, fill));
                } else {
                    let idx = r.below(live.len());
                    let (a, s, al, f) = live.swap_remove(idx);
                    for k in 0..s {
                        assert_eq!(
                            unsafe { *(a as *const u8).add(k) },
                            f,
                            "Schritt {step}: Block {a:#x} wurde fremdbeschrieben"
                        );
                    }
                    let l = Layout::from_size_align(s, al).unwrap();
                    unsafe { h.dealloc(a as *mut u8, l) };
                }
                let st = h.stats();
                assert_eq!(st.used + st.free, total, "Schritt {step}: Buchhaltung kaputt");
                assert_eq!(st.holes, h.hole_count());
                assert!(st.largest_hole <= st.free);
                assert!(
                    st.used >= live.iter().map(|(_, s, _, _)| *s).sum::<usize>(),
                    "Schritt {step}: weniger belegt als vergeben"
                );
            }

            for (a, s, al, _) in live {
                unsafe { h.dealloc(a as *mut u8, Layout::from_size_align(s, al).unwrap()) };
            }
            assert_eq!(h.used(), 0, "Seed {seed}: Speicher versickert");
            assert_eq!(h.free(), total);
            assert_eq!(h.hole_count(), 1, "Seed {seed}: Heap bleibt fragmentiert");
        }
    }

    /// Gleiche Groesse, viele Runden: der Heap darf nicht langsam "leerlaufen"
    /// (jede Runde muss dieselbe Anzahl Bloecke liefern wie die erste).
    #[test]
    fn repeated_rounds_yield_the_same_capacity() {
        let (mut h, _) = heap_with(32 * 1024);
        let l = Layout::from_size_align(200, 32).unwrap();
        let mut first = 0usize;
        for round in 0..10 {
            let mut ps = std::vec::Vec::new();
            loop {
                let p = unsafe { h.alloc(l) };
                if p.is_null() {
                    break;
                }
                ps.push(p);
            }
            if round == 0 {
                first = ps.len();
                assert!(first > 10);
            } else {
                assert_eq!(ps.len(), first, "Runde {round} liefert weniger Bloecke");
            }
            for p in ps {
                unsafe { h.dealloc(p, l) };
            }
            assert_eq!(h.used(), 0);
            assert_eq!(h.hole_count(), 1);
        }
    }
}
