//! Schicht **abi** — die Schnittstellen zum Userspace.
//!
//! Zwei streng getrennte Ebenen:
//! * [`native`] — die EIGENE, hauseigene API (Name aus `kcore::branding`):
//!   capability-basiert,
//!   handle-orientiert, ohne `fork`, ohne `errno`, ohne Signale. Das ist die
//!   Schnittstelle, auf die der Kernel dauerhaft ausgelegt ist.
//! * [`posix`] — ein reiner UEBERSETZER auf `native`, hinter dem Cargo-Feature
//!   `posix`. Er ist ersatzlos entfernbar: `cargo build --no-default-features`
//!   baut einen vollstaendigen Kernel ohne eine einzige Zeile POSIX-Semantik.
//!
//! Der Kernel-Core darf NIEMALS aus `posix` importieren. Umgekehrt kennt `posix`
//! ausschliesslich die ABI-Crate, keine Kernelinterna.

/// Beschreibung fuer das Boot-Log, wenn die POSIX-Schicht dabei ist.
#[cfg(feature = "posix")]
const NATIVE_WITH_POSIX: &str =
    concat!(env!("BRANDING_KERNEL_NAME"), "-native + posix (abwaehlbar)");
/// Beschreibung fuer das Boot-Log ohne POSIX-Schicht.
#[cfg(not(feature = "posix"))]
const NATIVE_ONLY: &str =
    concat!(env!("BRANDING_KERNEL_NAME"), "-native (POSIX nicht einkompiliert)");

pub mod native;

#[cfg(feature = "posix")]
pub mod posix;

/// Welche ABIs sind in diesem Build enthalten?
pub fn enabled() -> &'static str {
    #[cfg(feature = "posix")]
    {
        NATIVE_WITH_POSIX
    }
    #[cfg(not(feature = "posix"))]
    {
        NATIVE_ONLY
    }
}
