//! Schicht **abi** — die Schnittstellen zum Userspace.
//!
//! Zwei streng getrennte Ebenen:
//! * [`native`] — die EIGENE API `karst-native`: capability-basiert,
//!   handle-orientiert, ohne `fork`, ohne `errno`, ohne Signale. Das ist die
//!   Schnittstelle, auf die der Kernel dauerhaft ausgelegt ist.
//! * [`posix`] — ein reiner UEBERSETZER auf `native`, hinter dem Cargo-Feature
//!   `posix`. Er ist ersatzlos entfernbar: `cargo build --no-default-features`
//!   baut einen vollstaendigen Kernel ohne eine einzige Zeile POSIX-Semantik.
//!
//! Der Kernel-Core darf NIEMALS aus `posix` importieren. Umgekehrt kennt `posix`
//! ausschliesslich die ABI-Crate, keine Kernelinterna.

pub mod native;

#[cfg(feature = "posix")]
pub mod posix;

/// Welche ABIs sind in diesem Build enthalten?
pub fn enabled() -> &'static str {
    #[cfg(feature = "posix")]
    {
        "karst-native + posix (abwaehlbar)"
    }
    #[cfg(not(feature = "posix"))]
    {
        "karst-native (POSIX nicht einkompiliert)"
    }
}
