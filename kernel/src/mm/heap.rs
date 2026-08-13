//! Kernel-Heap und `#[global_allocator]`.
//!
//! **Schicht: mm.** Die Allokationslogik selbst steht in [`karst_mem::heap`] und
//! ist auf dem Host getestet; hier kommt nur der Speicher dazu: Frames vom
//! Frame-Allocator, abgebildet in ein eigenes PML4-Fenster.

use crate::klog;
use core::alloc::{GlobalAlloc, Layout};

use karst_mem::heap::{Heap, HeapStats};
use spin::Mutex;

use crate::arch::AddressSpace;
use crate::kcore::arch_iface::AddressSpaceOps;
use crate::kcore::mem::{FrameAllocator, MapError, MapFlags, VirtAddr, PAGE_SIZE};

/// Virtuelle Basis des Kernel-Heaps (PML4-Eintrag 510 — eigener Bereich,
/// weder im HHDM-Fenster (256) noch im Kernelabbild (511)).
pub const HEAP_BASE: u64 = 0xffff_ff00_0000_0000;
/// Anfangsgroesse des Heaps.
pub const HEAP_INITIAL: usize = 1024 * 1024;

struct KernelAllocator(Mutex<Heap>);

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { self.0.lock().alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { self.0.lock().dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator(Mutex::new(Heap::empty()));

/// Bildet den Heap ab und meldet ihn beim Allocator an.
///
/// # Safety
/// Der uebergebene Adressraum muss der AKTIVE sein, sonst zeigt [`HEAP_BASE`]
/// ins Leere. Genau einmal aufrufen.
pub unsafe fn init(
    space: &mut AddressSpace,
    fa: &mut dyn FrameAllocator,
) -> Result<usize, MapError> {
    let pages = HEAP_INITIAL / PAGE_SIZE;
    for i in 0..pages {
        let frame = fa.alloc_frame().ok_or(MapError::OutOfFrames)?;
        let virt = VirtAddr(HEAP_BASE + (i * PAGE_SIZE) as u64);
        unsafe { space.map(virt, frame, MapFlags::KERNEL_DATA, &mut *fa)? };
    }
    unsafe { ALLOCATOR.0.lock().add_region(HEAP_BASE as *mut u8, HEAP_INITIAL) };
    Ok(HEAP_INITIAL)
}

/// Gesamtgroesse des Heaps in Bytes.
pub fn size() -> usize {
    ALLOCATOR.0.lock().size()
}
/// Belegte Bytes.
pub fn used() -> usize {
    ALLOCATOR.0.lock().used()
}
/// Freie Bytes.
pub fn free() -> usize {
    ALLOCATOR.0.lock().free()
}

/// Konsistente Momentaufnahme aller Heap-Kennzahlen.
pub fn stats() -> HeapStats {
    ALLOCATOR.0.lock().stats()
}

/// Behandlung einer fehlgeschlagenen Allokation.
///
/// Der Standardhandler meldet nur "memory allocation of N bytes failed". Hier
/// wird stattdessen berichtet, WARUM es nicht ging: Groesse und Ausrichtung der
/// Anforderung gegen den tatsaechlichen Heapzustand samt Fragmentierung
/// (groesstes freies Loch). Danach folgt der regulaere Panic-Pfad mit Backtrace.
#[alloc_error_handler]
fn on_oom(layout: Layout) -> ! {
    let s = stats();
    klog!(
        "heap",
        "AUSSER SPEICHER: {} B (Ausrichtung {} B) angefordert",
        layout.size(),
        layout.align()
    );
    klog!(
        "heap",
        "Heapzustand : {} B gesamt, {} B belegt, {} B frei, {} Loecher, groesstes {} B",
        s.size,
        s.used,
        s.free,
        s.holes,
        s.largest_hole
    );
    panic!(
        "Heap erschoepft: {} B angefordert, groesstes freies Loch {} B",
        layout.size(),
        s.largest_hole
    )
}

/// Selbsttest des Heaps im laufenden Kernel: Allozieren, Ausrichtung, Freigeben
/// und die Buchhaltung dazu. Rueckgabe: `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    let total = 3usize;
    let mut ok = 0usize;
    let before = stats();
    let l = Layout::from_size_align(4096, 4096).expect("gueltiges Layout");

    let p = unsafe { ALLOCATOR.alloc(l) };
    if !p.is_null() && (p as usize) % 4096 == 0 {
        ok += 1;
    }
    if stats().used > before.used {
        ok += 1;
    }
    if !p.is_null() {
        unsafe { ALLOCATOR.dealloc(p, l) };
    }
    let after = stats();
    if after.used == before.used && after.free == before.free {
        ok += 1;
    }
    klog!(
        "heap",
        "Selbsttest Heap: {}/{} Zusagen erfuellt, {} B belegt, {} Loecher",
        ok,
        total,
        after.used,
        after.holes
    );
    (ok, total)
}
