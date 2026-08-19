//! # osum-native — die native ABI von Karstos
//!
//! Designregeln (bewusst KEIN POSIX):
//! * **Capability-basiert**: Es gibt keinen globalen Namensraum, auf den ein Prozess
//!   "einfach so" zugreifen kann. Jeder Zugriff geht ueber ein [`Handle`], das
//!   explizit uebergeben wurde und feste [`Rights`] traegt.
//! * **Handle-orientiert**: keine kleinen int-Dateideskriptoren mit impliziter
//!   Vererbung, keine 0/1/2-Sonderrollen.
//! * **Kein `fork`**: Prozesse werden mit explizit uebergebener Handle-Liste
//!   erzeugt ([`Syscall::ProcessSpawn`]) — kein Kopieren eines Adressraums.
//! * **Kein `errno`**: Fehler sind Rueckgabewerte ([`Error`]), thread-lokale
//!   Fehlerglobals existieren nicht.
//! * **Keine Signals**: Asynchrone Ereignisse laufen ueber Ports/Events
//!   ([`Syscall::PortWait`]), also ueber Nachrichten statt ueber Stack-Hijacking.
//!
//! Diese Crate definiert NUR die ABI (Typen, Zahlen, Bedeutungen). Sie enthaelt
//! keinen Kernelzustand und ist damit auch fuer eine spaetere Userspace-libc
//! bzw. fuer Tests direkt verwendbar.
#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod handle;
pub mod name;
pub mod port;
pub mod rights;
pub mod syscall;
pub mod table;

pub use handle::{Handle, HandleEntry, ObjectKind, HANDLE_INVALID};
pub use port::{Packet, Signals, PACKET_BYTES};
pub use rights::Rights;
pub use syscall::Syscall;
pub use table::HandleTable;

/// Fehlerwerte der nativen ABI. Bewusst klein, bewusst nicht POSIX-kompatibel:
/// Die POSIX-Schicht uebersetzt diese Werte, nicht umgekehrt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Error {
    /// Handle existiert nicht oder gehoert nicht zu diesem Prozess.
    BadHandle = -1,
    /// Handle existiert, hat aber nicht die noetigen [`Rights`].
    RightsDenied = -2,
    /// Objekt existiert nicht.
    NotFound = -3,
    /// Argument ausserhalb des erlaubten Bereichs.
    InvalidArgs = -4,
    /// Ressource erschoepft (Speicher, Handle-Tabelle, ...).
    Exhausted = -5,
    /// Operation passt nicht zum Objekttyp.
    WrongType = -6,
    /// Wuerde blockieren, Aufruf war nicht-blockierend.
    WouldBlock = -7,
    /// Von der Gegenseite geschlossen.
    Closed = -8,
    /// Vom Kernel (noch) nicht angeboten.
    NotSupported = -9,
}

impl Error {
    /// Rohwert fuer den Syscall-Rueckgabekanal.
    #[inline]
    pub const fn as_raw(self) -> i64 {
        self as i64
    }

    /// Rueckuebersetzung eines Rohwerts. Nur die vergebenen Nummern gelten;
    /// alles andere ist [`Error::NotSupported`] — ein unbekannter Fehlercode
    /// darf nie versehentlich als "Erfolg" oder als fremder Fehler gelten.
    pub const fn from_raw(raw: i64) -> Option<Error> {
        Some(match raw {
            -1 => Error::BadHandle,
            -2 => Error::RightsDenied,
            -3 => Error::NotFound,
            -4 => Error::InvalidArgs,
            -5 => Error::Exhausted,
            -6 => Error::WrongType,
            -7 => Error::WouldBlock,
            -8 => Error::Closed,
            -9 => Error::NotSupported,
            _ => return None,
        })
    }

    /// Wandelt einen Rueckgabewert der ABI zurueck in ein [`Result`].
    pub const fn decode(raw: i64) -> Result<u64> {
        if raw >= 0 {
            Ok(raw as u64)
        } else {
            match Error::from_raw(raw) {
                Some(e) => Err(e),
                None => Err(Error::NotSupported),
            }
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Error::BadHandle => "BadHandle",
            Error::RightsDenied => "RightsDenied",
            Error::NotFound => "NotFound",
            Error::InvalidArgs => "InvalidArgs",
            Error::Exhausted => "Exhausted",
            Error::WrongType => "WrongType",
            Error::WouldBlock => "WouldBlock",
            Error::Closed => "Closed",
            Error::NotSupported => "NotSupported",
        }
    }
}

/// Ergebnis eines nativen Aufrufs.
pub type Result<T> = core::result::Result<T, Error>;

