//! Kernel-Heap und `#[global_allocator]`.
//!
//! **Schicht: mm.** Die Allokationslogik selbst steht in [`karst_mem::heap`] und
//! ist auf dem Host getestet; hier kommt nur der Speicher dazu: Frames vom
//! Frame-Allocator, abgebildet in ein eigenes Fenster der obersten
//! Tabellenebene.

use crate::klog;
use core::alloc::{GlobalAlloc, Layout};

use karst_mem::heap::{Heap, HeapStats};
use spin::Mutex;

use crate::arch::AddressSpace;
use crate::kcore::arch_iface::AddressSpaceOps;
use crate::kcore::mem::{FrameAllocator, MapError, MapFlags, VirtAddr, PAGE_SIZE};

/// Virtuelle Basis des Kernel-Heaps (Eintrag 510 der obersten Tabellenebene —
/// eigener Bereich, weder im HHDM-Fenster (256) noch im Kernelabbild (511)).
pub const HEAP_BASE: u64 = 0xffff_ff00_0000_0000;
/// Anfangsgroesse des Heaps.
pub const HEAP_INITIAL: usize = 1024 * 1024;

// Beide Zusagen gelten schon beim Uebersetzen: [`init`] rechnet in ganzen
// Seiten ab [`HEAP_BASE`]. Eine krumme Konstante wuerde erst zur Laufzeit als
// halb abgebildeter Heap auffallen — und dann nicht als Fehlermeldung, sondern
// als Page Fault mitten in einer Allokation.
const _: () = assert!(
    HEAP_BASE % PAGE_SIZE as u64 == 0,
    "HEAP_BASE muss seitenausgerichtet sein"
);
const _: () = assert!(
    HEAP_INITIAL % PAGE_SIZE == 0 && HEAP_INITIAL > 0,
    "HEAP_INITIAL muss ein positives Vielfaches der Seitengroesse sein"
);

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

/// Liegt `p` wirklich im abgebildeten Heapfenster?
///
/// Damit laesst sich pruefen, dass der Allocator keinen Zeiger ausserhalb des
/// ihm uebergebenen Bereichs liefert — ein solcher Zeiger waere entweder ein
/// Rechenfehler in der Freiliste oder eine ueberschriebene Blockkopfzeile.
pub fn contains(p: *const u8) -> bool {
    let a = p as u64;
    let base = HEAP_BASE;
    let end = base + size() as u64;
    a >= base && a < end
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
///    (Nachbarloecher werden verschmolzen),
/// 8. eine Anforderung ueber 0 Bytes liefert trotzdem einen gueltigen,
///    ausgerichteten Zeiger (`Layout` mit Groesse 0 ist erlaubt) und laesst
///    sich freigeben,
/// 9. ein freigegebenes Loch wird wiederverwendet: dieselbe Anforderung
///    bekommt danach dieselbe Adresse, der Heap waechst also nicht bei jeder
///    Runde aus Fragmentierung heraus,
/// 10. jeder gelieferte Zeiger liegt innerhalb des abgebildeten Heapfensters
///     und ist mindestens auf [`karst_mem::heap::ALIGN`] ausgerichtet.
///
/// Rueckgabe: `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    let total = 10usize;
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
    let mut end = stats();
    if end == after {
        ok += 1;
    }

    // 8 — Groesse 0. `Layout::from_size_align(0, 1)` ist gueltiges Rust; der
    //     Allocator muss einen benutzbaren, ausgerichteten Zeiger liefern
    //     (Rust verlangt fuer Groesse 0 keinen bestimmten Wert, aber niemals
    //     einen Zeiger ausserhalb des Heaps).
    let zero = Layout::from_size_align(0, 1).expect("gueltiges Layout");
    let z = unsafe { ALLOCATOR.alloc(zero) };
    let zero_ok = !z.is_null() && contains(z) && (z as usize) % karst_mem::heap::ALIGN == 0;
    if !z.is_null() {
        unsafe { ALLOCATOR.dealloc(z, zero) };
    }
    if zero_ok && stats() == end {
        ok += 1;
    }

    // 9 — Wiederverwendung: erst belegen, freigeben, erneut dieselbe Groesse
    //     anfordern. Kommt eine andere Adresse zurueck, verschmilzt die
    //     Freiliste nicht richtig und der Heap fragmentiert mit jeder Runde.
    let reuse_l = Layout::from_size_align(512, 32).expect("gueltiges Layout");
    let r1 = unsafe { ALLOCATOR.alloc(reuse_l) };
    if !r1.is_null() {
        unsafe { ALLOCATOR.dealloc(r1, reuse_l) };
    }
    let r2 = unsafe { ALLOCATOR.alloc(reuse_l) };
    let reused = !r1.is_null() && r1 == r2;
    if !r2.is_null() {
        unsafe { ALLOCATOR.dealloc(r2, reuse_l) };
    }
    if reused && stats() == end {
        ok += 1;
    }

    // 10 — Serie unterschiedlicher Groessen: jeder Zeiger muss im Heapfenster
    //      liegen und die Grundausrichtung des Allocators erfuellen.
    let mut inside = true;
    let mut series = [core::ptr::null_mut::<u8>(); N];
    let mut series_l = [l; N];
    for i in 0..N {
        let lay = Layout::from_size_align(1 + i * 777, 8).expect("gueltiges Layout");
        series_l[i] = lay;
        let p = unsafe { ALLOCATOR.alloc(lay) };
        series[i] = p;
        if p.is_null() || !contains(p) || (p as usize) % karst_mem::heap::ALIGN != 0 {
            inside = false;
        }
    }
    for i in 0..N {
        if !series[i].is_null() {
            unsafe { ALLOCATOR.dealloc(series[i], series_l[i]) };
        }
    }
    end = stats();
    if inside && end == after {
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
    klog!(
        "heap",
        "  Heap-Randfaelle: Groesse 0 {}, Loch wiederverwendet {}, alle {} Zeiger im Fenster {:#018x}..{:#018x} {}",
        if zero_ok { "ok" } else { "nicht ok" },
        if reused { "ok" } else { "nicht ok" },
        N,
        HEAP_BASE,
        HEAP_BASE + before.size as u64,
        if inside { "ok" } else { "nicht ok" }
    );
    (ok, total)
}
