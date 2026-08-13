//! **Die arch-Grenze.** Hier stehen die Traits, die eine Architektur erfuellen
//! muss — und sonst nichts. Kein `asm!`, kein Registername, kein Portzugriff.
//!
//! Der Rest des Kernels (`mm`, `drivers`, `abi`, `kcore`) ruft ausschliesslich
//! ueber diese Traits bzw. ueber die duenne Fassade in [`crate::arch`] hinein.
//! Eine neue Architektur (aarch64) besteht damit aus genau einer neuen Datei-
//! sammlung unter `arch/<name>/` plus einer `impl`-Zeile — der Core bleibt
//! unveraendert.
//!
//! Pruefregel fuer Reviews: taucht in `mm/`, `drivers/`, `kcore/` oder `abi/`
//! ein x86-Begriff auf (CR3, PML4, PTE, `out`, `lidt`, ...), ist die Grenze
//! verletzt.

use crate::kcore::mem::{FrameAllocator, MapError, MapFlags, PhysAddr, VirtAddr};

/// Ein Adressraum (Seitentabellenbaum) einer Architektur.
///
/// `mm` entscheidet, WAS abgebildet wird; die Implementierung weiss, WIE.
pub trait AddressSpaceOps: Sized {
    /// Groesse einer grossen Seite (x86_64: 2 MiB, aarch64 4K-Granule: 2 MiB).
    const LARGE_PAGE_SIZE: usize;

    /// Legt einen leeren Adressraum an (Wurzeltabelle genullt).
    ///
    /// # Safety
    /// Der Allocator muss gueltige, exklusive Frames liefern.
    unsafe fn new_empty(alloc: &mut dyn FrameAllocator) -> Result<Self, MapError>;

    /// Uebernimmt den aktuell aktiven Adressraum (den des Bootloaders).
    ///
    /// # Safety
    /// Nur im fruehen Boot aufrufen, solange die Tabellen noch gueltig sind.
    unsafe fn current() -> Self;

    /// Physische Adresse der Wurzeltabelle.
    fn root(&self) -> PhysAddr;

    /// Bildet eine Basisseite ab.
    ///
    /// # Safety
    /// Veraendert die Speichersicht der CPU.
    unsafe fn map(
        &mut self,
        virt: VirtAddr,
        phys: PhysAddr,
        flags: MapFlags,
        alloc: &mut dyn FrameAllocator,
    ) -> Result<(), MapError>;

    /// Bildet eine grosse Seite ([`Self::LARGE_PAGE_SIZE`]) ab.
    ///
    /// # Safety
    /// Wie [`AddressSpaceOps::map`].
    unsafe fn map_large(
        &mut self,
        virt: VirtAddr,
        phys: PhysAddr,
        flags: MapFlags,
        alloc: &mut dyn FrameAllocator,
    ) -> Result<(), MapError>;

    /// Entfernt eine Abbildung und liefert den freigewordenen Frame.
    ///
    /// # Safety
    /// Der Aufrufer garantiert, dass die Seite nicht mehr benutzt wird.
    unsafe fn unmap(&mut self, virt: VirtAddr) -> Result<PhysAddr, MapError>;

    /// Loest eine virtuelle Adresse auf (auch innerhalb grosser Seiten).
    fn translate(&self, virt: VirtAddr) -> Option<PhysAddr>;

    /// Macht diesen Adressraum zum aktiven.
    ///
    /// # Safety
    /// Der Adressraum MUSS den laufenden Code, den Stack und alle gerade
    /// benutzten Daten enthalten — sonst folgt ein Triple Fault.
    unsafe fn activate(&self);
}

/// Gesicherter Zustand eines Kontrollflusses (Threadkontext).
///
/// Der Core weiss nur, dass es so ein Ding gibt, dass man es fuer einen
/// eigenen Stapel aufbauen und zwischen zweien umschalten kann. WELCHE
/// Register dabei gerettet werden, steht ausschliesslich in
/// `arch/<name>/context.rs`.
pub trait ContextOps: Sized + Copy {
    /// Kleinster Stapel, auf dem [`ContextOps::new`] noch sinnvoll arbeitet.
    const MIN_STACK_BYTES: usize;

    /// Noch nie gelaufener, leerer Kontext. Bekommt beim ersten Wechsel den
    /// Zustand des gerade laufenden Kontrollflusses eingetragen.
    fn empty() -> Self;

