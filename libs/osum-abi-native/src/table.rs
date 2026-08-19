//! Handle-Tabelle eines Prozesses — der Kern der Capability-Regeln.
//!
//! Die Tabelle liegt bewusst in der ABI-Crate und nicht im Kernel: so ist sie
//! auf dem Host erschoepfend testbar, und der Kernel enthaelt nur noch die
//! Verbindung zu echten Objekten.
//!
//! Zusagen dieser Datenstruktur (jede einzelne ist unten getestet):
//! * Ein Handle ist **Slot + Generation**. Ein geschlossenes Handle trifft nach
//!   Wiederverwendung des Slots niemals das neue Objekt (ABA/Use-after-close).
//! * Rechte koennen beim Duplizieren und beim Uebertragen nur **kleiner**
//!   werden — nie groesser.
//! * Es gibt **kein Erben**: eine frische Tabelle ist leer. Wer etwas darf,
//!   hat es explizit uebergeben bekommen.
//! * Handles sind **prozesslokal**: ein Wert aus einer fremden Tabelle ist hier
//!   entweder unbekannt (`BadHandle`) oder trifft ein anderes Objekt — er
//!   verleiht nie den Zugriff des anderen Prozesses.

use alloc::vec::Vec;

use crate::{Error, Handle, HandleEntry, ObjectKind, Result, Rights};

/// Obergrenze der Tabellenplaetze je Prozess. Eine Grenze ist Pflicht: ohne sie
/// koennte ein Prozess den Kernelspeicher durch Handle-Erzeugung erschoepfen.
pub const DEFAULT_LIMIT: usize = 1024;

/// Ein Tabellenplatz. Die Generation lebt weiter, auch wenn der Platz frei ist —
/// genau das macht alte Handles dauerhaft ungueltig.
struct Slot {
    generation: u32,
    entry: Option<HandleEntry>,
}

/// Handle-Tabelle **eines** Prozesses.
pub struct HandleTable {
    slots: Vec<Slot>,
    limit: usize,
    /// Prozesseigener Wuerfelwert. Er wird in das Generationsfeld des Handles
    /// eingerechnet, damit derselbe Slot in zwei Prozessen NICHT denselben
    /// Handle-Wert ergibt: ein abgehoerter Wert aus einem anderen Prozess ist
    /// damit auch dann kein Treffer, wenn dort zufaellig derselbe Platz
    /// belegt ist.
    nonce: u32,
}

impl HandleTable {
    /// Leere Tabelle mit der Standardgrenze.
    pub const fn new() -> Self {
        HandleTable { slots: Vec::new(), limit: DEFAULT_LIMIT, nonce: 0 }
    }

    /// Leere Tabelle mit eigener Platzgrenze.
    pub const fn with_limit(limit: usize) -> Self {
        HandleTable { slots: Vec::new(), limit, nonce: 0 }
    }

    /// Leere Tabelle mit prozesseigenem Wuerfelwert.
    pub const fn with_nonce(limit: usize, nonce: u32) -> Self {
        HandleTable { slots: Vec::new(), limit, nonce }
    }

    /// Der Wuerfelwert dieser Tabelle.
    pub const fn nonce(&self) -> u32 {
        self.nonce
    }

    /// Traegt ein Objekt ein und liefert das frische Handle.
    pub fn insert(&mut self, kind: ObjectKind, rights: Rights, object: u64) -> Result<Handle> {
        let entry = HandleEntry::with_object(Handle::new(0, 0), kind, rights, object);
        self.adopt(entry)
    }

