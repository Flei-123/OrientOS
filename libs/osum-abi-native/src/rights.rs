//! Capability-Rechte. Bewusst handgeschrieben statt `bitflags`-Dependency
//! (Kernziel: lightweight — 40 Zeilen ersparen eine externe Crate).

/// Rechtebits eines [`crate::Handle`]. Rechte koennen beim Weitergeben nur
/// VERKLEINERT werden ([`Rights::restrict`]) — nie vergroessert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(pub u32);

impl Rights {
    pub const NONE: Rights = Rights(0);
    /// Lesen von Daten aus dem Objekt.
    pub const READ: Rights = Rights(1 << 0);
    /// Schreiben von Daten in das Objekt.
    pub const WRITE: Rights = Rights(1 << 1);
    /// Ausfuehren / als Code abbilden.
    pub const EXEC: Rights = Rights(1 << 2);
    /// Handle duplizieren.
    pub const DUPLICATE: Rights = Rights(1 << 3);
    /// Handle an einen anderen Prozess uebertragen.
    pub const TRANSFER: Rights = Rights(1 << 4);
    /// Eigenschaften abfragen.
    pub const INSPECT: Rights = Rights(1 << 5);
    /// Objekt konfigurieren/veraendern (Groesse, Policy ...).
    pub const MANAGE: Rights = Rights(1 << 6);
    /// Kindobjekte erzeugen (z. B. in einem Verzeichnis-/Namespace-Objekt).
    pub const CREATE: Rights = Rights(1 << 7);
    /// In den eigenen Adressraum einblenden.
    pub const MAP: Rights = Rights(1 << 8);
    /// Auf Signalisierung warten.
    pub const WAIT: Rights = Rights(1 << 9);

    /// Alle derzeit definierten Rechte.
    pub const ALL: Rights = Rights(0x3ff);

    #[inline]
    pub const fn contains(self, other: Rights) -> bool {
        self.0 & other.0 == other.0
    }

    #[inline]
    pub const fn union(self, other: Rights) -> Rights {
        Rights(self.0 | other.0)
    }

    /// Schnittmenge — der EINZIGE erlaubte Weg, Rechte bei der Weitergabe zu aendern.
    #[inline]
    pub const fn restrict(self, mask: Rights) -> Rights {
        Rights(self.0 & mask.0)
    }

    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for Rights {
    type Output = Rights;
    #[inline]
    fn bitor(self, rhs: Rights) -> Rights {
        self.union(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Alle einzeln definierten Rechte — Grundlage der erschoepfenden Tests.
    const SINGLE: [Rights; 10] = [
        Rights::READ,
        Rights::WRITE,
        Rights::EXEC,
        Rights::DUPLICATE,
        Rights::TRANSFER,
        Rights::INSPECT,
        Rights::MANAGE,
        Rights::CREATE,
        Rights::MAP,
        Rights::WAIT,
    ];

    #[test]
    fn all_covers_exactly_the_defined_bits() {
        let mut u = Rights::NONE;
        for r in SINGLE {
            assert!(r.bits().is_power_of_two(), "{r:?} ist kein einzelnes Bit");
            assert!(!u.contains(r), "{r:?} doppelt vergeben");
            assert!(Rights::ALL.contains(r), "{r:?} fehlt in ALL");
            u = u | r;
        }
        assert_eq!(u, Rights::ALL, "ALL passt nicht zu den Einzelrechten");
        assert_eq!(Rights::ALL.bits(), 0x3ff);
        assert_eq!(Rights::NONE.bits(), 0);
    }

    #[test]
    fn none_is_neutral_and_always_contained() {
        for r in SINGLE {
            assert!(r.contains(Rights::NONE), "jedes Recht enthaelt NONE");
            assert_eq!(r.union(Rights::NONE), r);
            assert_eq!(r.restrict(Rights::ALL), r);
            assert_eq!(r.restrict(Rights::NONE), Rights::NONE);
        }
        assert!(Rights::NONE.contains(Rights::NONE));
        assert!(!Rights::NONE.contains(Rights::READ));
    }

    /// Erschoepfend ueber alle 1024 Rechtemengen: `restrict` darf NIE ein Bit
    /// hinzufuegen, `union` nie eines verlieren, beide sind kommutativ.
    #[test]
    fn restrict_and_union_are_exhaustively_sound() {
        for a in 0u32..=0x3ff {
            let ra = Rights(a);
            assert!(Rights::ALL.contains(ra));
            for b in 0u32..=0x3ff {
                let rb = Rights(b);
                let res = ra.restrict(rb);
                assert!(ra.contains(res), "restrict fuegt Rechte hinzu: {a:#x}/{b:#x}");
                assert!(rb.contains(res));
                assert_eq!(res, rb.restrict(ra), "restrict nicht kommutativ");
                let un = ra | rb;
                assert!(un.contains(ra) && un.contains(rb));
                assert_eq!(un, rb.union(ra), "union nicht kommutativ");
                // Nochmals mit derselben Maske einschraenken aendert nichts.
                assert_eq!(res.restrict(rb), res);
                assert_eq!(ra.contains(rb), a & b == b);
            }
        }
    }

    #[test]
    fn undefined_bits_are_not_part_of_all() {
        let bogus = Rights(1 << 31);
        assert!(!Rights::ALL.contains(bogus));
        // Weitergabe mit ALL als Maske filtert unbekannte Bits weg.
        assert_eq!((Rights::READ | bogus).restrict(Rights::ALL), Rights::READ);
    }

    #[test]
    fn rights_can_only_shrink() {
        let rw = Rights::READ | Rights::WRITE;
        assert!(rw.contains(Rights::READ));
        let ro = rw.restrict(Rights::READ);
        assert!(!ro.contains(Rights::WRITE));
        // restrict kann nie Rechte hinzufuegen:
        assert!(!ro.restrict(Rights::ALL).contains(Rights::WRITE));
    }
}
