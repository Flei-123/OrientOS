//! # karst-native — die native ABI von Karstos
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

#[cfg(test)]
extern crate std;

pub mod handle;
pub mod rights;
pub mod syscall;

pub use handle::{Handle, ObjectKind, HANDLE_INVALID};
pub use rights::Rights;
pub use syscall::Syscall;

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

    #[test]
    fn encoding_is_unambiguous() {
        assert_eq!(encode(Ok(42)), 42);
        assert!(encode(Err(Error::BadHandle)) < 0);
    }
}
