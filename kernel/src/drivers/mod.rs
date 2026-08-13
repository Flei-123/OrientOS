//! Schicht **drivers** — Geraetetreiber hinter Traits.
//!
//! Treiber duerfen `arch` benutzen (Portzugriffe, MMIO), aber nichts von `abi`
//! wissen. Sie melden sich ueber Traits aus `kcore` an — die Framebuffer-Konsole
//! z. B. ueber [`crate::kcore::print::TextSink`], sodass `println!` sie
//! automatisch mitbedient, sobald sie da ist.

pub mod fbcon;
pub mod font;
