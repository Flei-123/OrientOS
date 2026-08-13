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
}
