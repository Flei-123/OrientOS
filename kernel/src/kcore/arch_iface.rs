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

/// Schutzmerkmale, die die CPU meldet und die fuer die unprivilegierte Ebene
/// zaehlen.
///
/// Der Core entscheidet daraus nur "kann ich Userspace starten" und schreibt
/// das Ergebnis ins Boot-Log; WIE die Merkmale ermittelt werden, weiss allein
/// `arch`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CpuFeatures {
    /// Schneller Systemaufrufpfad vorhanden.
    pub fast_syscall: bool,
    /// Der Kernel darf keinen unprivilegierten Code ausfuehren.
    pub exec_guard: bool,
    /// Der Kernel darf unprivilegierte Daten nicht ungebremst anfassen.
    pub access_guard: bool,
    /// Per-CPU-Basisregister vorhanden.
    pub percpu_base: bool,
}

impl CpuFeatures {
    /// Reicht das Gemeldete aus, um ueberhaupt in die unprivilegierte Ebene zu
    /// wechseln?
    pub const fn user_capable(self) -> bool {
        self.fast_syscall && self.percpu_base
    }
}

/// Ergebnis eines Ausflugs in die unprivilegierte Ebene.
///
/// Die Felder sind bewusst architekturneutral benannt (`code_selector` statt
/// des x86-Registernamens), damit der Core sie protokollieren kann, ohne eine
/// Architektur zu kennen.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct UserExit {
    /// Wurde die unprivilegierte Ebene tatsaechlich betreten?
    pub entered: bool,
    /// Beendigungscode, den das Programm gemeldet hat.
    pub code: u64,
    /// Codesegment-Selektor, mit dem das Programm lief.
    pub code_selector: u16,
    /// Gemessene Privilegstufe waehrend des Laufs (3 = unprivilegiert).
    pub privilege_level: u8,
    /// Wie viele Systemaufrufe das Programm abgesetzt hat.
    pub syscalls: u64,
    /// Klartextgrund, falls `entered == false`.
    pub reason: &'static str,
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

    /// Oberes Ende des Notfallstapels, auf dem die schwerste Ausnahme laeuft
    /// (x86_64: `#DF` auf IST 1). Nur fuer Diagnoseausgaben im Boot-Log.
    /// `None`, wenn die Architektur so etwas nicht kennt.
    ///
    /// Steht bewusst im Trait: ohne diese Methode muesste `main.rs` direkt
    /// `arch::x86_64::gdt::double_fault_stack_top()` aufrufen — genau das
    /// waere ein Leck der arch-Grenze.
    fn fault_stack_top() -> Option<VirtAddr>;

    // ------------------------------------------------------------ Praeemption
    //
    // Verdraengung braucht einen Interruptpfad, der den VOLLEN Registersatz
    // rettet — welcher das ist, weiss nur `arch`. Der Core sagt lediglich, ob
    // er verdraengt werden will, und bekommt bei jedem Zeitgebertick den
    // Rueckruf [`crate::kcore::preempt::on_tick`].

    /// Schaltet den verdraengenden Zeitgeberpfad ein oder aus.
    ///
    /// Liefert den Zustand DANACH: `false` heisst "diese Architektur kann es
    /// (noch) nicht" — der Core faellt dann auf den kooperativen Planer zurueck.
    ///
    /// # Safety
    /// Veraendert den Interruptpfad; nur mit gueltigem Scheduler aufrufen.
    unsafe fn set_preemption(_on: bool) -> bool {
        false
    }

    /// Kann diese Architektur ueberhaupt verdraengen?
    fn preemption_available() -> bool {
        false
    }

    // ------------------------------------------------------- unprivilegierte Ebene

    /// Was die CPU an Schutzmerkmalen meldet.
    fn cpu_features() -> CpuFeatures {
        CpuFeatures::default()
    }

    /// Richtet den Systemaufrufpfad, den Per-CPU-Zustand und die Schutzbits
    /// ein. Liefert `false`, wenn die Architektur keine Ring-3-Ebene anbietet.
    ///
    /// # Safety
    /// Genau einmal im Boot, nach [`ArchOps::init_cpu`] und nach dem Heap.
    unsafe fn init_user_support() -> bool {
        false
    }

    /// Ist der Weg in die unprivilegierte Ebene fertig verdrahtet?
    fn user_support_ready() -> bool {
        false
    }

    /// Hinterlegt den Kernelstapel, auf den ein Systemaufruf bzw. eine
    /// Ausnahme aus der unprivilegierten Ebene umschaltet.
    fn set_kernel_stack(_top: VirtAddr) {}

    /// Startet `entry` unprivilegiert auf dem Stapel `stack_top` und kehrt
    /// zurueck, sobald sich das Programm beendet hat.
    ///
    /// # Safety
    /// `entry` und `stack_top` muessen im aktiven Adressraum unprivilegiert
    /// erreichbar abgebildet sein.
    unsafe fn enter_user(_entry: VirtAddr, _stack_top: VirtAddr) -> UserExit {
        UserExit { reason: "Architektur kennt keine unprivilegierte Ebene", ..UserExit::default() }
    }

    /// Codesegment-Selektor des gerade laufenden Kontrollflusses (Diagnose).
    fn code_selector() -> u16 {
        0
    }
}
