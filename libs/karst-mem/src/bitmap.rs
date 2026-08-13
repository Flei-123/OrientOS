//! Bitmap-basierter Frame-Allocator-Kern (reine Logik, host-testbar).
//!
//! Begruendung Bitmap statt Buddy (siehe ARCHITECTURE.md): konstanter, sehr
//! kleiner Metadaten-Overhead (1 Bit je 4 KiB = 32 KiB pro GiB RAM), triviale
//! Wiederherstellbarkeit nach Reclaim von Bootloader-Speicher und kein
//! rekursives Splitting im fruehen Boot. Grosse zusammenhaengende Allokationen
//! sind dafuer O(n) — fuer Phase 1 (Heap-Backing, Page-Tables) irrelevant.

/// Ein Frame-Index (physische Adresse / PAGE_SIZE).
pub type FrameIndex = usize;

/// Fehler des Allokators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocError {
    /// Kein passender freier Bereich mehr vorhanden.
    OutOfMemory,
    /// Index liegt ausserhalb der Bitmap.
    OutOfRange,
}

/// Bitmap ueber `frames` Frames. Bit gesetzt = belegt.
///
/// Der Speicher fuer die Bitmap wird von aussen geliefert (`&'static mut [u64]`),
/// damit diese Struktur keinerlei Allokator braucht und im fruehen Boot lebt.
pub struct Bitmap<'a> {
    bits: &'a mut [u64],
    frames: usize,
    used: usize,
    /// Suchzeiger fuer next-fit; vermeidet O(n) vom Anfang bei jeder Allokation.
    cursor: usize,
}

impl<'a> Bitmap<'a> {
    /// Erzeugt eine Bitmap, in der zunaechst ALLES als belegt markiert ist.
    /// Freigeben erfolgt gezielt ueber [`Bitmap::free_range`] fuer nutzbare Regionen.
    pub fn new_all_used(bits: &'a mut [u64], frames: usize) -> Self {
        assert!(bits.len() * 64 >= frames, "Bitmap zu klein fuer Framezahl");
        for w in bits.iter_mut() {
            *w = u64::MAX;
        }
        Self { bits, frames, used: frames, cursor: 0 }
    }

    #[inline]
    pub const fn frames(&self) -> usize {
        self.frames
    }
    #[inline]
    pub const fn used_frames(&self) -> usize {
        self.used
    }
    #[inline]
    pub const fn free_frames(&self) -> usize {
        self.frames - self.used
    }

    /// Anzahl u64-Woerter, die fuer `frames` Frames noetig sind.
    #[inline]
    pub const fn words_needed(frames: usize) -> usize {
        (frames + 63) / 64
    }

    #[inline]
    fn test(&self, i: FrameIndex) -> bool {
        (self.bits[i / 64] >> (i % 64)) & 1 == 1
    }

    #[inline]
    fn set(&mut self, i: FrameIndex) {
        self.bits[i / 64] |= 1u64 << (i % 64);
    }

    #[inline]
    fn clear(&mut self, i: FrameIndex) {
        self.bits[i / 64] &= !(1u64 << (i % 64));
    }

    /// Ist der Frame belegt?
    pub fn is_used(&self, i: FrameIndex) -> bool {
        i >= self.frames || self.test(i)
    }

    /// Markiert [start, end) als frei (nur Frames innerhalb der Bitmap).
    pub fn free_range(&mut self, start: FrameIndex, end: FrameIndex) {
        for i in start..end.min(self.frames) {
            if self.test(i) {
                self.clear(i);
                self.used -= 1;
            }
        }
    }

    /// Markiert [start, end) als belegt (z. B. Kernel-Image, Bitmap selbst).
    pub fn reserve_range(&mut self, start: FrameIndex, end: FrameIndex) {
        for i in start..end.min(self.frames) {
            if !self.test(i) {
                self.set(i);
                self.used += 1;
            }
        }
    }

    /// Allokiert genau einen Frame.
    pub fn alloc(&mut self) -> Result<FrameIndex, AllocError> {
        self.alloc_contiguous(1)
    }

    /// Allokiert `count` zusammenhaengende Frames (next-fit, mit Wrap-around).
    pub fn alloc_contiguous(&mut self, count: usize) -> Result<FrameIndex, AllocError> {
        if count == 0 || count > self.frames {
            return Err(AllocError::OutOfMemory);
        }
        let start_cursor = self.cursor;
        let mut i = start_cursor;
        let mut wrapped = false;
        while !(wrapped && i >= start_cursor) {
            if i + count > self.frames {
                i = 0;
                wrapped = true;
                continue;
            }
            // Wortweiser Schnellvorlauf: volle Woerter ueberspringen.
            if count == 1 && i % 64 == 0 && self.bits[i / 64] == u64::MAX {
                i += 64;
                continue;
            }
            let mut ok = true;
            let mut j = i;
            while j < i + count {
                if self.test(j) {
                    ok = false;
                    break;
                }
                j += 1;
            }
            if ok {
                for k in i..i + count {
                    self.set(k);
                }
                self.used += count;
                self.cursor = i + count;
                return Ok(i);
            }
            i = j + 1;
        }
        Err(AllocError::OutOfMemory)
    }

    /// Gibt `count` Frames ab `index` frei.
    pub fn free(&mut self, index: FrameIndex, count: usize) -> Result<(), AllocError> {
        if index + count > self.frames {
            return Err(AllocError::OutOfRange);
        }
        for i in index..index + count {
            if self.test(i) {
                self.clear(i);
                self.used -= 1;
            }
        }
        if index < self.cursor {
            self.cursor = index;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(frames: usize, buf: &mut [u64]) -> Bitmap<'_> {
        Bitmap::new_all_used(buf, frames)
    }

    #[test]
    fn all_used_initially() {
        let mut buf = [0u64; 4];
        let b = make(256, &mut buf);
        assert_eq!(b.free_frames(), 0);
        assert!(b.is_used(0));
    }

    #[test]
    fn alloc_and_free_roundtrip() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        assert_eq!(b.free_frames(), 256);
        let f = b.alloc().unwrap();
        assert_eq!(f, 0);
        assert!(b.is_used(0));
        assert_eq!(b.free_frames(), 255);
        b.free(f, 1).unwrap();
        assert_eq!(b.free_frames(), 256);
    }

    #[test]
    fn contiguous_allocation_skips_used() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        b.reserve_range(0, 10);
        let f = b.alloc_contiguous(8).unwrap();
        assert_eq!(f, 10);
        assert_eq!(b.free_frames(), 256 - 10 - 8);
    }

    #[test]
    fn exhaustion_reports_oom() {
        let mut buf = [0u64; 1];
        let mut b = make(64, &mut buf);
        b.free_range(0, 64);
        for _ in 0..64 {
            b.alloc().unwrap();
        }
        assert_eq!(b.alloc(), Err(AllocError::OutOfMemory));
    }

    #[test]
    fn words_needed_rounds_up() {
        assert_eq!(Bitmap::words_needed(1), 1);
        assert_eq!(Bitmap::words_needed(64), 1);
        assert_eq!(Bitmap::words_needed(65), 2);
    }
}
