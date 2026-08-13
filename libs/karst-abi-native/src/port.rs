//! Ports — der Ersatz fuer Signals.
//!
//! Ein Port ist ein gewoehnliches Kernelobjekt mit Handle und Rechten. Wer auf
//! Ereignisse warten will, bindet ein Objekt an einen Port
//! ([`crate::Syscall::PortBind`]) und wartet dort ([`crate::Syscall::PortWait`]).
//!
//! Warum kein Signal-Mechanismus: Ein Signal kommt ohne Handle, ohne Rechte und
//! ohne Zustimmung des Empfaengers an und entfuehrt dessen Ausfuehrungsfaden.
//! Das ist genau die Umgebungsautoritaet ("ambient authority"), die diese ABI
//! nicht kennt. Ein Portpaket kommt dagegen nur an, wenn der Empfaenger
//! * ein Porthandle besitzt (mit [`crate::Rights::WAIT`]) und
//! * das Objekt selbst gebunden hat (mit [`crate::Rights::MANAGE`] am Port).
//!
//! Der `key` gehoert dem Wartenden: er waehlt ihn beim Binden und erkennt das
//! Ereignis daran wieder. Der Kernel gibt NIE eine Objektnummer oder gar eine
//! Adresse heraus.

use crate::handle::ObjectKind;

/// Ereignisbits eines Portpakets.
///
/// Bewusst wenige und bewusst objektunabhaengig: ein Wartender soll nicht
/// wissen muessen, was fuer ein Objekt hinter dem Schluessel steckt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signals(pub u32);

impl Signals {
    pub const NONE: Signals = Signals(0);
    /// Es liegen Daten/Nachrichten zum Lesen bereit.
    pub const READABLE: Signals = Signals(1 << 0);
    /// Es kann geschrieben werden, ohne zu blockieren.
    pub const WRITABLE: Signals = Signals(1 << 1);
    /// Die Gegenseite ist verschwunden (Kanalende geschlossen).
    pub const PEER_CLOSED: Signals = Signals(1 << 2);
    /// Ein Prozess ist beendet.
    pub const PROCESS_EXIT: Signals = Signals(1 << 3);

    /// Alle derzeit definierten Ereignisbits.
    pub const ALL: Signals = Signals(0xf);

    #[inline]
    pub const fn contains(self, other: Signals) -> bool {
        self.0 & other.0 == other.0
    }

    #[inline]
    pub const fn union(self, other: Signals) -> Signals {
        Signals(self.0 | other.0)
    }

    /// Schnittmenge — beim Binden wird die Menge nur kleiner, nie groesser.
    #[inline]
    pub const fn restrict(self, mask: Signals) -> Signals {
        Signals(self.0 & mask.0)
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for Signals {
    type Output = Signals;
    #[inline]
    fn bitor(self, rhs: Signals) -> Signals {
        self.union(rhs)
    }
}

/// Groesse eines Portpakets in Bytes. Fester Wert, damit ein Wartender den
/// Puffer ohne Rueckfrage bereitstellen kann.
pub const PACKET_BYTES: usize = 16;

/// Ein Ereignis, wie es [`crate::Syscall::PortWait`] in den Puffer des
/// Aufrufers schreibt. Enthaelt ausschliesslich Werte, die der Wartende selbst
/// gesetzt hat (`key`) oder die reine Ereignisbeschreibung — keine
/// Objektnummer, kein Zeiger, keine Kerneladresse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet {
    /// Vom Wartenden beim Binden vergebener Schluessel.
    pub key: u64,
    /// Eingetretene Ereignisse.
    pub signals: Signals,
    /// Art des gebundenen Objekts (nur zur Information).
    pub kind: ObjectKind,
}

impl Packet {
    pub const fn new(key: u64, signals: Signals, kind: ObjectKind) -> Self {
        Packet { key, signals, kind }
    }

    /// Feste Byte-Darstellung (little-endian): 8 B key, 4 B signals,
    /// 2 B Objektart, 2 B reserviert (immer 0).
    pub fn to_bytes(self) -> [u8; PACKET_BYTES] {
        let mut b = [0u8; PACKET_BYTES];
        b[0..8].copy_from_slice(&self.key.to_le_bytes());
        b[8..12].copy_from_slice(&self.signals.bits().to_le_bytes());
        b[12..14].copy_from_slice(&(self.kind as u16).to_le_bytes());
        b
    }

