//! Handles — die einzige Referenz auf Kernelobjekte in karst-native.

use crate::Rights;

/// Ungueltiges Handle. Anders als bei POSIX ist 0 KEIN gueltiger "stdin".
pub const HANDLE_INVALID: Handle = Handle(0);

/// Ein Handle ist ein 64-Bit-Wert: 32 Bit Slot-Index + 32 Bit Generation.
/// Die Generation verhindert Use-after-close-Verwechslungen (ABA-Problem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Handle(pub u64);

impl Handle {
    #[inline]
    pub const fn new(slot: u32, generation: u32) -> Self {
        Handle(((generation as u64) << 32) | (slot as u64 + 1))
    }
    #[inline]
    pub const fn slot(self) -> u32 {
        ((self.0 & 0xffff_ffff) as u32).wrapping_sub(1)
    }
    #[inline]
    pub const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 & 0xffff_ffff != 0
    }
}

/// Typ des Kernelobjekts hinter einem Handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ObjectKind {
    /// Byteorientierter Datenstrom (Konsole, Pipe-Ende).
    Stream = 1,
    /// Adressierbares Speicherobjekt (VMO), einblendbar.
    Memory = 2,
    /// Nachrichtenkanal (bidirektional, uebertraegt auch Handles).
    Channel = 3,
    /// Wartepunkt fuer asynchrone Ereignisse — ersetzt Signals.
    Port = 4,
    /// Prozess.
    Process = 5,
    /// Thread.
    Thread = 6,
    /// Namensraum-Knoten (Verzeichnis-Aequivalent, aber ohne globalen Root).
    Namespace = 7,
    /// Zeitgeber.
    Timer = 8,
}

/// Ein Handle mitsamt seinen Rechten und dem Objekttyp, wie ihn die
/// Handle-Tabelle des Kernels speichert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleEntry {
    pub handle: Handle,
    pub kind: ObjectKind,
    pub rights: Rights,
}

impl HandleEntry {
    pub const fn new(handle: Handle, kind: ObjectKind, rights: Rights) -> Self {
        Self { handle, kind, rights }
    }

    /// Prueft die Rechte fuer eine Operation.
    pub fn check(&self, needed: Rights) -> crate::Result<()> {
        if self.rights.contains(needed) {
            Ok(())
        } else {
            Err(crate::Error::RightsDenied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_generation_roundtrip() {
        let h = Handle::new(7, 3);
        assert_eq!(h.slot(), 7);
        assert_eq!(h.generation(), 3);
        assert!(h.is_valid());
        assert!(!HANDLE_INVALID.is_valid());
    }

    #[test]
    fn rights_are_enforced() {
        let e = HandleEntry::new(Handle::new(1, 1), ObjectKind::Stream, Rights::READ);
        assert!(e.check(Rights::READ).is_ok());
        assert_eq!(e.check(Rights::WRITE), Err(crate::Error::RightsDenied));
    }

    /// Alle Objekttypen — damit eine neue Variante hier auffaellt.
    const ALL_KINDS: [ObjectKind; 8] = [
        ObjectKind::Stream,
        ObjectKind::Memory,
        ObjectKind::Channel,
        ObjectKind::Port,
        ObjectKind::Process,
        ObjectKind::Thread,
        ObjectKind::Namespace,
        ObjectKind::Timer,
    ];

    #[test]
    fn object_kinds_have_stable_distinct_numbers() {
        let mut seen = std::vec::Vec::new();
        for (i, k) in ALL_KINDS.iter().enumerate() {
            let n = *k as u16;
            assert_eq!(n as usize, i + 1, "Nummer von {k:?} verschoben");
            assert!(!seen.contains(&n));
            seen.push(n);
        }
        // Kein Objekttyp darf die 0 belegen: 0 bleibt "kein Objekt".
        assert!(!seen.contains(&0));
    }

    #[test]
    fn slot_and_generation_roundtrip_over_the_whole_range() {
        // Slot 0 ist gueltig (anders als POSIX-Fd 0), das Handle 0 ist es nicht.
        let zero = Handle::new(0, 0);
        assert!(zero.is_valid());
        assert_ne!(zero, HANDLE_INVALID);
        assert_eq!(zero.slot(), 0);
        assert_eq!(zero.generation(), 0);

        // Der hoechste darstellbare Slot ist u32::MAX-1: im unteren Wort steht
        // `slot + 1`, damit die 0 fuer HANDLE_INVALID frei bleibt.
        for slot in [0u32, 1, 2, 255, 4096, 0x00ff_ffff, u32::MAX - 1] {
            for generation in [0u32, 1, 7, 0x8000_0000, u32::MAX] {
                let h = Handle::new(slot, generation);
                assert_eq!(h.slot(), slot, "Slot {slot} verloren");
                assert_eq!(h.generation(), generation, "Generation {generation} verloren");
                assert!(h.is_valid());
            }
        }
    }

    #[test]
    fn different_generations_are_different_handles() {
        // Kern der Use-after-close-Absicherung: gleicher Slot, neue Generation
        // ergibt ein anderes Handle.
        let old = Handle::new(5, 1);
        let new = Handle::new(5, 2);
        assert_ne!(old, new);
        assert_eq!(old.slot(), new.slot());
        assert_ne!(old.generation(), new.generation());
        assert_ne!(Handle::new(5, 1), Handle::new(6, 1));
    }

    #[test]
    fn invalid_handle_stays_invalid() {
        assert_eq!(HANDLE_INVALID.0, 0);
        assert!(!HANDLE_INVALID.is_valid());
        // Auch mit gesetzter Generation bleibt ein leeres Slotfeld ungueltig.
        assert!(!Handle(0xdead_beef_0000_0000).is_valid());
        assert!(Handle(1).is_valid());
    }

    #[test]
    fn check_accepts_supersets_and_rejects_missing_bits() {
        let full = HandleEntry::new(
            Handle::new(2, 9),
            ObjectKind::Memory,
            Rights::READ | Rights::WRITE | Rights::MAP,
        );
        assert!(full.check(Rights::NONE).is_ok(), "leere Forderung ist immer erfuellt");
        assert!(full.check(Rights::READ | Rights::WRITE).is_ok());
        assert!(full.check(Rights::MAP).is_ok());
        assert_eq!(full.check(Rights::EXEC), Err(crate::Error::RightsDenied));
        assert_eq!(
            full.check(Rights::READ | Rights::EXEC),
            Err(crate::Error::RightsDenied),
            "ein fehlendes Bit reicht zum Ablehnen"
        );

        let none = HandleEntry::new(Handle::new(0, 0), ObjectKind::Port, Rights::NONE);
        for r in [Rights::READ, Rights::WRITE, Rights::WAIT, Rights::ALL] {
            assert_eq!(none.check(r), Err(crate::Error::RightsDenied));
        }
        assert!(none.check(Rights::NONE).is_ok());
    }

    #[test]
    fn entry_keeps_handle_kind_and_rights() {
        let h = Handle::new(11, 3);
        for k in ALL_KINDS {
            let e = HandleEntry::new(h, k, Rights::INSPECT);
            assert_eq!(e.handle, h);
            assert_eq!(e.kind, k);
            assert_eq!(e.rights, Rights::INSPECT);
            assert_eq!(e, HandleEntry::new(h, k, Rights::INSPECT));
        }
    }
}
