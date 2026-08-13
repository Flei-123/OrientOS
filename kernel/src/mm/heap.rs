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
    // Der Heap darf nie ueber etwas Bestehendes gelegt werden — das waere
    // stiller Datenverlust statt eines Fehlers.
    if space.translate(VirtAddr(HEAP_BASE)).is_some() {
        return Err(MapError::AlreadyMapped);
    }
    let pages = HEAP_INITIAL / PAGE_SIZE;
    for i in 0..pages {
        let virt = VirtAddr(HEAP_BASE + (i * PAGE_SIZE) as u64);
        let res = match fa.alloc_frame() {
            None => Err(MapError::OutOfFrames),
            Some(frame) => {
                match unsafe { space.map(virt, frame, MapFlags::KERNEL_DATA, &mut *fa) } {
                    Ok(()) => Ok(()),
                    // Der Frame haengt nirgends: sofort zurueckgeben, sonst
                    // versickert er.
                    Err(e) => {
                        fa.free_frame(frame);
                        Err(e)
                    }
                }
            }
        };
        if let Err(e) = res {
            // Sauber zuruecknehmen: jede bereits eingehaengte Seite wieder
            // loesen und ihren Frame freigeben. Ein halb abgebildeter Heap
            // waere schlimmer als gar keiner — er fiele erst viel spaeter als
            // Page Fault auf.
            for j in 0..i {
                let v = VirtAddr(HEAP_BASE + (j * PAGE_SIZE) as u64);
                if let Ok(f) = unsafe { space.unmap(v) } {
                    fa.free_frame(f);
                }
            }
            klog!(
                "heap",
                "Heap-Einrichtung abgebrochen nach {} von {} Seiten: {}",
                i,
                pages,
                e.describe()
            );
            return Err(e);
        }
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

/// Selbsttest des Heaps im laufenden Kernel.
///
/// Geprueft werden Erfolgs- **und** Fehlerpfad:
/// 1. Allokation mit 4-KiB-Ausrichtung liefert einen passend ausgerichteten Zeiger,
/// 2. die Buchhaltung waechst dabei,
/// 3. nach der Freigabe stimmen belegt/frei wieder exakt,
/// 4. eine Anforderung groesser als der ganze Heap gibt `null` zurueck, statt
///    zu panicken oder Speicher zu zerstoeren,
/// 5. der Heapzustand ist danach unveraendert (kein verlorener Block),
/// 6. verschraenkte Allokationen sind ueberschneidungsfrei und beschreibbar,
/// 7. nach ihrer Freigabe ist der Heap wieder so fragmentiert wie zuvor
///    (Nachbarloecher werden verschmolzen).
///
/// Rueckgabe: `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    let total = 7usize;
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

    // 4/5 — Ueberforderung. `GlobalAlloc::alloc` meldet Misserfolg mit `null`;
    // erst `alloc`s Fehlerbehandlung macht daraus den Bericht in `on_oom`.
    let huge = Layout::from_size_align(before.size * 4, 16).expect("gueltiges Layout");
    let q = unsafe { ALLOCATOR.alloc(huge) };
    if q.is_null() {
        ok += 1;
    } else {
        unsafe { ALLOCATOR.dealloc(q, huge) };
    }
    let after_oom = stats();
    if after_oom == after {
        ok += 1;
    }

    // 6/7 — verschraenkte Allokationen unterschiedlicher Groesse, in anderer
    //       Reihenfolge freigegeben als belegt.
    const N: usize = 8;
    let mut ptrs = [core::ptr::null_mut::<u8>(); N];
    let mut layouts = [l; N];
    let mut disjoint = true;
    for i in 0..N {
        let size = 24 + i * 130;
        let align = 1usize << (3 + (i % 4));
        let lay = Layout::from_size_align(size, align).expect("gueltiges Layout");
        layouts[i] = lay;
        let ptr = unsafe { ALLOCATOR.alloc(lay) };
        ptrs[i] = ptr;
        if ptr.is_null() || (ptr as usize) % align != 0 {
            disjoint = false;
            continue;
        }
        // Jeden Block vollstaendig mit einem eigenen Muster fuellen.
        unsafe { core::ptr::write_bytes(ptr, (i as u8).wrapping_add(1), size) };
    }
    // Muster nachlesen: ueberlappen zwei Bloecke, ist mindestens eines zerstoert.
    for i in 0..N {
        let size = 24 + i * 130;
        if ptrs[i].is_null() {
            disjoint = false;
            continue;
        }
        for b in 0..size {
            if unsafe { core::ptr::read_volatile(ptrs[i].add(b)) } != (i as u8).wrapping_add(1) {
                disjoint = false;
                break;
            }
        }
    }
    if disjoint {
        ok += 1;
    }
    // Freigabe in gemischter Reihenfolge: erst die geraden, dann die ungeraden.
    for i in (0..N).step_by(2) {
        if !ptrs[i].is_null() {
            unsafe { ALLOCATOR.dealloc(ptrs[i], layouts[i]) };
        }
    }
    for i in (1..N).step_by(2) {
        if !ptrs[i].is_null() {
            unsafe { ALLOCATOR.dealloc(ptrs[i], layouts[i]) };
        }
    }
    let end = stats();
    if end == after {
        ok += 1;
    }

    klog!(
        "heap",
        "Selbsttest Heap: {}/{} Zusagen erfuellt, {} B belegt, {} Loecher",
        ok,
        total,
        end.used,
        end.holes
    );
    klog!(
        "heap",
        "  Heap-Fehlerpfade: {} B Anforderung abgewiesen (groesstes Loch {} B), Zustand unveraendert: {}",
        huge.size(),
        end.largest_hole,
        if end == after { "ja" } else { "nein" }
    );
    (ok, total)
}
