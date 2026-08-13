//! Winziger deterministischer Zufallsgenerator — **nur fuer Host-Tests**.
//!
//! Bewusst selbst geschrieben (rund 30 Zeilen), damit die Eigenschaftstests in
//! [`crate::bitmap`] und [`crate::heap`] ohne eine externe Crate wie `proptest`
//! oder `rand` auskommen. Ein linearer Kongruenzgenerator (LCG) nach Knuth
//! (MMIX-Konstanten) reicht dafuer vollstaendig: die Tests brauchen nur eine
//! reproduzierbare, gleichverteilte Folge, keine kryptografische Guete.
//!
//! Reproduzierbarkeit ist hier Absicht: schlaegt ein Eigenschaftstest fehl,
//! schlaegt er bei jedem Lauf mit derselben Folge fehl.

/// Deterministischer LCG-Zufallsgenerator fuer Tests.
pub struct Lcg(u64);

impl Lcg {
    /// Neuer Generator mit festem Startwert.
    pub const fn new(seed: u64) -> Self {
        Lcg(seed ^ 0x9e37_79b9_7f4a_7c15)
    }

    /// Naechster 64-Bit-Wert.
    pub fn next_u64(&mut self) -> u64 {
        // MMIX-Konstanten von Knuth.
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        // Oberes Wort zurueckgeben: die unteren Bits eines LCG sind schwach.
        (self.0 >> 32) | (self.0 << 32)
    }

    /// Wert in `0..n` (n > 0).
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "below(0) ist nicht definiert");
        (self.next_u64() % n as u64) as usize
    }

    /// Wert in `lo..=hi`.
    pub fn range(&mut self, lo: usize, hi: usize) -> usize {
        assert!(hi >= lo);
        lo + self.below(hi - lo + 1)
    }

    /// Mit Wahrscheinlichkeit `percent`/100 wahr.
    pub fn chance(&mut self, percent: u32) -> bool {
        self.below(100) < percent as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcg_is_deterministic_and_spread() {
        let mut a = Lcg::new(1);
        let mut b = Lcg::new(1);
        let mut c = Lcg::new(2);
        assert_eq!(a.next_u64(), b.next_u64(), "gleicher Seed, gleiche Folge");
        assert_ne!(Lcg::new(1).next_u64(), c.next_u64(), "andere Seeds trennen sich");

        // Grobe Gleichverteilung: 10 Faecher, 10_000 Ziehungen, keins leer,
        // keins ueber dem Doppelten des Erwartungswerts.
        let mut buckets = [0usize; 10];
        let mut r = Lcg::new(0xdead_beef);
        for _ in 0..10_000 {
            buckets[r.below(10)] += 1;
        }
        for (i, n) in buckets.iter().enumerate() {
            assert!(*n > 500 && *n < 2000, "Fach {i} unplausibel: {n}");
        }
    }

    #[test]
    fn range_and_chance_stay_in_bounds() {
        let mut r = Lcg::new(7);
        for _ in 0..1000 {
            let v = r.range(5, 9);
            assert!((5..=9).contains(&v));
            assert_eq!(r.range(3, 3), 3);
        }
        let mut hits = 0;
        for _ in 0..1000 {
            if r.chance(30) {
                hits += 1;
            }
        }
        assert!(hits > 150 && hits < 450, "chance(30) lieferte {hits}/1000");
        assert!(!r.chance(0));
        assert!(r.chance(100));
    }
}
