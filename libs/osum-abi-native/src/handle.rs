//! Handles — die einzige Referenz auf Kernelobjekte in osum-native.

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
    /// Kennung des Kernelobjekts, auf das dieses Handle zeigt. Bewusst eine
    /// ZAHL und kein Zeiger: aus einem Handle laesst sich damit auch bei einem
    /// Fehler im Kernel keine Adresse gewinnen.
    pub object: u64,
}

impl HandleEntry {
    pub const fn new(handle: Handle, kind: ObjectKind, rights: Rights) -> Self {
        Self { handle, kind, rights, object: 0 }
    }

    /// Eintrag mit Objektkennung.
    pub const fn with_object(
        handle: Handle,
        kind: ObjectKind,
        rights: Rights,
        object: u64,
    ) -> Self {
        Self { handle, kind, rights, object }
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

    #[test]
    fn slot_and_generation_fields_do_not_bleed_into_each_other() {
        // Rohbild pruefen: unteres Wort = slot+1, oberes Wort = Generation.
        let h = Handle::new(0x0000_0007, 0xdead_beef);
        assert_eq!(h.0, 0xdead_beef_0000_0008);
        assert_eq!(h.slot(), 7);
        assert_eq!(h.generation(), 0xdead_beef);
        // Maximale Generation darf den Slot nicht kippen und umgekehrt.
        let max_gen = Handle::new(1, u32::MAX);
        assert_eq!(max_gen.slot(), 1);
        assert_eq!(max_gen.generation(), u32::MAX);
        let max_slot = Handle::new(u32::MAX - 1, 0);
        assert_eq!(max_slot.slot(), u32::MAX - 1);
        assert_eq!(max_slot.generation(), 0);
        assert!(max_slot.is_valid());
    }

    /// Eine kleine Handle-Tabelle wie im Kernel: Slots mit Generationszaehler.
    /// Der Test weist nach, dass die Generation genau das leistet, wofuer sie
    /// da ist — ein geschlossenes Handle oeffnet nach Wiederverwendung des
    /// Slots kein fremdes Objekt (Use-after-close / ABA).
    struct Table {
        slots: [Option<HandleEntry>; 4],
        gen: [u32; 4],
    }

    impl Table {
        fn new() -> Self {
            Table { slots: [None; 4], gen: [0; 4] }
        }

        fn insert(&mut self, kind: ObjectKind, rights: Rights) -> crate::Result<Handle> {
            for i in 0..self.slots.len() {
                if self.slots[i].is_none() {
                    self.gen[i] = self.gen[i].wrapping_add(1);
                    let h = Handle::new(i as u32, self.gen[i]);
                    self.slots[i] = Some(HandleEntry::new(h, kind, rights));
                    return Ok(h);
                }
            }
            Err(crate::Error::Exhausted)
        }

        fn get(&self, h: Handle) -> crate::Result<&HandleEntry> {
            if !h.is_valid() {
                return Err(crate::Error::BadHandle);
            }
            let e = self
                .slots
                .get(h.slot() as usize)
                .and_then(|s| s.as_ref())
                .ok_or(crate::Error::BadHandle)?;
            if e.handle != h {
                return Err(crate::Error::BadHandle);
            }
            Ok(e)
        }

        fn close(&mut self, h: Handle) -> crate::Result<()> {
            self.get(h)?;
            self.slots[h.slot() as usize] = None;
            Ok(())
        }
    }

    #[test]
    fn closed_handles_never_reopen_a_reused_slot() {
        let mut t = Table::new();
        let a = t.insert(ObjectKind::Stream, Rights::READ).unwrap();
        assert_eq!(t.get(a).unwrap().kind, ObjectKind::Stream);
        t.close(a).unwrap();
        assert_eq!(t.get(a).unwrap_err(), crate::Error::BadHandle);
        assert_eq!(t.close(a).unwrap_err(), crate::Error::BadHandle, "doppeltes close");

        // Derselbe Slot, neues Objekt: das alte Handle darf nicht passen.
        let b = t.insert(ObjectKind::Timer, Rights::WAIT).unwrap();
        assert_eq!(b.slot(), a.slot(), "Slot wurde nicht wiederverwendet");
        assert_ne!(b, a);
        assert_eq!(t.get(a).unwrap_err(), crate::Error::BadHandle);
        assert_eq!(t.get(b).unwrap().kind, ObjectKind::Timer);
    }

    #[test]
    fn table_rejects_invalid_and_out_of_range_handles() {
        let mut t = Table::new();
        let h = t.insert(ObjectKind::Channel, Rights::READ | Rights::WRITE).unwrap();
        assert_eq!(t.get(HANDLE_INVALID).unwrap_err(), crate::Error::BadHandle);
        assert_eq!(t.get(Handle::new(99, 1)).unwrap_err(), crate::Error::BadHandle);
        assert_eq!(t.get(Handle(u64::MAX)).unwrap_err(), crate::Error::BadHandle);
        // Richtiger Slot, falsche Generation.
        assert_eq!(
            t.get(Handle::new(h.slot(), h.generation() + 1)).unwrap_err(),
            crate::Error::BadHandle
        );
        assert!(t.get(h).is_ok());
    }

    #[test]
    fn full_table_reports_exhausted_and_recovers_after_close() {
        let mut t = Table::new();
        let mut hs = std::vec::Vec::new();
        for _ in 0..4 {
            hs.push(t.insert(ObjectKind::Port, Rights::WAIT).unwrap());
        }
        assert_eq!(t.insert(ObjectKind::Port, Rights::WAIT), Err(crate::Error::Exhausted));
        // Alle Handles sind paarweise verschieden und alle gueltig.
        for (i, a) in hs.iter().enumerate() {
            assert!(a.is_valid());
            assert!(t.get(*a).is_ok());
            for b in &hs[i + 1..] {
                assert_ne!(a, b);
            }
        }
        t.close(hs[2]).unwrap();
        let neu = t.insert(ObjectKind::Process, Rights::INSPECT).unwrap();
        assert_eq!(neu.slot(), 2);
        assert!(t.get(neu).is_ok());
        assert_eq!(t.get(hs[2]).unwrap_err(), crate::Error::BadHandle);
    }

    #[test]
    fn rights_survive_the_table_and_are_checked_per_operation() {
        let mut t = Table::new();
        let ro = t.insert(ObjectKind::Stream, Rights::READ | Rights::INSPECT).unwrap();
        let e = *t.get(ro).unwrap();
        assert!(e.check(Rights::READ).is_ok());
        assert!(e.check(Rights::READ | Rights::INSPECT).is_ok());
        assert_eq!(e.check(Rights::WRITE), Err(crate::Error::RightsDenied));
        // Weitergabe darf nur verkleinern — hier auf reines Lesen.
        let weiter = e.rights.restrict(Rights::READ);
        let entry2 = HandleEntry::new(Handle::new(3, 1), e.kind, weiter);
        assert_eq!(entry2.check(Rights::INSPECT), Err(crate::Error::RightsDenied));
        assert!(entry2.check(Rights::READ).is_ok());
    }
}
