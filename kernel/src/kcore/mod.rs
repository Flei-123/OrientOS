//! Schicht **core** — architekturneutrale Kernlogik und die Typen, ueber die
//! sich `arch`, `mm`, `drivers` und `abi` unterhalten.
//!
//! Regeln fuer diese Schicht (werden bei Reviews geprueft):
//! * kein Assembler, kein Port-IO, keine Register- oder Tabellenbitnamen
//!   irgendeiner Architektur (die automatische Probe in PREFLIGHT.md sucht
//!   genau danach — auch in Kommentaren),
//! * keine POSIX-Semantik (kein errno, kein fork, keine Signale),
//! * nur `core`/`alloc` und die neutralen Workspace-Crates.
//!
//! Modul- und Verzeichnisname lauten `kcore`, weil `core` bereits der Name der
//! Standard-Kern-Crate von Rust ist und ein Modul `core` im Crate-Root die
//! Pfadaufloesung (`core::fmt` usw.) mehrdeutig macht.

pub mod arch_iface;
pub mod branding;
pub mod mem;
pub mod panic;
pub mod print;
pub mod sched;
