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
    use crate::testrand::Lcg;

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
        assert_eq!(Bitmap::words_needed(0), 0);
        assert_eq!(Bitmap::words_needed(128), 2);
        assert_eq!(Bitmap::words_needed(129), 3);
    }

    // ---------------------------------------------------------------- Randfaelle

    #[test]
    fn frame_count_need_not_be_a_multiple_of_64() {
        // 70 Frames in 2 Woertern: die 58 ueberzaehligen Bits duerfen NIE
        // vergeben werden, sonst zeigt ein Frame auf Speicher, den es nicht gibt.
        let mut buf = [0u64; 2];
        let mut b = make(70, &mut buf);
        b.free_range(0, 1000); // absichtlich ueber das Ende hinaus
        assert_eq!(b.free_frames(), 70);
        let mut seen = std::vec::Vec::new();
        while let Ok(f) = b.alloc() {
            assert!(f < 70, "Frame {f} liegt ausserhalb der Bitmap");
            seen.push(f);
        }
        assert_eq!(seen.len(), 70);
        assert_eq!(b.free_frames(), 0);
        assert!(b.is_used(70), "Frames hinter dem Ende gelten als belegt");
        assert!(b.is_used(usize::MAX));
    }

    #[test]
    fn reserve_and_free_are_idempotent() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        b.free_range(0, 256); // zweites Mal darf den Zaehler nicht verbiegen
        assert_eq!(b.free_frames(), 256);
        b.reserve_range(10, 20);
        b.reserve_range(10, 20);
        assert_eq!(b.free_frames(), 246);
        b.free_range(10, 20);
        assert_eq!(b.free_frames(), 256);
        assert_eq!(b.used_frames(), 0);
        assert_eq!(b.frames(), 256);
    }

    #[test]
    fn empty_and_reversed_ranges_do_nothing() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        b.free_range(100, 100);
        b.reserve_range(100, 100);
        b.reserve_range(200, 50); // end < start
        assert_eq!(b.free_frames(), 256);
    }

    #[test]
    fn alloc_rejects_impossible_counts() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        assert_eq!(b.alloc_contiguous(0), Err(AllocError::OutOfMemory));
        assert_eq!(b.alloc_contiguous(257), Err(AllocError::OutOfMemory));
        assert_eq!(b.free_frames(), 256, "Fehlversuch darf nichts belegen");
        // Genau die volle Bitmap geht noch.
        assert_eq!(b.alloc_contiguous(256), Ok(0));
        assert_eq!(b.free_frames(), 0);
    }

    #[test]
    fn free_outside_bitmap_is_rejected() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        let f = b.alloc_contiguous(4).unwrap();
        assert_eq!(b.free(250, 10), Err(AllocError::OutOfRange));
        assert_eq!(b.free(256, 1), Err(AllocError::OutOfRange));
        assert_eq!(b.free_frames(), 252, "abgewiesenes free darf nichts aendern");
        assert_eq!(b.free(f, 4), Ok(()));
        assert_eq!(b.free_frames(), 256);
        // Doppeltes free bleibt buchhalterisch korrekt.
        assert_eq!(b.free(f, 4), Ok(()));
        assert_eq!(b.free_frames(), 256);
    }

    #[test]
    fn allocation_spans_word_boundaries() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        b.reserve_range(0, 60);
        // 10 zusammenhaengende Frames passen erst ab 60, also ueber die
        // Wortgrenze bei 64 hinweg.
        let f = b.alloc_contiguous(10).unwrap();
        assert_eq!(f, 60);
        for i in 60..70 {
            assert!(b.is_used(i));
        }
        assert!(!b.is_used(70));
    }

    #[test]
    fn cursor_wraps_around_and_reuses_freed_frames() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        let mut all = std::vec::Vec::new();
        for _ in 0..256 {
            all.push(b.alloc().unwrap());
        }
        assert_eq!(b.alloc(), Err(AllocError::OutOfMemory));
        // Ein Frame in der Mitte frei -> der Suchzeiger muss ihn finden.
        b.free(128, 1).unwrap();
        assert_eq!(b.alloc(), Ok(128));
        // Ein Frame ganz vorne frei -> Wrap-around noetig.
        b.free(0, 1).unwrap();
        assert_eq!(b.alloc(), Ok(0));
        assert_eq!(b.alloc(), Err(AllocError::OutOfMemory));
        for f in all {
            b.free(f, 1).unwrap();
        }
        assert_eq!(b.free_frames(), 256);
    }

    #[test]
    fn fragmentation_blocks_large_contiguous_requests() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        // Jeden zweiten Frame belegen: 128 frei, aber nirgends 2 am Stueck.
        for i in (0..256).step_by(2) {
            b.reserve_range(i, i + 1);
        }
        assert_eq!(b.free_frames(), 128);
        assert_eq!(b.alloc_contiguous(2), Err(AllocError::OutOfMemory));
        assert!(b.alloc_contiguous(1).is_ok());
    }

    #[test]
    fn single_frame_bitmap_works() {
        let mut buf = [0u64; 1];
        let mut b = make(1, &mut buf);
        b.free_range(0, 1);
        assert_eq!(b.alloc(), Ok(0));
        assert_eq!(b.alloc(), Err(AllocError::OutOfMemory));
        b.free(0, 1).unwrap();
        assert_eq!(b.alloc_contiguous(1), Ok(0));
    }

    #[test]
    fn zero_frame_bitmap_never_allocates() {
        // Entartete Memory-Map: kein einziger nutzbarer Frame.
        let mut buf = [0u64; 1];
        let mut b = make(0, &mut buf);
        assert_eq!(b.frames(), 0);
        assert_eq!(b.free_frames(), 0);
        assert_eq!(b.alloc(), Err(AllocError::OutOfMemory));
        assert_eq!(b.alloc_contiguous(1), Err(AllocError::OutOfMemory));
        assert!(b.is_used(0));
        b.free_range(0, 10); // darf nicht in Panik geraten
        assert_eq!(b.free_frames(), 0);
    }

    #[test]
    #[should_panic(expected = "Bitmap zu klein")]
    fn undersized_backing_store_panics_at_construction() {
        let mut buf = [0u64; 1];
        let _ = Bitmap::new_all_used(&mut buf, 65);
    }

    #[test]
    fn allocations_never_overlap() {
        let mut buf = [0u64; 4];
        let mut b = make(256, &mut buf);
        b.free_range(0, 256);
        let mut owned = [false; 256];
        let mut round = 0usize;
        loop {
            let n = 1 + round % 3;
            round += 1;
            match b.alloc_contiguous(n) {
                Ok(f) => {
                    for i in f..f + n {
                        assert!(!owned[i], "Frame {i} zweimal vergeben");
                        owned[i] = true;
                    }
                }
                Err(_) => break,
            }
        }
        assert_eq!(b.free_frames(), owned.iter().filter(|x| !**x).count());
    }

    // ------------------------------------------------------- Eigenschaftstest

    /// Zufaellige alloc/free-Folge gegen ein Referenzmodell (`Vec<bool>`).
    ///
    /// Geprueft wird nach JEDEM Schritt: Belegungszaehler, Rueckgabewert,
    /// Ueberlappungsfreiheit — und die Zusicherung, dass `OutOfMemory` nur
    /// kommt, wenn das Modell wirklich keinen passenden Bereich mehr hat.
    #[test]
    fn random_alloc_free_matches_reference_model() {
        const FRAMES: usize = 512;
        for seed in 0..12u64 {
            let mut buf = [0u64; 8];
            let mut b = make(FRAMES, &mut buf);
            b.free_range(0, FRAMES);
            let mut model = std::vec![false; FRAMES]; // true = belegt
            let mut live: std::vec::Vec<(usize, usize)> = std::vec::Vec::new();
            let mut r = Lcg::new(seed);

            for step in 0..2000 {
                if live.is_empty() || r.chance(55) {
                    let count = r.range(1, 8);
                    let res = b.alloc_contiguous(count);
                    let possible = (0..=FRAMES - count)
                        .any(|i| model[i..i + count].iter().all(|u| !*u));
                    match res {
                        Ok(f) => {
                            assert!(possible, "Schritt {step}: Allokation trotz Modell-OOM");
                            assert!(f + count <= FRAMES, "Bereich {f}+{count} ausserhalb");
                            for i in f..f + count {
                                assert!(!model[i], "Schritt {step}: Frame {i} doppelt vergeben");
                                model[i] = true;
                            }
                            live.push((f, count));
                        }
                        Err(AllocError::OutOfMemory) => {
                            assert!(!possible, "Schritt {step}: OOM obwohl Platz da war");
                        }
                        Err(e) => panic!("Schritt {step}: unerwarteter Fehler {e:?}"),
                    }
                } else {
                    let idx = r.below(live.len());
                    let (f, count) = live.swap_remove(idx);
                    b.free(f, count).unwrap();
                    for i in f..f + count {
                        model[i] = false;
                    }
                }
                let used = model.iter().filter(|u| **u).count();
                assert_eq!(b.used_frames(), used, "Schritt {step}: Zaehler weicht ab");
                assert_eq!(b.free_frames(), FRAMES - used);
            }

            for i in 0..FRAMES {
                assert_eq!(b.is_used(i), model[i], "Bit {i} weicht vom Modell ab");
            }
            for (f, count) in live {
                b.free(f, count).unwrap();
            }
            assert_eq!(b.free_frames(), FRAMES, "Seed {seed}: Frames versickert");
        }
    }

    /// Nach einer beliebigen Folge muss der Allocator wieder die volle
    /// Kapazitaet hergeben — der Zustand darf nicht "verschleissen".
    #[test]
    fn allocator_recovers_full_capacity_after_random_churn() {
        const FRAMES: usize = 256;
        let mut buf = [0u64; 4];
        let mut b = make(FRAMES, &mut buf);
        b.free_range(0, FRAMES);
        let mut r = Lcg::new(99);
        let mut live: std::vec::Vec<(usize, usize)> = std::vec::Vec::new();
        for _ in 0..2000 {
            if r.chance(60) {
                let n = r.range(1, 16);
                if let Ok(f) = b.alloc_contiguous(n) {
                    live.push((f, n));
                }
            } else if !live.is_empty() {
                let (f, n) = live.swap_remove(r.below(live.len()));
                b.free(f, n).unwrap();
            }
        }
        for (f, n) in live {
            b.free(f, n).unwrap();
        }
        assert_eq!(b.free_frames(), FRAMES);
        // Ein einziger Block ueber die gesamte Bitmap muss wieder gehen.
        assert_eq!(b.alloc_contiguous(FRAMES), Ok(0));
    }
}
