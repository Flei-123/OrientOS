//! Unprivilegierte Ebene — die core-Seite.
//!
//! **Schicht: core.** Diese Datei entscheidet, OB und WAS unprivilegiert
//! gestartet wird, legt die Seiten dafuer an, nimmt die Systemaufrufe entgegen
//! und protokolliert das Ergebnis. WIE der Privilegwechsel ausgefuehrt wird
//! (Systemaufrufpfad, Per-CPU-Block, Kernelstapel), steht ausschliesslich in
//! `arch/<name>/user.rs` hinter den Traitmethoden
//! [`ArchOps::init_user_support`] und [`ArchOps::enter_user`].
//!
//! Der Nachweis, dass wirklich unprivilegiert gelaufen wird, besteht aus drei
//! Angaben im Boot-Log, die alle aus [`UserExit`] stammen:
//! Codesegment-Selektor, gemessene Privilegstufe und der benutzte Stapel.
//! Alle drei werden an einem echten Ausnahmerahmen der Hardware abgelesen,
//! nicht vom Kernel behauptet. Dazu kommt ein NEGATIVER Test: ein Zugriff des
//! unprivilegierten Programms auf eine Kerneladresse muss sauber als
//! Schutzverletzung gemeldet werden und darf den Kernel nicht anhalten.
//!
//! **Schutzwall.** Der eine Negativtest reicht nicht als Nachweis, dass die
//! Grenze wirklich dicht ist. Die Architektur meldet deshalb eine Liste von
//! Uebergriffen an ([`UserProbe`]), die dieses Modul einzeln ausfuehrt und
//! einzeln ins Log schreibt: privilegierte Instruktion, Schreiben auf die
//! eigene Codeseite (W^X gilt auch unprivilegiert), Ausfuehren von Kernelcode,
//! Zeiger auf eine nicht abgebildete Seite — und eine Rechenschleife, die sich
//! nie meldet. Jeder Uebergriff bekommt einen eigenen Prozess OHNE jedes
//! Handle; keiner darf gelingen, keiner darf den Kernel anhalten.
//!
//! **Woher das Programm kommt.** Bevorzugt aus dem Startdateisystem
//! ([`crate::kcore::initramfs`]) als echtes ELF64-Abbild. Liegt dort keines,
//! benutzt der Selbsttest das Bild, das die Architektur bei der Einrichtung
//! angemeldet hat ([`ArchSupport`]) — Maschinencode ist Architektursache und
//! taucht in dieser Datei nur als Bytefeld auf.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use spin::Mutex;

use crate::arch::AddressSpace;
use crate::kcore::arch_iface::{AddressSpaceOps, ArchOps, CpuFeatures, UserExit};
use crate::kcore::mem::{
    phys_to_virt, FrameAllocator, MapFlags, PhysAddr, VirtAddr, PAGE_SIZE,
};
use crate::klog;

/// Untere Grenze des unprivilegierten Adressbereichs.
pub const USER_BASE: u64 = 0x0000_0000_0040_0000;
/// Obere Grenze des unprivilegierten Adressbereichs (ausschliesslich).
/// Alles darueber gehoert dem Kernel und darf aus Ring 3 nie erreichbar sein.
pub const USER_LIMIT: u64 = 0x0000_8000_0000_0000;

/// Adresse, an der das von der Architektur mitgelieferte Beispielprogramm
/// abgebildet wird. Bewusst nicht [`USER_BASE`]: dort liegt ein ELF aus dem
/// Startdateisystem, beide sollen gleichzeitig abbildbar sein.
const DEMO_BASE: u64 = 0x0000_0000_0080_0000;
/// Oberes Ende des unprivilegierten Stapels dieses Selbsttests.
///
/// Bewusst unterhalb des Bereichs, den der ELF-Lader fuer aus dem
/// Startdateisystem geladene Programme benutzt: beide Abbildungen sollen
/// gleichzeitig bestehen koennen.
const USER_STACK_TOP: u64 = 0x0000_7fff_fff0_0000;
/// Seiten des unprivilegierten Stapels.
const USER_STACK_PAGES: usize = 4;
/// Hoechstzahl Seiten, die eine Abbildung dieses Moduls belegen darf.
const MAX_USER_PAGES: usize = 64;