/// Kodiert ein `Result<u64>` in den einzelnen Registerrueckgabewert der ABI:
/// >= 0 Erfolg, < 0 Fehlercode. Werte >= 2^63 sind per Konvention verboten.
#[inline]
pub fn encode(r: Result<u64>) -> i64 {
    match r {
        Ok(v) => (v & 0x7fff_ffff_ffff_ffff) as i64,
        Err(e) => e.as_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Alle Fehlerwerte der ABI — Grundlage der erschoepfenden Tests.
    /// Eine neue Variante faellt hier sofort auf (die Tests zaehlen mit).
    pub(crate) const ALL_ERRORS: [Error; 9] = [
        Error::BadHandle,
        Error::RightsDenied,
        Error::NotFound,
        Error::InvalidArgs,
        Error::Exhausted,
        Error::WrongType,
        Error::WouldBlock,
        Error::Closed,
        Error::NotSupported,
    ];

    /// Winziger deterministischer Generator — nur fuer die Eigenschaftstests
    /// dieser Datei. Bewusst 4 Zeilen statt einer `rand`-Dependency.
    struct Xs(u64);
    impl Xs {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn decode_is_the_exact_inverse_of_encode() {
        for e in ALL_ERRORS {
            assert_eq!(Error::from_raw(e.as_raw()), Some(e));
            assert_eq!(Error::decode(encode(Err(e))), Err(e));
        }
        for v in [0u64, 1, 42, 4096, i64::MAX as u64] {
            assert_eq!(Error::decode(encode(Ok(v))), Ok(v));
        }
        // Unbekannte negative Werte gelten als "nicht angeboten", nie als Erfolg.
        for raw in [-10i64, -1000, i64::MIN] {
            assert_eq!(Error::from_raw(raw), None);
            assert_eq!(Error::decode(raw), Err(Error::NotSupported));
        }
    }

    #[test]
    fn encoding_is_unambiguous() {
        assert_eq!(encode(Ok(42)), 42);
        assert!(encode(Err(Error::BadHandle)) < 0);
    }

    #[test]
    fn error_numbers_are_dense_distinct_and_negative() {
        let mut seen = std::vec::Vec::new();
        for (i, e) in ALL_ERRORS.iter().enumerate() {
            let raw = e.as_raw();
            assert_eq!(raw, -(i as i64 + 1), "Nummer von {} verschoben", e.name());
            assert!(raw < 0, "{} ist nicht negativ", e.name());
            assert!(!seen.contains(&raw), "{} doppelt vergeben", e.name());
            seen.push(raw);
        }
        // Die 0 bleibt fuer den Erfolgsfall reserviert.
        assert!(!seen.contains(&0));
    }

    #[test]
    fn error_names_are_unique_and_nonempty() {
        let mut seen = std::vec::Vec::new();
        for e in ALL_ERRORS {
            let n = e.name();
            assert!(!n.is_empty());
            assert!(!seen.contains(&n), "Name {n} doppelt");
            seen.push(n);
        }
        assert_eq!(seen.len(), ALL_ERRORS.len());
    }

    #[test]
    fn every_error_encodes_to_its_own_raw_value() {
        for e in ALL_ERRORS {
            assert_eq!(encode(Err(e)), e.as_raw());
            assert!(encode(Err(e)) < 0, "{} kollidiert mit dem Erfolgsbereich", e.name());
        }
    }

    #[test]
    fn success_values_stay_non_negative() {
        for v in [0u64, 1, 42, 4096, i64::MAX as u64 - 1, i64::MAX as u64] {
            let r = encode(Ok(v));
            assert!(r >= 0, "Erfolg {v} wurde als Fehler kodiert: {r}");
            assert_eq!(r as u64, v);
        }
    }

    #[test]
    fn out_of_contract_success_values_are_clamped_not_wrapped() {
        // Werte >= 2^63 sind laut ABI verboten. Wichtig ist, dass sie NIE als
        // Fehler durchgehen — sonst wuerde ein Erfolg zum Fehlercode.
        for v in [1u64 << 63, u64::MAX, 0x8000_0000_0000_002a] {
            let r = encode(Ok(v));
            assert!(r >= 0, "Wert {v:#x} wurde zu einem Fehlercode {r}");
            assert_eq!(r as u64, v & 0x7fff_ffff_ffff_ffff);
        }
    }

    /// Eigenschaftstest: Erfolg und Fehler duerfen sich im Rueckgabekanal nie
    /// verwechseln lassen, und erlaubte Erfolgswerte kommen unveraendert an.
    #[test]
    fn encode_never_confuses_success_and_error() {
        let mut r = Xs(0x1234_5678_9abc_def1);
        for _ in 0..20_000 {
            let raw = r.next();
            let v = raw >> 1; // im erlaubten Bereich (< 2^63)
            let enc = encode(Ok(v));
            assert!(enc >= 0);
            assert_eq!(enc as u64, v);
            for e in ALL_ERRORS {
                assert_ne!(enc, e.as_raw(), "Erfolg {v} sieht aus wie {}", e.name());
            }
            // Auch der verbotene Bereich darf nicht in den Fehlerraum kippen.
            assert!(encode(Ok(v | (1 << 63))) >= 0);
        }
    }

    #[test]
    fn result_alias_carries_the_error_type() {
        let ok: Result<u64> = Ok(7);
        let err: Result<u64> = Err(Error::WouldBlock);
        assert_eq!(ok.unwrap(), 7);
        assert_eq!(err.unwrap_err(), Error::WouldBlock);
        assert_eq!(err.unwrap_err().name(), "WouldBlock");
        // Copy/Eq gelten — der Kernel reicht Fehler ohne Klonen weiter.
        let e = Error::Closed;
        let f = e;
        assert_eq!(e, f);
        assert_ne!(Error::Closed, Error::NotFound);
    }
}
