//! **Die einzige Stelle im Quelltext, an der ein Produktname steht.**
//!
//! Alle Namen kommen aus Cargo-Metadaten, eingespeist von `kernel/build.rs`:
//! * Kernelname  = Cargo-Paketname des Kernels,
//! * OS-Name     = `[package.metadata.branding] os-name` in `kernel/Cargo.toml`
//!   (oder Umgebungsvariable `OS_NAME_OVERRIDE`, oder abgeleitet).
//!
//! Wer das Projekt umbenennen will, aendert **nicht** den Quelltext, sondern
//! fuehrt `./rename.sh <kernel> <os>` aus — siehe RENAME.md. Dass hier wirklich
//! die einzige Stelle ist, prueft `./test.sh` in einem eigenen Schritt.
//!
//! Regel fuer alle anderen Dateien: kein Produktname als Literal. Statt
//! `"karst laeuft"` schreibt man `"{} laeuft", branding::KERNEL_NAME`.

/// Name des Kernels, klein geschrieben (Cargo-Paketname).
pub const KERNEL_NAME: &str = env!("BRANDING_KERNEL_NAME");

/// Name des Betriebssystems.
pub const OS_NAME: &str = env!("BRANDING_OS_NAME");

/// Version aus `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Praefix jeder Boot-Logzeile, z. B. `[karst]`.
pub const LOG_TAG: &str = concat!("[", env!("BRANDING_KERNEL_NAME"), "]");

/// Name der nativen ABI, z. B. `karst-native`.
pub const NATIVE_ABI: &str = concat!(env!("BRANDING_KERNEL_NAME"), "-native");

/// Kopfzeile fuer Banner und Panics, z. B. `karst v0.1.0 — Kernel von Karstos`.
pub fn banner() -> impl core::fmt::Display {
    struct Banner;
    impl core::fmt::Display for Banner {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{KERNEL_NAME} v{VERSION} — Kernel von {OS_NAME}")
        }
    }
    Banner
}