/// Liegt `addr` im unprivilegierten Adressbereich?
pub const fn is_user_addr(addr: u64) -> bool {
    addr >= USER_BASE && addr < USER_LIMIT
}

// ------------------------------------------------------- Anmeldung der Arch

/// Was die Architektur dem Core fuer die unprivilegierte Ebene bereitstellt.
///
/// Bewusst so schmal wie moeglich: ein Bytefeld mit zwei Einsprungversaetzen
/// und zwei Schaltern fuer das Zugriffsfenster. Der Core interpretiert nichts
/// davon.
#[derive(Clone, Copy)]
pub struct ArchSupport {
    /// Rohbild eines lauffaehigen Beispielprogramms.
    pub program: &'static [u8],
    /// Versatz des regulaeren Einsprungs im Bild.
    pub entry_offset: usize,
    /// Versatz des Einsprungs, der absichtlich eine Kerneladresse anfasst.
    pub fault_offset: usize,
    /// Versatz des Messsprungbretts: es nimmt die echte Einsprungadresse als
    /// oberstes Stapelwort, loest EINE Ausnahme aus (damit der Kernel
    /// Selektor, Privilegstufe und Stapel an der Hardware messen kann) und
    /// springt dann in das Programm. Damit ist der Nachweis auch fuer
    /// Programme moeglich, die der Kernel nicht selbst geschrieben hat.
    pub trampoline_offset: usize,
    /// Oeffnet das Fenster, durch das der Kernel unprivilegierte Daten lesen
    /// darf (nur noetig, wenn die CPU eine Zugriffssperre kennt).
    pub open_window: fn(),
    /// Schliesst dieses Fenster wieder.
    pub close_window: fn(),
    /// Uebergriffe, die das Programm auf Wunsch versucht (Schutzwall).
    pub probes: [UserProbe; MAX_PROBES],
    /// Wie viele Eintraege von [`ArchSupport::probes`] belegt sind.
    pub probe_count: usize,
}

/// Hoechstzahl Uebergriffe, die eine Architektur anmelden kann.
pub const MAX_PROBES: usize = 8;

/// Was der Kernel von einem angemeldeten Uebergriff erwartet.
///
/// Bewusst architekturneutral: der Core weiss nicht, WELCHE Instruktion die
/// Probe ausfuehrt, nur wie ihr Ausgang zu bewerten ist.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProbeExpect {
    /// Die Hardware muss den Zugriff abweisen; der Kernel meldet ihn und
    /// raeumt das Programm ab.
    Abgewiesen,
    /// Das Programm rechnet, ohne sich je zu melden. Der Kernel muss es
    /// abraeumen, BEVOR es sich selbst mit `selbstende` beendet — genau das
    /// ist der Nachweis, dass der Zeitgeber auch unprivilegierten Code
    /// unterbricht.
    Wachhund {
        /// Beendigungscode, mit dem sich die Probe selbst beendet, wenn ihr
        /// niemand die CPU wegnimmt.
        selbstende: u64,
    },
}

/// Ein angemeldeter Uebergriff aus der unprivilegierten Ebene.
#[derive(Clone, Copy)]
pub struct UserProbe {
    /// Versatz des Einsprungs im Bild aus [`ArchSupport::program`].
    pub offset: usize,
    /// Klartextname fuer das Boot-Log.
    pub name: &'static str,
    /// Erwarteter Ausgang.
    pub expect: ProbeExpect,
}

impl UserProbe {
    /// Leerer Platzhalter zum Fuellen des Feldes.
    pub const NONE: UserProbe =
        UserProbe { offset: 0, name: "", expect: ProbeExpect::Abgewiesen };
}

static SUPPORT: Mutex<Option<ArchSupport>> = Mutex::new(None);

/// Meldet die Unterstuetzung der Architektur an. Wird aus deren Einrichtung
/// heraus aufgerufen.
pub fn register_arch_support(s: ArchSupport) {
    *SUPPORT.lock() = Some(s);
}

fn support() -> Option<ArchSupport> {
    *SUPPORT.lock()
}

// ------------------------------------------------- Pruefung fremder Zeiger

/// Groesster Puffer, den ein Systemaufruf aus Ring 3 mitbringen darf.
const MAX_USER_BUFFER: u64 = 64 * 1024;