    /// Nimmt einen fertigen Eintrag auf (z. B. aus einer Uebertragung) und
    /// vergibt dafuer ein NEUES Handle. Der alte Handle-Wert wird bewusst nicht
    /// uebernommen: Handle-Werte gelten nur in genau einer Tabelle.
    pub fn adopt(&mut self, entry: HandleEntry) -> Result<Handle> {
        let slot = match self.slots.iter().position(|s| s.entry.is_none()) {
            Some(i) => i,
            None => {
                if self.slots.len() >= self.limit {
                    return Err(Error::Exhausted);
                }
                self.slots.push(Slot { generation: 0, entry: None });
                self.slots.len() - 1
            }
        };
        let s = &mut self.slots[slot];
        // Generation zaehlt bei JEDER Belegung hoch, nicht beim Freigeben:
        // damit unterscheidet sich das neue Handle immer vom alten.
        s.generation = s.generation.wrapping_add(1);
        if s.generation == 0 {
            s.generation = 1;
        }
        let h = Handle::new(slot as u32, s.generation ^ self.nonce);
        s.entry = Some(HandleEntry::with_object(h, entry.kind, entry.rights, entry.object));
        Ok(h)
    }

    /// Sucht einen Eintrag. Falscher Index, falsche Generation und das
    /// ungueltige Handle enden alle in [`Error::BadHandle`].
    pub fn get(&self, h: Handle) -> Result<&HandleEntry> {
        if !h.is_valid() {
            return Err(Error::BadHandle);
        }
        let e = self
            .slots
            .get(h.slot() as usize)
            .and_then(|s| s.entry.as_ref())
            .ok_or(Error::BadHandle)?;
        if e.handle != h {
            return Err(Error::BadHandle);
        }
        Ok(e)
    }

    /// Wie [`Self::get`], prueft zusaetzlich die noetigen Rechte.
    /// Ein gueltiges Handle ohne das Recht liefert [`Error::RightsDenied`] —
    /// bewusst ein ANDERER Fehler als ein gefaelschtes Handle.
    pub fn get_with(&self, h: Handle, needed: Rights) -> Result<&HandleEntry> {
        let e = self.get(h)?;
        e.check(needed)?;
        Ok(e)
    }

    /// Schliesst ein Handle und liefert den entfernten Eintrag.
    pub fn close(&mut self, h: Handle) -> Result<HandleEntry> {
        let e = *self.get(h)?;
        self.slots[h.slot() as usize].entry = None;
        Ok(e)
    }

    /// Dupliziert ein Handle mit eingeschraenkten Rechten.
    /// Braucht [`Rights::DUPLICATE`]; die Maske kann Rechte nur wegnehmen.
    pub fn duplicate(&mut self, h: Handle, mask: Rights) -> Result<Handle> {
        let e = *self.get_with(h, Rights::DUPLICATE)?;
        let neu = HandleEntry::with_object(h, e.kind, e.rights.restrict(mask), e.object);
        self.adopt(neu)
    }

    /// Nimmt ein Handle zur Uebergabe an einen anderen Prozess heraus.
    /// Braucht [`Rights::TRANSFER`]; der Eintrag verlaesst diese Tabelle und
    /// traegt nur noch die eingeschraenkten Rechte.
    pub fn take_for_transfer(&mut self, h: Handle, mask: Rights) -> Result<HandleEntry> {
        let e = *self.get_with(h, Rights::TRANSFER)?;
        self.slots[h.slot() as usize].entry = None;
        Ok(HandleEntry::with_object(h, e.kind, e.rights.restrict(mask), e.object))
    }

    /// Anzahl belegter Plaetze.
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.entry.is_some()).count()
    }

    /// Ist die Tabelle leer? (Frisch erzeugte Prozesse: ja — kein Erben.)
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Platzgrenze dieser Tabelle.
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Alle belegten Eintraege.
    pub fn iter(&self) -> impl Iterator<Item = &HandleEntry> + '_ {
        self.slots.iter().filter_map(|s| s.entry.as_ref())
    }
}

