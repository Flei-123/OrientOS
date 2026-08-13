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