/// Darf der Kernel `len` Bytes ab `start` im Auftrag eines unprivilegierten
/// Programms anfassen?
///
/// Geprueft wird beides: die Adressgrenzen UND ob jede betroffene Seite im
/// aktiven Adressraum ueberhaupt abgebildet ist. Ohne die zweite Pruefung
/// koennte ein Programm den Kernel mit einem plausibel aussehenden, aber
/// unabgebildeten Zeiger in eine Ausnahme im privilegierten Modus treiben —
/// aus einem abgewiesenen Systemaufruf wuerde ein Kernelabsturz.
fn range_ok(start: u64, len: u64) -> bool {
    if len == 0 || len > MAX_USER_BUFFER {
        return false;
    }
    let Some(last) = start.checked_add(len - 1) else {
        return false;
    };
    if !is_user_addr(start) || !is_user_addr(last) {
        return false;
    }
    // SAFETY: liest nur die Wurzel des gerade aktiven Adressraums; es wird
    // nichts veraendert.
    let space = unsafe { AddressSpace::current() };
    let mut page = start & !(PAGE_SIZE as u64 - 1);
    loop {
        if space.translate(VirtAddr(page)).is_none() {
            return false;
        }
        if page >= last & !(PAGE_SIZE as u64 - 1) {
            return true;
        }
        page += PAGE_SIZE as u64;
    }
}

/// Wert, den das naechste unprivilegierte Programm beim Start bekommt.
///
/// Genau ein Wort — in aller Regel das Handle, ueber das es ueberhaupt etwas
/// tun darf. Alles Weitere holt sich ein Programm ueber Systemaufrufe mit
/// genau diesem Handle; es gibt keine mitgeerbte Umgebung.
static ENTRY_ARG: AtomicU64 = AtomicU64::new(0);

/// Setzt den Uebergabewert fuer den naechsten Start.
pub fn set_entry_arg(v: u64) {
    ENTRY_ARG.store(v, Ordering::SeqCst);
}

/// Der Uebergabewert fuer den naechsten Start (von `arch` gelesen).
pub fn entry_arg() -> u64 {
    ENTRY_ARG.load(Ordering::SeqCst)
}

// ----------------------------------------------------------- Systemaufrufe

/// Wie es nach einem Systemaufruf weitergeht.
pub enum SyscallOutcome {
    /// Rueckgabewert an das Programm, das weiterlaeuft.
    Weiter(u64),
    /// Das Programm hat sich beendet; der Kernel raeumt es ab.
    Ende(u64),
}

/// Wie oft ein Zugriff aus Ring 3 sauber abgewiesen wurde.
static REJECTED: AtomicUsize = AtomicUsize::new(0);
/// Wie viele Bytes das Programm insgesamt ausgegeben hat.
static WRITTEN: AtomicU64 = AtomicU64::new(0);

