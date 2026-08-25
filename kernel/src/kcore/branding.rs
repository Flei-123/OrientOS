//! **Die einzige Stelle im Quelltext, an der ein Produktname steht.**
//!
//! Alle Werte kommen von `kernel/build.rs`, das sie in dieser Reihenfolge
//! bezieht: einzelne Umgebungsvariablen (`OS_NAME_OVERRIDE` usw.) →
//! `brands/<BRAND>.toml` → `[package.metadata.branding]` in
//! `kernel/Cargo.toml` → Ableitung aus dem Cargo-Paketnamen.
//!
//! **Zwei Wege, das Produkt umzubenennen:**
//!
//! * **Zweitmarke, ohne den Baum anzufassen** (der Normalfall):
//!   `./build.sh --brand xoffi` — liest `brands/xoffi.toml`. Derselbe
//!   Quelltext ergibt ein anders benanntes System. Siehe `BRANDING.md`.
//! * **Endgueltig umbenennen** (auch Verzeichnisse und Doku):
//!   `./rename.sh <kernel> <os>` — siehe `RENAME.md`.
//!
//! Dass hier wirklich die einzige Stelle ist, prueft `./test.sh` in einem
//! eigenen Schritt.
//!
//! Regel fuer alle anderen Dateien: kein Produktname als Literal. Statt
//! `"osum laeuft"` schreibt man `"{} laeuft", branding::KERNEL_NAME`.

/// Name des Kernels, klein geschrieben (normalerweise der Cargo-Paketname).
///
/// Bleibt ueber Marken hinweg gleich: eine Zweitmarke aendert das Produkt,
/// nicht den Kernel — wie NT unter jeder Windows-Ausgabe.
pub const KERNEL_NAME: &str = env!("BRANDING_KERNEL_NAME");

/// Name des Betriebssystems, wie ihn ein Mensch zu sehen bekommt.
pub const OS_NAME: &str = env!("BRANDING_OS_NAME");

/// Kurzname der Marke, klein geschrieben. Fuer Dateinamen (`<slug>.iso`),
/// Verzeichnisse und alles, was maschinenlesbar sein muss.
pub const SLUG: &str = env!("BRANDING_SLUG");

/// Herausgeber. Leer, wenn die Marke keinen angibt.
pub const PUBLISHER: &str = env!("BRANDING_PUBLISHER");

/// Oeffentliche Adresse der Marke. Leer, wenn nicht gesetzt.
pub const WEB: &str = env!("BRANDING_WEB");

/// Paketquelle dieser Marke. Eigener Feed je Marke — ein XoffiOS darf sich
/// nie zu einem OrientOS „aktualisieren".
pub const FEED: &str = env!("BRANDING_FEED");

/// Version aus `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Praefix jeder Boot-Logzeile, z. B. `[osum]`.
pub const LOG_TAG: &str = concat!("[", env!("BRANDING_KERNEL_NAME"), "]");

/// Name der nativen ABI, z. B. `osum-native`.
pub const NATIVE_ABI: &str = concat!(env!("BRANDING_KERNEL_NAME"), "-native");

/// Kopfzeile fuer Banner und Panics, z. B. `osum v0.1.0 — Kernel von OrientOS`.
pub fn banner() -> impl core::fmt::Display {
    struct Banner;
    impl core::fmt::Display for Banner {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{KERNEL_NAME} v{VERSION} — Kernel von {OS_NAME}")
        }
    }
    Banner
}
