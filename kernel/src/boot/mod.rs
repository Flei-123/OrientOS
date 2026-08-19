//! Schicht **boot** — alles, was von einem konkreten Bootprotokoll abhaengt.
//!
//! Der Rest des Kernels sieht nur die neutralen Typen, die hier herauskommen
//! ([`osum_mem::region::MemoryRegion`], [`limine::FramebufferInfo`]). Ein
//! Wechsel des Bootprotokolls (z. B. auf Multiboot2 fuer Altgeraete) betrifft
//! ausschliesslich dieses Verzeichnis.

pub mod limine;