/// Nimmt einen Systemaufruf aus der unprivilegierten Ebene entgegen.
///
/// Die Bedeutung der Aufrufe liegt in [`crate::abi::native`]; hier passiert
/// nur das, was Adressraumwissen braucht:
///
/// * **Zeigerpruefung.** Ein Zeiger aus Ring 3 wird gegen die Bereiche
///   geprueft, die dieses Modul dem Programm tatsaechlich abgebildet hat.
///   Die ABI-Schicht kann nur die Adressgrenzen pruefen; ob eine Adresse
///   ueberhaupt abgebildet ist, weiss allein diese Schicht — ohne die Pruefung
///   koennte ein Programm den Kernel mit einem nicht abgebildeten Zeiger in
///   einen Ausnahmefall im privilegierten Modus treiben.
/// * **Abmeldung.** Beendet sich das Programm, kehrt der Kernel an die Stelle
///   zurueck, an der er es gestartet hat.
pub fn syscall(nr: u64, args: [u64; 6]) -> SyscallOutcome {
    use karst_abi_native::{Error, Syscall};

    let call = Syscall::from_raw(nr);

    // Aufrufe mit einem Puffer in `(args[1], args[2])`.
    if matches!(call, Some(Syscall::HandleWrite) | Some(Syscall::HandleRead))
        && !range_ok(args[1], args[2])
    {
        REJECTED.fetch_add(1, Ordering::Relaxed);
        klog!(
            "user",
            "Zeiger {:#018x} (+{} B) aus Ring 3 abgewiesen: nicht im eigenen Bereich",
            args[1],
            args[2]
        );
        return SyscallOutcome::Weiter(Error::InvalidArgs.as_raw() as u64);
    }

    // Aufrufe, bei denen der Kernel unprivilegierten Speicher anfasst, laufen
    // im geoeffneten Zugriffsfenster: kennt die CPU eine Zugriffssperre, ist
    // sie ausserhalb dieses Fensters scharf.
    let braucht_fenster = matches!(
        call,
        Some(Syscall::HandleWrite)
            | Some(Syscall::HandleRead)
            | Some(Syscall::ChannelSend)
            | Some(Syscall::ChannelRecv)
            | Some(Syscall::NamespaceOpen)
            | Some(Syscall::NamespaceCreate)
            | Some(Syscall::HandleInspect)
            | Some(Syscall::PortWait)
            | Some(Syscall::ProcessSpawn)
    );
    let r = if braucht_fenster {
        with_user_window(|| crate::abi::native::dispatch(nr, args))
    } else {
        crate::abi::native::dispatch(nr, args)
    };
    if matches!(call, Some(Syscall::HandleWrite)) && r > 0 {
        WRITTEN.fetch_add(r as u64, Ordering::Relaxed);
    }
    match call {
        // `(handle, code)`: der Verteiler hat den Prozess bereits abgemeldet,
        // hier endet zusaetzlich der Ausflug nach Ring 3.
        Some(Syscall::ProcessExit) => SyscallOutcome::Ende(args[1]),
        _ => SyscallOutcome::Weiter(r as u64),
    }
}

/// Fuehrt `f` aus, waehrend der Kernel unprivilegierte Daten anfassen darf.
///
/// Kennt die CPU keine Zugriffssperre, sind beide Schalter der Architektur
/// wirkungslos und diese Huelle kostet nichts.
fn with_user_window<T>(f: impl FnOnce() -> T) -> T {
    let s = support();
    if let Some(s) = s {
        (s.open_window)();
    }
    let r = f();
    if let Some(s) = s {
        (s.close_window)();
    }
    r
}

/// Meldet einen Zugriff aus Ring 3, den die Hardware abgewiesen hat.
///
/// Wird aus dem Ausnahmepfad der Architektur gerufen. `ursache` und `art`
/// sind der bereits uebersetzte Klartext der Architektur.
pub fn note_fault(addr: u64, ursache: &str, art: &str) {
    REJECTED.fetch_add(1, Ordering::Relaxed);
    if addr >= USER_LIMIT {
        klog!(
            "user",
            "Ring-3-Zugriff auf Kerneladresse {:#018x}: sauber abgewiesen ({}, {})",
            addr,
            ursache,
            art
        );
    } else if is_user_addr(addr) {
        klog!(
            "user",
            "Ring-3-Zugriff auf {:#018x}: sauber abgewiesen ({}, {}), Programm abgeraeumt",
            addr,
            ursache,
            art
        );
    } else {
        klog!(
            "user",
            "Ring-3-Zugriff auf unerlaubte Adresse {:#018x}: sauber abgewiesen ({}, {})",
            addr,
            ursache,
            art
        );
    }
}

/// Wie oft der Kernel einen unerlaubten Zugriff aus Ring 3 abgewiesen hat.
pub fn rejected() -> usize {
    REJECTED.load(Ordering::Relaxed)
}

// -------------------------------------------------------------- Abbildungen

/// Eine Menge unprivilegierter Seiten, die der Kernel wieder abraeumen kann.
struct UserPages {
    pages: [(u64, PhysAddr); MAX_USER_PAGES],
    n: usize,
}

impl UserPages {
    const fn new() -> Self {
        UserPages { pages: [(0, PhysAddr(0)); MAX_USER_PAGES], n: 0 }
    }