    /// Baut einen Startkontext, der bei `entry` auf dem Stapel unterhalb von
    /// `stack_top` beginnt.
    ///
    /// # Safety
    /// `stack_top` muss das obere Ende eines exklusiv besessenen, mindestens
    /// [`ContextOps::MIN_STACK_BYTES`] grossen, beschreibbaren Bereichs sein.
    unsafe fn new(stack_top: VirtAddr, entry: extern "C" fn() -> !) -> Self;

    /// Aktueller Stapelzeiger des gesicherten Kontextes (fuer Diagnose).
    fn stack_pointer(&self) -> VirtAddr;

    /// Sichert den laufenden Zustand nach `from` und setzt `to` fort.
    ///
    /// # Safety
    /// `to` muss ein gueltiger, gerade nicht laufender Kontext sein, dessen
    /// Stapel noch existiert; `from` muss beschreibbar sein. Der Aufruf kehrt
    /// erst zurueck, wenn jemand wieder nach `from` wechselt.
    unsafe fn switch(from: *mut Self, to: *const Self);
}

/// Periodischer Systemzeitgeber.
pub trait TimerOps {
    /// Startet den Zeitgeber mit `hz` Ticks pro Sekunde.
    ///
    /// # Safety
    /// Programmiert Hardware; nur einmal im Boot aufrufen.
    unsafe fn start(hz: u32);
    /// Bisher gezaehlte Ticks.
    fn ticks() -> u64;
    /// Eingestellte Frequenz.
    fn hz() -> u32;
}

/// Interruptcontroller (x86_64: 8259-PIC, spaeter APIC).
pub trait InterruptCtlOps {
    /// Initialisiert und maskiert alle Leitungen.
    ///
    /// # Safety
    /// Programmiert Hardware.
    unsafe fn init();
    /// Quittiert einen Interrupt.
    ///
    /// # Safety
    /// Nur aus einem Interrupt-Handler heraus aufrufen.
    unsafe fn eoi(irq: u8);
    /// Maskiert bzw. demaskiert eine Leitung.
    ///
    /// # Safety
    /// Programmiert Hardware.
    unsafe fn set_masked(irq: u8, masked: bool);
    /// Name des aktiven Controllers (fuer Boot-Logs).
    fn name() -> &'static str;
}

/// Alles, was der Kernel von einer Architektur braucht.
pub trait ArchOps {
    /// Anzeigename, z. B. `"x86_64"`.
    const NAME: &'static str;
    /// Groesse einer Basisseite.
    const PAGE_SIZE: usize;

    /// Adressraum-Implementierung dieser Architektur.
    type AddressSpace: AddressSpaceOps;
    /// Threadkontext dieser Architektur (Registerrettung).
    type Context: ContextOps;
    /// Zeitgeber dieser Architektur.
    type Timer: TimerOps;
    /// Interruptcontroller dieser Architektur.
    type IntCtl: InterruptCtlOps;

    /// Bringt die serielle Debugkonsole zum Laufen — der allererste Schritt.
    ///
    /// # Safety
    /// Genau einmal ganz am Anfang von `kmain`.
    unsafe fn init_serial();
    /// Gibt Text auf der seriellen Konsole aus.
    fn serial_write(s: &str);

    /// Richtet die CPU-Grundstrukturen ein (Deskriptortabellen, Traps).
    ///
    /// # Safety
    /// Nach [`ArchOps::init_serial`], vor allem anderen.
    unsafe fn init_cpu();

    /// Sperrt Interrupts.
    fn disable_interrupts();
    /// Gibt Interrupts frei.
    fn enable_interrupts();
    /// Sind Interrupts gerade freigegeben?
    fn interrupts_enabled() -> bool;
    /// Haelt die CPU bis zum naechsten Interrupt an (stromsparendes Idle).
    fn wait_for_interrupt();
    /// Haelt die CPU endgueltig an.
    fn halt_forever() -> !;

    /// Laeuft die Frame-Pointer-Kette ab und meldet jede Ruecksprungadresse.
    fn backtrace(f: &mut dyn FnMut(u64));

    /// Schreibt den CPU-Namen nach `buf` und liefert die Laenge.
    fn cpu_brand(buf: &mut [u8]) -> usize;
}