    /// Rueckrichtung fuer Tests und fuer eine spaetere Userspace-Bibliothek.
    /// Unbekannte Objektarten und gesetzte Reservebits ergeben `None` — ein
    /// halb verstandenes Paket wird nie geraten.
    pub fn from_bytes(b: &[u8; PACKET_BYTES]) -> Option<Packet> {
        if b[14] != 0 || b[15] != 0 {
            return None;
        }
        let key = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        let signals = Signals(u32::from_le_bytes([b[8], b[9], b[10], b[11]]));
        if !Signals::ALL.contains(signals) {
            return None;
        }
        let kind = match u16::from_le_bytes([b[12], b[13]]) {
            1 => ObjectKind::Stream,
            2 => ObjectKind::Memory,
            3 => ObjectKind::Channel,
            4 => ObjectKind::Port,
            5 => ObjectKind::Process,
            6 => ObjectKind::Thread,
            7 => ObjectKind::Namespace,
            8 => ObjectKind::Timer,
            _ => return None,
        };
        Some(Packet { key, signals, kind })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINGLE: [Signals; 4] =
        [Signals::READABLE, Signals::WRITABLE, Signals::PEER_CLOSED, Signals::PROCESS_EXIT];

    #[test]
    fn all_covers_exactly_the_defined_signals() {
        let mut u = Signals::NONE;
        for s in SINGLE {
            assert!(s.bits().is_power_of_two());
            assert!(!u.contains(s), "{s:?} doppelt vergeben");
            assert!(Signals::ALL.contains(s));
            u = u | s;
        }
        assert_eq!(u, Signals::ALL);
        assert!(Signals::NONE.is_empty());
        assert!(!Signals::READABLE.is_empty());
    }

    #[test]
    fn restrict_never_adds_signals() {
        for a in 0u32..=0xf {
            for b in 0u32..=0xf {
                let r = Signals(a).restrict(Signals(b));
                assert!(Signals(a).contains(r));
                assert!(Signals(b).contains(r));
                assert_eq!(r, Signals(b).restrict(Signals(a)));
            }
        }
        // Unbekannte Bits fallen bei der Einschraenkung mit ALL weg.
        assert_eq!(Signals(1 << 31).restrict(Signals::ALL), Signals::NONE);
    }

    #[test]
    fn packet_roundtrips_through_bytes() {
        let kinds = [
            ObjectKind::Stream,
            ObjectKind::Memory,
            ObjectKind::Channel,
            ObjectKind::Port,
            ObjectKind::Process,
            ObjectKind::Thread,
            ObjectKind::Namespace,
            ObjectKind::Timer,
        ];
        for (i, kind) in kinds.iter().enumerate() {
            for s in SINGLE {
                let p = Packet::new(0xdead_beef_0000_0000 + i as u64, s, *kind);
                let b = p.to_bytes();
                assert_eq!(b.len(), PACKET_BYTES);
                assert_eq!(Packet::from_bytes(&b), Some(p));
            }
        }
    }

    #[test]
    fn broken_packets_are_rejected_not_guessed() {
        let mut b = Packet::new(1, Signals::READABLE, ObjectKind::Channel).to_bytes();
        // Reservebits gesetzt.
        b[15] = 1;
        assert_eq!(Packet::from_bytes(&b), None);
        b[15] = 0;
        // Unbekannte Objektart.
        b[12] = 99;
        assert_eq!(Packet::from_bytes(&b), None);
        b[12] = 3;
        // Unbekanntes Ereignisbit.
        b[11] = 0x80;
        assert_eq!(Packet::from_bytes(&b), None);
    }

    #[test]
    fn packet_carries_no_kernel_data() {
        // Zusage der ABI: im Paket steht nichts als der eigene Schluessel, die
        // Ereignisbits und die Objektart. Insbesondere keine Objektnummer.
        let p = Packet::new(7, Signals::READABLE, ObjectKind::Channel);
        let b = p.to_bytes();
        assert_eq!(&b[0..8], &7u64.to_le_bytes());
        assert_eq!(&b[14..16], &[0, 0]);
        assert_eq!(core::mem::size_of_val(&b), PACKET_BYTES);
    }
}