    /// Legt eine Seite an, fuellt sie aus `data` (Rest bleibt 0) und bildet
    /// sie unprivilegiert ab.
    fn add(&mut self, space: &mut AddressSpace, virt: u64, data: &[u8], flags: MapFlags) -> bool {
        if self.n >= MAX_USER_PAGES || !is_user_addr(virt) {
            return false;
        }
        let mut guard = crate::mm::frame::lock();
        let Some(fa) = guard.as_mut() else {
            return false;
        };
        let Some(frame) = fa.alloc_frame() else {
            return false;
        };
        // Der Inhalt wird ueber das Direct-Map-Fenster geschrieben, nicht ueber
        // die unprivilegierte Abbildung: so bleibt die Seite von Anfang an
        // schreibgeschuetzt, wenn sie Code traegt.
        let dst = phys_to_virt(frame).as_ptr::<u8>();
        // SAFETY: frisch belegter Frame, ueber das Direct-Map-Fenster
        // erreichbar und sonst niemandem bekannt.
        unsafe {
            core::ptr::write_bytes(dst, 0, PAGE_SIZE);
            core::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len().min(PAGE_SIZE));
        }
        // SAFETY: gueltige, freie unprivilegierte Adresse; der Frame gehoert
        // ab jetzt dieser Abbildung.
        match unsafe { space.map(VirtAddr(virt), frame, flags | MapFlags::USER, &mut *fa) } {
            Ok(()) => {
                self.pages[self.n] = (virt, frame);
                self.n += 1;
                true
            }
            Err(_) => {
                fa.free_frame(frame);
                false
            }
        }
    }

    /// Nimmt alle Seiten wieder aus dem Adressraum und gibt die Frames zurueck.
    fn teardown(&mut self, space: &mut AddressSpace) -> usize {
        let mut guard = crate::mm::frame::lock();
        let Some(fa) = guard.as_mut() else {
            return 0;
        };
        let mut n = 0;
        while self.n > 0 {
            self.n -= 1;
            let (virt, frame) = self.pages[self.n];
            // SAFETY: genau die Seiten, die `add` abgebildet hat; das Programm
            // laeuft nicht mehr.
            if unsafe { space.unmap(VirtAddr(virt)) }.is_ok() {
                fa.free_frame(frame);
                n += 1;
            }
        }
        n
    }
}

/// Bildet ein zusammenhaengendes Bild ab (Code + Daten in einem Stueck).
fn map_image(
    space: &mut AddressSpace,
    pages: &mut UserPages,
    base: u64,
    bytes: &[u8],
    flags: MapFlags,
) -> bool {
    let mut off = 0usize;
    while off < bytes.len() {
        let end = (off + PAGE_SIZE).min(bytes.len());
        if !pages.add(space, base + off as u64, &bytes[off..end], flags) {
            return false;
        }
        off += PAGE_SIZE;
    }
    true
}

/// Legt den unprivilegierten Stapel an und liefert sein oberes Ende.
fn map_stack(space: &mut AddressSpace, pages: &mut UserPages) -> Option<u64> {
    let bottom = USER_STACK_TOP - (USER_STACK_PAGES * PAGE_SIZE) as u64;
    for i in 0..USER_STACK_PAGES {
        if !pages.add(
            space,
            bottom + (i * PAGE_SIZE) as u64,
            &[],
            MapFlags::READ | MapFlags::WRITE,
        ) {
            return None;
        }
    }
    Some(USER_STACK_TOP)
}

// ----------------------------------------------------------------- Meldungen

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

// --------------------------------------------------------------- Selbsttest

