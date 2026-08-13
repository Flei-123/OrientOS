//! Anbindung der nativen ABI an den Kernel.
//!
//! **Stand: Grundgeruest.** Die ABI selbst (Typen, Nummern, Rechte) ist in der
//! Crate [`karst_abi_native`] vollstaendig definiert und host-getestet. Hier
//! steht die Kernelseite: die Handle-Tabelle je Prozess und der Verteiler.
//! Implementiert ist bislang nur [`Syscall::Version`] — alles andere meldet
//! sauber [`Error::NotSupported`] statt zu `todo!()`. So bleibt der Kernel in
//! jedem Zustand lauffaehig, und die Phase "Userspace" der ROADMAP fuellt die
//! Faelle einzeln auf.

use alloc::vec::Vec;
use karst_abi_native::handle::HandleEntry;
use karst_abi_native::{encode, Error, Handle, ObjectKind, Result, Rights, Syscall};

/// Handle-Tabelle eines Prozesses.
///
/// Bewusst KEINE POSIX-Semantik: es gibt keine "kleinste freie Zahl", keine
/// Sonderrollen 0/1/2 und kein implizites Erben. Freigewordene Plaetze werden
/// mit erhoehter Generation wiederverwendet, damit ein altes Handle nach dem
/// Schliessen niemals versehentlich ein neues Objekt trifft.
pub struct HandleTable {
    slots: Vec<Option<HandleEntry>>,
    generations: Vec<u32>,
}

impl HandleTable {
    /// Leere Tabelle.
    pub fn new() -> Self {
        HandleTable { slots: Vec::new(), generations: Vec::new() }
    }

    /// Traegt ein Objekt ein und liefert das Handle.
    pub fn insert(&mut self, kind: ObjectKind, rights: Rights) -> Handle {
        let slot = match self.slots.iter().position(|s| s.is_none()) {
            Some(i) => i,
            None => {
                self.slots.push(None);
                self.generations.push(0);
                self.slots.len() - 1
            }
        };
        self.generations[slot] = self.generations[slot].wrapping_add(1);
        let h = Handle::new(slot as u32, self.generations[slot]);
        self.slots[slot] = Some(HandleEntry::new(h, kind, rights));
        h
    }

    /// Sucht einen Eintrag und prueft die Generation.
    pub fn get(&self, h: Handle) -> Result<&HandleEntry> {
        let slot = h.slot() as usize;
        let e = self.slots.get(slot).and_then(|s| s.as_ref()).ok_or(Error::BadHandle)?;
        if e.handle.generation() != h.generation() {
            return Err(Error::BadHandle);
        }
        Ok(e)
    }

    /// Schliesst ein Handle.
    pub fn close(&mut self, h: Handle) -> Result<()> {
        let slot = h.slot() as usize;
        let cur = self.slots.get_mut(slot).ok_or(Error::BadHandle)?;
        match cur {
            Some(e) if e.handle.generation() == h.generation() => {
                *cur = None;
                Ok(())
            }
            _ => Err(Error::BadHandle),
        }
    }

    /// Anzahl belegter Plaetze.
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// Ist die Tabelle leer?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for HandleTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Fuehrt einen nativen Aufruf aus.
///
/// Rueckgabe ist der kodierte ABI-Wert (>= 0 Erfolg, < 0 Fehler).
pub fn dispatch(nr: u64, _args: [u64; 6]) -> i64 {
    let Some(call) = Syscall::from_raw(nr) else {
        return Error::InvalidArgs.as_raw();
    };
    let result: Result<u64> = match call {
        Syscall::Version => Ok(karst_abi_native::syscall::ABI_VERSION),
        // Alle weiteren Aufrufe brauchen Scheduler, VFS und Userspace —
        // siehe ROADMAP.md, Phasen 2 bis 4.
        _ => Err(Error::NotSupported),
    };
    encode(result)
}

/// Kurzer Selbsttest der ABI-Kernelseite fuer das Boot-Log.
/// Liefert (ABI-Version, Handles nach dem Test, Fehlercode eines
/// absichtlich ungueltigen Handles).
pub fn self_test() -> (i64, usize, &'static str) {
    let version = dispatch(Syscall::Version as u64, [0; 6]);
    let mut t = HandleTable::new();
    let h = t.insert(ObjectKind::Stream, Rights::READ | Rights::WRITE);
    let stale = h;
    let _ = t.close(h);
    let err = match t.get(stale) {
        Err(e) => e.name(),
        Ok(_) => "FEHLER: geschlossenes Handle noch gueltig",
    };
    let _ = t.insert(ObjectKind::Channel, Rights::ALL);
    (version, t.len(), err)
}
