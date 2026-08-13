//! Unprivilegierte Ebene — die core-Seite.
//!
//! **Schicht: core.** Diese Datei entscheidet, OB und WAS unprivilegiert
//! gestartet wird, und protokolliert das Ergebnis. WIE der Privilegwechsel
//! ausgefuehrt wird (Systemaufrufpfad, Per-CPU-Block, Kernelstapel), steht
//! ausschliesslich in `arch/<name>/user.rs` hinter den Traitmethoden
//! [`ArchOps::init_user_support`] und [`ArchOps::enter_user`].
//!
//! Der Nachweis, dass wirklich unprivilegiert gelaufen wird, besteht aus drei
//! Angaben im Boot-Log, die alle aus [`UserExit`] stammen:
//! Codesegment-Selektor, gemessene Privilegstufe und der benutzte Stapel.
//! Dazu kommt ein NEGATIVER Test: ein Zugriff des unprivilegierten Programms
//! auf eine Kerneladresse muss sauber als Schutzverletzung gemeldet werden und
//! darf den Kernel nicht anhalten.

use crate::kcore::arch_iface::{ArchOps, CpuFeatures, UserExit};
use crate::kcore::mem::VirtAddr;
use crate::klog;

/// Untere Grenze des unprivilegierten Adressbereichs.
pub const USER_BASE: u64 = 0x0000_0000_0040_0000;
/// Obere Grenze des unprivilegierten Adressbereichs (ausschliesslich).
/// Alles darueber gehoert dem Kernel und darf aus Ring 3 nie erreichbar sein.
pub const USER_LIMIT: u64 = 0x0000_8000_0000_0000;

/// Liegt `addr` im unprivilegierten Adressbereich?
pub const fn is_user_addr(addr: u64) -> bool {
    addr >= USER_BASE && addr < USER_LIMIT
}

/// Meldet die Schutzmerkmale der CPU ins Boot-Log und liefert sie zurueck.
pub fn report_features() -> CpuFeatures {
    let f = <crate::arch::Active as ArchOps>::cpu_features();
    klog!(
        "user",
        "CPU-Schutz  : Schnellaufruf {}, Ausfuehrsperre {}, Zugriffssperre {}, Per-CPU-Basis {}",
        yesno(f.fast_syscall),
        yesno(f.exec_guard),
        yesno(f.access_guard),
        yesno(f.percpu_base)
    );
    f
}

fn yesno(b: bool) -> &'static str {
    if b {
        "ja"
    } else {
        "nein (uebersprungen)"
    }
}

/// Richtet den Weg in die unprivilegierte Ebene ein.
///
/// # Safety
/// Genau einmal im Boot, nach der CPU-Einrichtung und nach dem Heap.
pub unsafe fn init() -> bool {
    let ok = unsafe { <crate::arch::Active as ArchOps>::init_user_support() };
    klog!(
        "user",
        "Systemaufrufpfad: {}",
        if ok { "eingerichtet" } else { "nicht verfuegbar — kein Ring 3 in diesem Build" }
    );
    ok
}

/// Startet ein unprivilegiertes Programm und liefert, wie es ausgegangen ist.
///
/// # Safety
/// `entry` und `stack_top` muessen im aktiven Adressraum unprivilegiert
/// erreichbar abgebildet sein.
pub unsafe fn run(entry: VirtAddr, stack_top: VirtAddr) -> UserExit {
    unsafe { <crate::arch::Active as ArchOps>::enter_user(entry, stack_top) }
}

/// Schreibt das Ergebnis eines Ausflugs in die unprivilegierte Ebene so ins
/// Log, dass es maschinell pruefbar ist.
pub fn report_exit(x: &UserExit) {
    if x.entered {
        klog!(
            "user",
            "Ring 3      : CS={:#06x} (RPL={}), CPL={}, Stapel={:#018x}, {} Systemaufruf(e), Ende {}",
            x.code_selector,
            x.code_selector & 3,
            x.privilege_level,
            x.stack_pointer,
            x.syscalls,
            x.code
        );
    } else {
        klog!("user", "Ring 3      : nicht betreten — {}", x.reason);
    }
}

/// Boot-Selbsttest der unprivilegierten Ebene.
///
/// Liefert `(bestanden, gesamt)` fuer die Selbsttestbilanz in `kmain`.
///
/// **Baustelle des Moduls "ring3".** Solange die Architektur keinen
/// Privilegwechsel anbietet, meldet diese Funktion das ehrlich und zaehlt
/// nichts — der Boot bleibt gruen.
pub fn boot_selftest(_space: &mut crate::arch::AddressSpace) -> (usize, usize) {
    let f = report_features();
    if !f.user_capable() || !<crate::arch::Active as ArchOps>::user_support_ready() {
        klog!("user", "Ring 3      : nicht betreten — Architektur meldet keine Unterstuetzung");
        return (0, 0);
    }
    (0, 0)
}