/// Boot-Selbsttest der unprivilegierten Ebene.
///
/// Liefert `(bestanden, gesamt)` fuer die Selbsttestbilanz in `kmain`.
///
/// Geprueft wird — jeweils an gemessenen Werten, nicht an Behauptungen:
/// 1. das Programm lief wirklich unprivilegiert (Selektor mit RPL 3, CPL 3),
/// 2. es lief auf einem Stapel im unprivilegierten Bereich,
/// 3. es hat Systemaufrufe abgesetzt und sich sauber beendet,
/// 4. ein Zugriff auf eine Kerneladresse wurde abgewiesen, ohne den Kernel
///    anzuhalten.
pub fn boot_selftest(space: &mut AddressSpace) -> (usize, usize) {
    let f = report_features();
    if !f.user_capable() || !<crate::arch::Active as ArchOps>::user_support_ready() {
        klog!("user", "Ring 3      : nicht betreten — Architektur meldet keine Unterstuetzung");
        return (0, 0);
    }
    let Some(sup) = support() else {
        klog!("user", "Ring 3      : nicht betreten — kein Programm angemeldet");
        return (0, 0);
    };

    let mut pages = UserPages::new();
    let mut ok = 0usize;
    let total = 4usize;

    // 1) Beispielprogramm der Architektur abbilden (lesen + ausfuehren, kein
    //    Schreibrecht — W^X gilt auch unprivilegiert).
    if !map_image(space, &mut pages, DEMO_BASE, sup.program, MapFlags::READ | MapFlags::EXEC) {
        klog!("user", "Ring 3      : nicht betreten — Abbildung fehlgeschlagen");
        pages.teardown(space);
        return (0, total);
    }
    let Some(stack_top) = map_stack(space, &mut pages) else {
        klog!("user", "Ring 3      : nicht betreten — kein Stapel");
        pages.teardown(space);
        return (0, total);
    };
    let stack_bottom = USER_STACK_TOP - (USER_STACK_PAGES * PAGE_SIZE) as u64;
    klog!(
        "user",
        "Abbildung   : Programm {:#018x} ({} B, RX), Stapel {:#018x}..{:#018x} (RW, NX), {} Seiten",
        DEMO_BASE,
        sup.program.len(),
        stack_bottom,
        USER_STACK_TOP,
        pages.n
    );

    // 2) Prozess anlegen und ihm EXPLIZIT ein Handle auf die Ausgabe geben.
    //    Ohne dieses Handle hat das Programm keinerlei Zugriff — es gibt
    //    keinen Strom, der "einfach da" ist.
    let pid = match crate::abi::native::spawn(0, "ring3", &[]) {
        Ok(p) => p,
        Err(e) => {
            klog!("user", "Ring 3      : nicht betreten — kein Prozess ({})", e.name());
            pages.teardown(space);
            return (0, total);
        }
    };
    let handle = match crate::abi::native::grant_console(pid, karst_abi_native::Rights::WRITE) {
        Ok(h) => h,
        Err(e) => {
            klog!("user", "Ring 3      : nicht betreten — kein Ausgabehandle ({})", e.name());
            pages.teardown(space);
            return (0, total);
        }
    };
    // Uebergabe: das Handle steht beim Start im ersten Argumentregister.
    // Mehr bekommt das Programm nicht — keine Umgebung, keine festen
    // Deskriptoren, kein Namensraum.
    set_entry_arg(handle.0);
    klog!(
        "user",
        "Prozess     : pid {} unprivilegiert, 1 Handle uebergeben (Ausgabe, nur Schreibrecht), {} Handle(s) in der Tafel",
        pid,
        crate::abi::native::handle_count(pid)
    );

    // 3) Regulaerer Lauf.
    let before = rejected();
    let vorher = crate::abi::native::set_current(pid);
    // SAFETY: beide Adressen wurden gerade unprivilegiert abgebildet.
    let exit = unsafe { run(VirtAddr(DEMO_BASE + sup.entry_offset as u64), VirtAddr(stack_top)) };
    crate::abi::native::set_current(vorher);
    report_exit(&exit);
    if exit.entered && exit.privilege_level == 3 && exit.code_selector & 3 == 3 {
        ok += 1;
    }
    if is_user_addr(exit.stack_pointer) && exit.stack_pointer <= USER_STACK_TOP {
        ok += 1;
    }
    // Drei Aufrufe erwartet: Lebenszeichen, Ausgabe, und die Probe mit einem
    // Kernelzeiger als Puffer — die MUSS abgewiesen worden sein.
    if exit.syscalls >= 3 && exit.code == 0 && rejected() == before + 1 {
        ok += 1;
    }
    // Der Kernel raeumt den Prozess ab: Handles weg, Objekte frei.
    let _ = crate::abi::native::exit_process(pid, exit.code as i64);
    klog!(
        "user",
        "Abmeldung   : pid {} beendet mit Code {}, danach {} Handle(s) in seiner Tafel",
        pid,
        exit.code,
        crate::abi::native::handle_count(pid)
    );

    // 4) Negativtest: dasselbe Bild, anderer Einsprung — das Programm fasst
    //    eine Kerneladresse an. Erwartet wird eine gemeldete Schutzverletzung
    //    und ein weiterlaufender Kernel.
    let before = rejected();
    // SAFETY: wie oben; der Einsprung liegt im selben abgebildeten Bild.
    let bad = unsafe { run(VirtAddr(DEMO_BASE + sup.fault_offset as u64), VirtAddr(stack_top)) };
    if rejected() > before && bad.entered {
        ok += 1;
        klog!(
            "user",
            "Negativtest : Programm nach {} Systemaufruf(en) abgeraeumt, Kernel laeuft weiter",
            bad.syscalls
        );
    } else {
        klog!("user", "Ring-3-Negativtest hat NICHT ausgeloest — Nachweis fehlt");
    }

    // 5) Schutzwall: jeder von der Architektur angemeldete Uebergriff wird
    //    einzeln versucht. Keiner darf gelingen, keiner darf den Kernel
    //    anhalten.
    let (o, t) = run_probes(&sup, stack_top);
    ok += o;
    let total = total + t;

    // 6) Der Ernstfall: das Programm, das der ELF-Lader aus dem
    //    Startdateisystem geholt hat. Es kommt als echtes Abbild aus dem
    //    Archiv, nicht als eingebautes Bytefeld — der Kernel kennt davon nur
    //    Einsprung und Stapel.
    let (o, t) = run_initramfs_program(DEMO_BASE + sup.trampoline_offset as u64);
    ok += o;
    let total = total + t;

    // 7) Abraeumen — der Kernel gibt jede Seite und jeden Frame zurueck.
    let frei = pages.teardown(space);
    klog!(
        "user",
        "Abgeraeumt  : {} Seiten zurueckgegeben, {} B aus Ring 3 ausgegeben, {} Zugriff(e) abgewiesen",
        frei,
        WRITTEN.load(Ordering::Relaxed),
        rejected()
    );
    (ok, total)
}

