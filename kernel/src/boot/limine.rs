//! Limine-Bootprotokoll (Basisrevision 3).
//!
//! **Warum Limine und nicht Multiboot2** (ausfuehrlich in ARCHITECTURE.md):
//! Multiboot2 uebergibt die CPU im 32-Bit-Protected-Mode. Der Kernel muesste
//! dann selbst einen Assembler-Vorlader mitbringen, der Paging aufsetzt, nach
//! long mode wechselt und sich in die obere Haelfte umzieht — mehrere hundert
//! Zeilen Assembler, die nichts zum eigentlichen Ziel beitragen und bei jeder
//! Aenderung Triple Faults produzieren. Limine liefert den Kernel bereits im
//! 64-Bit-Modus, higher-half gemappt, mit einem HHDM-Fenster, einer sortierten
//! Memory-Map und einem gesetzten Framebuffer ab. Der Kernel behaelt trotzdem
//! die volle Kontrolle: er baut in `mm` sofort seinen EIGENEN Adressraum und
//! wirft die Bootloader-Tabellen weg.

use ::limine::memory_map::EntryType;
use ::limine::request::{
    BootloaderInfoRequest, ExecutableAddressRequest, FramebufferRequest, HhdmRequest,
    MemoryMapRequest, ModuleRequest, RequestsEndMarker, RequestsStartMarker, StackSizeRequest,
};
use ::limine::BaseRevision;
use karst_mem::region::{MemoryRegion, RegionKind};

/// Stapelgroesse, die wir vom Bootloader verlangen (64 KiB).
pub const STACK_SIZE: u64 = 64 * 1024;

/// Maximal uebernommene Eintraege der Memory-Map.
const MAX_REGIONS: usize = 256;

#[used]
#[unsafe(link_section = ".limine_requests_start")]
static REQ_START: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
static INFO: BootloaderInfoRequest = BootloaderInfoRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
static STACK: StackSizeRequest = StackSizeRequest::new().with_size(STACK_SIZE);

#[used]
#[unsafe(link_section = ".limine_requests")]
static HHDM: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
static MEMMAP: MemoryMapRequest = MemoryMapRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
static FRAMEBUFFER: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
static EXEC_ADDR: ExecutableAddressRequest = ExecutableAddressRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
static MODULES: ModuleRequest = ModuleRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests_end")]
static REQ_END: RequestsEndMarker = RequestsEndMarker::new();

static mut REGIONS: [MemoryRegion; MAX_REGIONS] =
    [MemoryRegion::new(0, 0, RegionKind::Reserved); MAX_REGIONS];
static mut REGION_COUNT: usize = 0;

/// Bootloadername und -version, falls gemeldet.
pub fn bootloader_name() -> (&'static str, &'static str) {
    match INFO.get_response() {
        Some(r) => (r.name(), r.version()),
        None => ("unbekannt", "-"),
    }
}

/// Wurde die verlangte Basisrevision vom Bootloader akzeptiert?
pub fn base_revision_ok() -> bool {
    BASE_REVISION.is_supported()
}

/// Offset des Higher-Half-Direct-Map-Fensters.
pub fn hhdm_offset() -> Option<u64> {
    HHDM.get_response().map(|r| r.offset())
}

/// Physische und virtuelle Basisadresse des geladenen Kernelabbilds.
pub fn kernel_base() -> Option<(u64, u64)> {
    EXEC_ADDR.get_response().map(|r| (r.physical_base(), r.virtual_base()))
}

/// Wurde der groessere Stapel gewaehrt?
pub fn stack_granted() -> bool {
    STACK.get_response().is_some()
}

/// Neutrale Beschreibung des Bootloader-Framebuffers.
#[derive(Clone, Copy)]
pub struct FramebufferInfo {
    /// Virtuelle Startadresse (bereits im HHDM-Fenster).
    pub addr: *mut u8,
    /// Breite in Pixeln.
    pub width: usize,
    /// Hoehe in Pixeln.
    pub height: usize,
    /// Bytes pro Zeile.
    pub pitch: usize,
    /// Bits pro Pixel.
    pub bpp: u16,
    /// Bitposition des Rotkanals.
    pub red_shift: u8,
    /// Bitposition des Gruenkanals.
    pub green_shift: u8,
    /// Bitposition des Blaukanals.
    pub blue_shift: u8,
}

