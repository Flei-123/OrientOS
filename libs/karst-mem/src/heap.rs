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

#[inline]
const fn align_up(v: usize, a: usize) -> usize {
    (v + a - 1) & !(a - 1)
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
        let s = align_up(start as usize, ALIGN);
        let e = (start as usize + size) & !(ALIGN - 1);
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
        let need = align_up(if layout.size() == 0 { 1 } else { layout.size() }, ALIGN);

        let mut prev: *mut *mut FreeBlock = &mut self.head;
        let mut cur = self.head;
        while !cur.is_null() {
            let bstart = cur as usize;
            let bsize = unsafe { (*cur).size };
            let bend = bstart + bsize;
            let payload = align_up(bstart + HEADER, align);

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
}