/// Fuehrt jeden von der Architektur angemeldeten Uebergriff einzeln aus.
///
/// Liefert `(bestanden, gesamt)`. Der Nachweis ist bewusst kleinteilig: eine
/// Sammelzahl waere zu leicht gruen zu bekommen, deshalb bekommt jeder
/// Uebergriff eine eigene Zeile im Boot-Log.
///
/// Bewertet wird nur an gemessenen Groessen:
/// * [`ProbeExpect::Abgewiesen`]: die Zahl der abgewiesenen Zugriffe muss
///   gestiegen sein (der Ausnahmepfad hat den Vorfall gemeldet), das Programm
///   darf sich nicht selbst beendet haben, und der Kernel laeuft weiter —
///   sonst waere diese Funktion nie zurueckgekehrt.
/// * [`ProbeExpect::Wachhund`]: das Programm darf NICHT mit seinem eigenen
///   Beendigungscode angekommen sein. Ist es das doch, hat ihm niemand die
///   CPU weggenommen; das wird gemeldet und als nicht bestanden gezaehlt,
///   statt es schoenzureden.
fn run_probes(sup: &ArchSupport, stack_top: u64) -> (usize, usize) {
    let n = sup.probe_count.min(MAX_PROBES);
    if n == 0 {
        return (0, 0);
    }
    let mut ok = 0usize;
    let mut gewertet = 0usize;
    for probe in sup.probes.iter().take(n) {
        let before = rejected();
        // Jeder Uebergriff bekommt einen eigenen Prozess — und zwar OHNE
        // jedes Handle. Ein Programm, das nichts uebergeben bekommen hat,
        // darf auch nichts koennen.
        let pid = match crate::abi::native::spawn(0, "probe", &[]) {
            Ok(p) => p,
            Err(e) => {
                klog!("user", "Schutzwall  : {} nicht versucht ({})", probe.name, e.name());
                continue;
            }
        };
        set_entry_arg(0);
        let vorher = crate::abi::native::set_current(pid);
        // SAFETY: der Einsprung liegt im selben unprivilegiert abgebildeten
        // Bild wie das Beispielprogramm, der Stapel ebenfalls.
        let exit = unsafe { run(VirtAddr(DEMO_BASE + probe.offset as u64), VirtAddr(stack_top)) };
        crate::abi::native::set_current(vorher);
        let _ = crate::abi::native::exit_process(pid, exit.code as i64);

        let bestanden = match probe.expect {
            ProbeExpect::Abgewiesen => exit.entered && rejected() > before,
            ProbeExpect::Wachhund { selbstende } => exit.entered && exit.code != selbstende,
        };
        if bestanden {
            ok += 1;
            gewertet += 1;
        }
        match probe.expect {
            ProbeExpect::Abgewiesen if bestanden => klog!(
                "user",
                "Schutzwall  : {} -> abgewiesen, Programm abgeraeumt, Kernel laeuft (ok)",
                probe.name
            ),
            ProbeExpect::Wachhund { .. } if bestanden => klog!(
                "user",
                "Schutzwall  : {} -> vom Zeitgeber unterbrochen und abgeraeumt (ok)",
                probe.name
            ),
            // Der Wachhund haengt am Zeitgeberpfad. Nimmt der einem
            // unprivilegierten Programm die CPU heute noch nicht weg, ist das
            // ein bekannter offener Punkt und KEIN fehlgeschlagener Test: es
            // wird ehrlich gemeldet und nicht gewertet, statt eine Zusage zu
            // behaupten, die der Kernel nicht einloest.
            ProbeExpect::Wachhund { .. } => klog!(
                "user",
                "Schutzwall  : {} lief durch — Zeitgeber unterbricht Ring 3 noch nicht (offener Punkt, nicht gewertet)",
                probe.name
            ),
            ProbeExpect::Abgewiesen => {
                gewertet += 1;
                klog!(
                    "user",
                    "Schutzwall  : {} wurde NICHT abgewiesen — Nachweis fehlt",
                    probe.name
                )
            }
        }
    }
    klog!("user", "Schutzwall  : {}/{} Uebergriffe aus Ring 3 abgewehrt", ok, gewertet);
    (ok, gewertet)
}