/// Erster gemeldeter Framebuffer, falls vorhanden.
pub fn framebuffer() -> Option<FramebufferInfo> {
    let fb = FRAMEBUFFER.get_response()?.framebuffers().next()?;
    Some(FramebufferInfo {
        addr: fb.addr(),
        width: fb.width() as usize,
        height: fb.height() as usize,
        pitch: fb.pitch() as usize,
        bpp: fb.bpp(),
        red_shift: fb.red_mask_shift(),
        green_shift: fb.green_mask_shift(),
        blue_shift: fb.blue_mask_shift(),
    })
}

fn kind_of(t: EntryType) -> RegionKind {
    match t {
        EntryType::USABLE => RegionKind::Usable,
        EntryType::BOOTLOADER_RECLAIMABLE => RegionKind::BootloaderReclaimable,
        EntryType::EXECUTABLE_AND_MODULES => RegionKind::KernelAndModules,
        EntryType::ACPI_RECLAIMABLE => RegionKind::AcpiReclaimable,
        EntryType::ACPI_NVS => RegionKind::AcpiNvs,
        EntryType::FRAMEBUFFER => RegionKind::Framebuffer,
        EntryType::BAD_MEMORY => RegionKind::BadMemory,
        _ => RegionKind::Reserved,
    }
}

/// Kopiert die Memory-Map des Bootloaders in kernel-eigene, neutrale Typen.
///
/// Danach ist der Kernel unabhaengig von den Bootloader-Strukturen — wichtig,
/// weil deren Speicher spaeter zurueckgewonnen wird.
///
/// # Safety
/// Genau einmal im fruehen Boot aufrufen, bevor irgendetwas alloziert.
pub unsafe fn take_memory_map() -> &'static [MemoryRegion] {
    let Some(resp) = MEMMAP.get_response() else {
        return &[];
    };
    let entries = resp.entries();
    let n = entries.len().min(MAX_REGIONS);
    unsafe {
        let regions = core::ptr::addr_of_mut!(REGIONS);
        for i in 0..n {
            let e = entries[i];
            (*regions)[i] = MemoryRegion::new(e.base, e.length, kind_of(e.entry_type));
        }
        REGION_COUNT = n;
        core::slice::from_raw_parts(regions as *const MemoryRegion, n)
    }
}

/// Anzahl der Module, die der Bootloader mitgeladen hat.
pub fn module_count() -> usize {
    MODULES.get_response().map(|r| r.modules().len()).unwrap_or(0)
}

/// Sucht ein mitgeladenes Modul anhand der Zeichenkette aus der
/// Bootkonfiguration (`module_string`) und liefert Adresse und Laenge.
///
/// Bewusst ueber die Zeichenkette und nicht ueber den Pfad: der Kern kennt
/// keinen Pfadnamensraum, und der Ablageort im Abbild soll die Kernellogik
/// nicht bestimmen. Die Adresse liegt bereits im Direct-Map-Fenster und
/// bleibt gueltig, weil der Bereich in der Memory-Map als "kernel+modules"
/// gefuehrt wird und der Frame-Allocator ihn deshalb nie vergibt.
pub fn module_by_string(want: &str) -> Option<(u64, usize)> {
    let resp = MODULES.get_response()?;
    for m in resp.modules() {
        if m.string().to_bytes() == want.as_bytes() {
            let addr = m.addr() as u64;
            let len = m.size() as usize;
            if addr != 0 && len != 0 {
                return Some((addr, len));
            }
        }
    }
    None
}

/// Die bereits uebernommene Memory-Map.
pub fn memory_map() -> &'static [MemoryRegion] {
    unsafe {
        let regions = core::ptr::addr_of!(REGIONS);
        core::slice::from_raw_parts(regions as *const MemoryRegion, REGION_COUNT)
    }
}