impl Default for HandleTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_table_grants_nothing() {
        let t = HandleTable::new();
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
        // Kein Handle, kein Zugriff: auch "naheliegende" Werte greifen nicht.
        for raw in [0u64, 1, 2, 3, 0xffff_ffff, u64::MAX] {
            assert_eq!(t.get(Handle(raw)).unwrap_err(), Error::BadHandle);
        }
    }

    #[test]
    fn stale_generation_is_rejected_after_slot_reuse() {
        let mut t = HandleTable::new();
        let alt = t.insert(ObjectKind::Stream, Rights::READ, 7).unwrap();
        t.close(alt).unwrap();
        let neu = t.insert(ObjectKind::Stream, Rights::READ, 8).unwrap();
        assert_eq!(neu.slot(), alt.slot(), "Slot wurde nicht wiederverwendet");
        assert_ne!(neu.generation(), alt.generation());
        assert_eq!(t.get(alt).unwrap_err(), Error::BadHandle);
        assert_eq!(t.get(neu).unwrap().object, 8);
        assert_eq!(t.close(alt).unwrap_err(), Error::BadHandle, "doppeltes Schliessen");
    }

    #[test]
    fn guessing_indexes_does_not_open_objects() {
        // Der Angreifer kennt den Index (0), aber nicht die Generation.
        let mut t = HandleTable::new();
        let echt = t.insert(ObjectKind::Channel, Rights::ALL, 1).unwrap();
        let mut treffer = 0usize;
        for g in 0u32..4096 {
            if g == echt.generation() {
                continue;
            }
            if t.get(Handle::new(echt.slot(), g)).is_ok() {
                treffer += 1;
            }
        }
        assert_eq!(treffer, 0, "geratene Generation hat ein Objekt geoeffnet");
    }

    #[test]
    fn rights_only_shrink_on_duplicate_and_transfer() {
        let mut t = HandleTable::new();
        let voll = t
            .insert(
                ObjectKind::Stream,
                Rights::READ | Rights::WRITE | Rights::DUPLICATE | Rights::TRANSFER,
                42,
            )
            .unwrap();
        // Duplizieren auf reines Lesen.
        let ro = t.duplicate(voll, Rights::READ).unwrap();
        assert_eq!(t.get(ro).unwrap().rights, Rights::READ);
        assert_eq!(t.get_with(ro, Rights::WRITE).unwrap_err(), Error::RightsDenied);
        // Der Versuch, beim Duplizieren MEHR zu bekommen, scheitert still:
        // die Maske ist eine Schnittmenge, kein Wunschzettel.
        assert_eq!(t.get_with(ro, Rights::DUPLICATE).unwrap_err(), Error::RightsDenied);
        let noch_kleiner = t.duplicate(voll, Rights::ALL).unwrap();
        assert_eq!(t.get(noch_kleiner).unwrap().rights, t.get(voll).unwrap().rights);
    }

    #[test]
    fn duplicate_needs_the_duplicate_right() {
        let mut t = HandleTable::new();
        let h = t.insert(ObjectKind::Stream, Rights::READ | Rights::WRITE, 1).unwrap();
        assert_eq!(t.duplicate(h, Rights::READ).unwrap_err(), Error::RightsDenied);
        assert_eq!(t.len(), 1, "abgewiesenes Duplizieren hat trotzdem belegt");
    }

    #[test]
    fn transfer_moves_the_entry_and_needs_the_transfer_right() {
        let mut a = HandleTable::new();
        let mut b = HandleTable::new();
        let ohne = a.insert(ObjectKind::Stream, Rights::READ, 1).unwrap();
        assert_eq!(a.take_for_transfer(ohne, Rights::ALL).unwrap_err(), Error::RightsDenied);

        let mit = a.insert(ObjectKind::Stream, Rights::READ | Rights::TRANSFER, 2).unwrap();
        let e = a.take_for_transfer(mit, Rights::READ).unwrap();
        assert_eq!(a.get(mit).unwrap_err(), Error::BadHandle, "Geber behaelt das Handle");
        let neu = b.adopt(e).unwrap();
        assert_eq!(b.get(neu).unwrap().object, 2);
        assert_eq!(b.get(neu).unwrap().rights, Rights::READ, "Rechte nicht eingeschraenkt");
        // Der Handle-WERT des Gebers gilt beim Nehmer nicht automatisch.
        assert!(b.get(mit).is_err() || b.get(mit).unwrap().handle == neu);
    }

    #[test]
    fn handles_are_process_local() {
        // Zwei Prozesse, zwei Wuerfelwerte — wie im Kernel.
        let mut a = HandleTable::with_nonce(DEFAULT_LIMIT, 0x5a5a_1234);
        let mut b = HandleTable::with_nonce(DEFAULT_LIMIT, 0x0f0f_9876);
        let geheim = a.insert(ObjectKind::Process, Rights::ALL, 0xdead).unwrap();
        // b hat NICHTS. Der geklaute Wert oeffnet in b nichts.
        assert_eq!(b.get(geheim).unwrap_err(), Error::BadHandle);
        // Auch wenn b denselben Slot belegt: anderer Wuerfelwert, anderer Wert,
        // und der geklaute Wert trifft weiterhin nichts.
        let eigen = b.insert(ObjectKind::Stream, Rights::READ, 1).unwrap();
        assert_eq!(eigen.slot(), geheim.slot());
        assert_ne!(eigen, geheim, "gleicher Handle-Wert in zwei Prozessen");
        assert_eq!(b.get(geheim).unwrap_err(), Error::BadHandle);
        assert_eq!(b.get(eigen).unwrap().object, 1);
        // Und in a trifft b's Wert ebenso wenig.
        assert_eq!(a.get(eigen).unwrap_err(), Error::BadHandle);
    }

    #[test]
    fn nonce_changes_the_handle_value_but_not_the_slot() {
        let mut a = HandleTable::with_nonce(DEFAULT_LIMIT, 0);
        let mut b = HandleTable::with_nonce(DEFAULT_LIMIT, 0xabcd_ef01);
        let ha = a.insert(ObjectKind::Stream, Rights::READ, 1).unwrap();
        let hb = b.insert(ObjectKind::Stream, Rights::READ, 1).unwrap();
        assert_eq!(ha.slot(), hb.slot());
        assert_ne!(ha.generation(), hb.generation());
        assert_eq!(b.nonce(), 0xabcd_ef01);
        assert!(a.get(ha).is_ok() && b.get(hb).is_ok());
    }

    #[test]
    fn limit_is_enforced_and_recovers_after_close() {
        let mut t = HandleTable::with_limit(3);
        let mut hs = alloc::vec::Vec::new();
        for i in 0..3 {
            hs.push(t.insert(ObjectKind::Port, Rights::WAIT, i).unwrap());
        }
        assert_eq!(t.insert(ObjectKind::Port, Rights::WAIT, 9).unwrap_err(), Error::Exhausted);
        assert_eq!(t.limit(), 3);
        t.close(hs[1]).unwrap();
        let neu = t.insert(ObjectKind::Port, Rights::WAIT, 9).unwrap();
        assert_eq!(neu.slot(), 1);
        assert_eq!(t.len(), 3);
        assert_eq!(t.iter().count(), 3);
    }

    #[test]
    fn errors_distinguish_forgery_from_missing_rights() {
        let mut t = HandleTable::new();
        let h = t.insert(ObjectKind::Stream, Rights::READ, 1).unwrap();
        assert_eq!(t.get_with(h, Rights::WRITE).unwrap_err(), Error::RightsDenied);
        assert_eq!(t.get_with(Handle::new(h.slot(), h.generation() + 1), Rights::READ)
            .unwrap_err(), Error::BadHandle);
    }

    #[test]
    fn generation_never_becomes_zero_on_wrap() {
        let mut t = HandleTable::new();
        let h = t.insert(ObjectKind::Timer, Rights::WAIT, 0).unwrap();
        // Slot 0 kuenstlich auf die letzte Generation setzen und neu belegen.
        t.close(h).unwrap();
        t.slots[0].generation = u32::MAX;
        let neu = t.insert(ObjectKind::Timer, Rights::WAIT, 0).unwrap();
        assert_eq!(neu.generation(), 1, "Generation 0 wuerde ein Handle mehrdeutig machen");
        // Nach 2^32 Wiederbelegungen desselben Platzes wiederholt sich ein
        // Handle-Wert zwangslaeufig — das ist die dokumentierte Grenze eines
        // 32-Bit-Zaehlers, nicht ein Loch in der Pruefung.
        assert!(t.get(neu).is_ok());
    }
}