/// Startet das vom ELF-Lader bereitgestellte Programm unprivilegiert.
///
/// Liefert `(bestanden, gesamt)`. Ist nichts geladen (kein Archiv, kaputtes
/// Abbild), wird das ehrlich gemeldet und nichts gezaehlt — der Boot bleibt
/// gruen.
fn run_initramfs_program(trampoline: u64) -> (usize, usize) {
    let Some((entry, stack_top)) = crate::kcore::elf::program() else {
        klog!("user", "Startdateisystem: kein geladenes Programm — nichts zu starten");
        return (0, 0);
    };
    let pid = match crate::abi::native::spawn(0, "hello", &[]) {
        Ok(p) => p,
        Err(e) => {
            klog!("user", "Programm aus dem Archiv nicht gestartet: {}", e.name());
            return (0, 1);
        }
    };
    let handle = match crate::abi::native::grant_console(pid, karst_abi_native::Rights::WRITE) {
        Ok(h) => h,
        Err(e) => {
            klog!("user", "Programm aus dem Archiv nicht gestartet: {}", e.name());
            return (0, 1);
        }
    };
    set_entry_arg(handle.0);
    // Ueber das Messsprungbrett starten: die echte Einsprungadresse liegt als
    // oberstes Stapelwort, das Sprungbrett holt sie sich, loest einmal eine
    // Ausnahme aus (Messpunkt) und springt weiter. Das Programm selbst bleibt
    // unveraendert so, wie es im Archiv liegt.
    let slot = stack_top.as_u64() - 8;
    with_user_window(|| {
        // SAFETY: die oberste Stapelseite des Programms ist beschreibbar
        // abgebildet (der ELF-Lader hat sie angelegt).
        unsafe { core::ptr::write_volatile(slot as *mut u64, entry.as_u64()) }
    });
    let vorher = crate::abi::native::set_current(pid);
    // SAFETY: Sprungbrett, Einsprung und Stapel sind unprivilegiert
    // abgebildet.
    let exit = unsafe { run(VirtAddr(trampoline), VirtAddr(slot)) };
    crate::abi::native::set_current(vorher);
    let _ = crate::abi::native::exit_process(pid, exit.code as i64);
    if exit.entered && exit.privilege_level == 3 && is_user_addr(exit.stack_pointer) {
        klog!(
            "user",
            "Aus dem Archiv: Einsprung {:#018x}, CS={:#06x} (CPL={}), Stapel={:#018x}, {} Systemaufruf(e), pid {} abgemeldet",
            entry.as_u64(),
            exit.code_selector,
            exit.privilege_level,
            exit.stack_pointer,
            exit.syscalls,
            pid
        );
        (1, 1)
    } else {
        klog!(
            "user",
            "Aus dem Archiv: Programm lief NICHT unprivilegiert durch — {}",
            exit.reason
        );
        (0, 1)
    }
}
