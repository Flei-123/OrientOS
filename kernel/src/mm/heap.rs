//! Kernel-Heap und `#[global_allocator]`.
//!
//! **Schicht: mm.** Die Allokationslogik selbst steht in [`karst_mem::heap`] und
//! ist auf dem Host getestet; hier kommt nur der Speicher dazu: Frames vom
//! Frame-Allocator, abgebildet in ein eigenes Fenster der obersten
//! Tabellenebene.
//!
//! Der Heap startet mit [`HEAP_INITIAL`] und **waechst bei Bedarf nach**
//! ([`grow`]): laeuft er voll, haengt er weitere Seiten hinten an, bis
//! [`HEAP_MAX`] erreicht ist. Erst wenn auch das nicht mehr geht, liefert
//! `alloc` einen Nullzeiger, und `alloc`s Fehlerbehandlung macht daraus den
//! Bericht in [`on_oom`] — mit Heapzustand, Wachstumsbilanz und Backtrace
//! statt der nichtssagenden Standardmeldung.

use crate::klog;
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

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
/// Obergrenze, bis zu der der Heap **selbsttaetig** nachwaechst (siehe
/// [`grow`]). Der Rest des physischen Speichers bleibt fuer Frames, Stapel und
/// spaetere Adressraeume reserviert; ein Kernel, der seinen Heap unbegrenzt
/// wachsen laesst, verhungert an anderer Stelle, ohne dass jemand die Ursache
/// sieht.
pub const HEAP_MAX: usize = 16 * 1024 * 1024;
/// Kleinster Zuwachs einer Erweiterung. Groesser als eine Seite, damit eine
/// Serie kleiner Anforderungen nicht Seite fuer Seite nachlaedt.
const GROW_CHUNK: usize = 64 * 1024;

const _: () = assert!(
    HEAP_MAX >= HEAP_INITIAL && HEAP_MAX % PAGE_SIZE == 0,
    "HEAP_MAX muss mindestens HEAP_INITIAL sein und seitenausgerichtet"
);
const _: () = assert!(
    GROW_CHUNK % PAGE_SIZE == 0 && GROW_CHUNK > 0,
    "GROW_CHUNK muss ein positives Vielfaches der Seitengroesse sein"
);

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
        let mut h = self.0.lock();
        let p = unsafe { h.alloc(layout) };
        if !p.is_null() {
            return p;
        }
        // Nichts frei: bevor die Anforderung scheitert (und `alloc` daraus eine
        // Panik macht), bekommt der Heap eine Chance nachzuwachsen. Gelingt das
        // nicht, bleibt es beim ehrlichen Nullzeiger.
        if grow_locked(&mut h, layout) == 0 {
            return core::ptr::null_mut();
        }
        unsafe { h.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { self.0.lock().dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator(Mutex::new(Heap::empty()));

// ------------------------------------------------------------ Nachwachsen
//
// Der Heap ist kein festes Fenster mehr: laeuft er voll, haengt er weitere
// Seiten hinten an (bis [`HEAP_MAX`]). Dafuer braucht er zwei Dinge, die es
// erst zur Laufzeit gibt — den aktiven Adressraum und den Frame-Allocator.

/// Der aktive Adressraum, in dem [`HEAP_BASE`] liegt. Von [`init`] gesetzt.
///
/// Ein roher Zeiger, weil `kmain` den Adressraum besitzt und ihn nie wieder
/// hergibt: er lebt bis zum Ende des Kernels. Benutzt wird er ausschliesslich
/// unter der Heap-Sperre und nur zum Anhaengen weiterer Seiten.
static SPACE: AtomicPtr<AddressSpace> = AtomicPtr::new(core::ptr::null_mut());
/// Tatsaechlich abgebildete Bytes ab [`HEAP_BASE`] — die massgebliche Grenze
/// des Heapfensters (die Buchhaltung des Allocators kann davon abweichen,
/// wenn er einen Rest verwirft).
static MAPPED: AtomicUsize = AtomicUsize::new(0);
/// Wie oft der Heap nachgewachsen ist.
static GROWS: AtomicUsize = AtomicUsize::new(0);
/// Wie oft eine Erweiterung nicht moeglich war (Obergrenze, keine Frames,
/// Frame-Allocator gesperrt oder kein Adressraum hinterlegt).
static GROW_FAILS: AtomicUsize = AtomicUsize::new(0);

/// Warum eine Erweiterung nicht ging — fuer den OOM-Bericht im Klartext.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GrowError {
    /// [`init`] hat noch keinen Adressraum hinterlegt.
    NoAddressSpace,
    /// Die Anforderung passt nicht mehr unter [`HEAP_MAX`].
    AtLimit,
    /// Der Frame-Allocator war belegt — jetzt gerade geht es nicht.
    AllocatorBusy,
    /// Es liess sich keine einzige weitere Seite abbilden.
    Mapping(MapError),
}

impl GrowError {
    /// Klartextbegruendung fuer das Log.
    pub const fn describe(self) -> &'static str {
        match self {
            GrowError::NoAddressSpace => "kein Adressraum hinterlegt",
            GrowError::AtLimit => "Obergrenze des Heaps erreicht",
            GrowError::AllocatorBusy => "Frame-Allocator gerade gesperrt",
            GrowError::Mapping(e) => e.describe(),
        }
    }
}

/// Bytes, die der Heap noch wachsen darf.
pub fn headroom() -> usize {
    HEAP_MAX.saturating_sub(MAPPED.load(Ordering::Relaxed))
}

/// Abgebildete Bytes des Heapfensters.
pub fn mapped() -> usize {
    MAPPED.load(Ordering::Relaxed)
}

/// Wie oft der Heap nachgewachsen ist.
pub fn grow_count() -> usize {
    GROWS.load(Ordering::Relaxed)
}

/// Wie oft eine Erweiterung abgelehnt werden musste.
pub fn grow_failures() -> usize {
    GROW_FAILS.load(Ordering::Relaxed)
}

/// Haengt `bytes` (aufgerundet auf ganze Seiten) hinten an das Heapfenster an.
///
/// Der Bereich waechst **zusammenhaengend** ab `HEAP_BASE + `[`mapped`], damit
/// die Freiliste den neuen Platz mit dem letzten Loch verschmelzen kann statt
/// zu fragmentieren. Schlaegt eine Seite mittendrin fehl, wird nur der bereits
/// abgebildete Teil verwendet; nichts bleibt halb eingehaengt.
///
/// Liefert die Zahl der wirklich hinzugefuegten Bytes.
pub fn grow(bytes: usize) -> Result<usize, GrowError> {
    let mut h = ALLOCATOR.0.lock();
    grow_inner(&mut h, bytes)
}

/// Wie [`grow`], aber mit bereits gehaltener Heap-Sperre.
fn grow_inner(h: &mut Heap, bytes: usize) -> Result<usize, GrowError> {
    let space = SPACE.load(Ordering::Relaxed);
    if space.is_null() {
        GROW_FAILS.fetch_add(1, Ordering::Relaxed);
        return Err(GrowError::NoAddressSpace);
    }
    let mapped = MAPPED.load(Ordering::Relaxed);
    let want = bytes.div_ceil(PAGE_SIZE) * PAGE_SIZE;
    if want == 0 || want > HEAP_MAX - mapped {
        // Bewusst gar nicht erst anfangen: eine Erweiterung, die die
        // Anforderung ohnehin nicht bedienen kann, wuerde nur Frames binden.
        GROW_FAILS.fetch_add(1, Ordering::Relaxed);
        return Err(GrowError::AtLimit);
    }
    let Some(mut guard) = crate::mm::frame::try_lock() else {
        GROW_FAILS.fetch_add(1, Ordering::Relaxed);
        return Err(GrowError::AllocatorBusy);
    };
    let Some(fa) = guard.as_mut() else {
        GROW_FAILS.fetch_add(1, Ordering::Relaxed);
        return Err(GrowError::NoAddressSpace);
    };
    // Sicher: `SPACE` zeigt auf den Adressraum von `kmain`, der bis zum Ende
    // des Kernels lebt und ausserhalb dieser Funktion nicht veraendert wird.
    let space = unsafe { &mut *space };

    let pages = want / PAGE_SIZE;
    let mut done = 0usize;
    let mut fehler = None;
    for i in 0..pages {
        let virt = VirtAddr(HEAP_BASE + (mapped + i * PAGE_SIZE) as u64);
        let Some(frame) = fa.alloc_frame() else {
            fehler = Some(MapError::OutOfFrames);
            break;
        };
        match unsafe { space.map(virt, frame, MapFlags::KERNEL_DATA, &mut *fa) } {
            Ok(()) => done += 1,
            Err(e) => {
                // Der Frame haengt nirgends — sofort zurueck, sonst versickert er.
                fa.free_frame(frame);
                fehler = Some(e);
                break;
            }
        }
    }
    if done == 0 {
        GROW_FAILS.fetch_add(1, Ordering::Relaxed);
        return Err(GrowError::Mapping(fehler.unwrap_or(MapError::OutOfFrames)));
    }
    let added = done * PAGE_SIZE;
    MAPPED.store(mapped + added, Ordering::Relaxed);
    GROWS.fetch_add(1, Ordering::Relaxed);
    // Sicher: der Bereich ist frisch abgebildet, gehoert ausschliesslich dem
    // Heap und bleibt bis zum Ende des Kernels bestehen.
    unsafe { h.add_region((HEAP_BASE + mapped as u64) as *mut u8, added) };
    Ok(added)
}

/// Erweiterung fuer genau eine gescheiterte Anforderung. Liefert die
/// hinzugefuegten Bytes (0 = ging nicht).
fn grow_locked(h: &mut Heap, layout: Layout) -> usize {
    // Platz fuer Nutzdaten, Verwaltungskopf und die schlimmstmoegliche
    // Ausrichtungsluecke, plus [`GROW_CHUNK`] Vorrat fuer die naechsten
    // Anforderungen.
    let need = layout
        .size()
        .saturating_add(layout.align())
        .saturating_add(karst_mem::heap::HEADER)
        .saturating_add(karst_mem::heap::ALIGN);
    grow_inner(h, need.max(GROW_CHUNK)).unwrap_or(0)
}

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
    MAPPED.store(HEAP_INITIAL, Ordering::Relaxed);
    // Ab hier kann der Heap selbsttaetig nachwachsen: er kennt den aktiven
    // Adressraum, in dem [`HEAP_BASE`] liegt, und holt sich Frames beim
    // globalen Frame-Allocator.
    SPACE.store(space as *mut AddressSpace, Ordering::Relaxed);
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
    // Massgeblich ist das ABGEBILDETE Fenster, nicht die Buchhaltung des
    // Allocators: nach einer Erweiterung liegen gueltige Zeiger auch jenseits
    // der urspruenglichen Groesse.
    let end = base + mapped() as u64;
    a >= base && a < end
}

/// Behandlung einer fehlgeschlagenen Allokation.
///
/// Der Standardhandler meldet nur "memory allocation of N bytes failed". Hier
/// wird stattdessen berichtet, WARUM es nicht ging: Groesse und Ausrichtung der
/// Anforderung gegen den tatsaechlichen Heapzustand samt Fragmentierung
/// (groesstes freies Loch). Danach folgt der regulaere Panic-Pfad mit Backtrace.
///
/// Hier landet nur, wer selbst nach einer Erweiterung ([`grow`]) keinen Platz
/// bekommen hat — deshalb steht im Bericht auch, wie oft der Heap bereits
/// gewachsen ist und wie viel er noch duerfte.
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
    klog!(
        "heap",
        "Wachstum    : {} B abgebildet von hoechstens {} B, {} Erweiterung(en), {} abgelehnt, {} B Spielraum, {} Frames frei",
        mapped(),
        HEAP_MAX,
        grow_count(),
        grow_failures(),
        headroom(),
        // Nicht blockierend: der Fehlerpfad darf unter keinen Umstaenden auf
        // eine Sperre warten, die vielleicht genau der Aufrufer haelt.
        match crate::mm::frame::try_lock() {
            Some(g) => g.as_ref().map_or(0, |a| a.free()),
            None => 0,
        }
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
///     und ist mindestens auf [`karst_mem::heap::ALIGN`] ausgerichtet,
/// 11. **Nachwachsen**: eine Anforderung, die groesser ist als alles derzeit
///     Freie, sprengt den Heap nicht, sondern laesst ihn wachsen — der Block
///     ist danach vollstaendig beschreibbar, liegt im (nun groesseren) Fenster,
///     und die Zahl der abgebildeten Bytes ist um genau so viel gestiegen, wie
///     der Allocator dazubekommen hat,
/// 12. die Erweiterung endet bei [`HEAP_MAX`]: eine Anforderung jenseits der
///     Obergrenze wird abgelehnt (`null`, gezaehlt), ohne dass auch nur eine
///     Seite dafuer abgebildet wird,
/// 13. die Fehlerfaelle des Nachwachsens sind wirklich erreichbar:
///     [`GrowError::AtLimit`] bei Groesse 0 und jenseits der Obergrenze,
///     [`GrowError::AllocatorBusy`], solange der Frame-Allocator gehalten wird
///     (statt dort blockierend zu warten und den Kernel anzuhalten).
///
/// Rueckgabe: `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    let total = 13usize;
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
    // Die Anforderung liegt bewusst jenseits von [`HEAP_MAX`], sonst wuerde der
    // Heap sie durch Nachwachsen bedienen (Fall 11).
    let huge = Layout::from_size_align(HEAP_MAX + PAGE_SIZE, 16).expect("gueltiges Layout");
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

    // 11 — Nachwachsen. Angefordert wird mehr, als der Heap gerade frei hat;
    //      ein fester Heap muesste hier `null` liefern. Geprueft wird nicht nur
    //      "hat geklappt", sondern auch, dass wirklich neue Seiten dazukamen
    //      und der Block ueber seine ganze Laenge beschreibbar ist (ein
    //      Rechenfehler beim Anhaengen faellt sonst erst spaeter als #PF auf).
    // Ergebnis der Faelle 1..10 festhalten, bevor das Wachstum die Kennzahlen
    // veraendert.
    let unveraendert = end == after;
    let mapped_before = mapped();
    let grows_before = grow_count();
    let grow_size = end.free + GROW_CHUNK;
    let grow_l = Layout::from_size_align(grow_size, 64).expect("gueltiges Layout");
    let gp = unsafe { ALLOCATOR.alloc(grow_l) };
    let mut grew = !gp.is_null()
        && contains(gp)
        && (gp as usize) % 64 == 0
        && mapped() > mapped_before
        && grow_count() > grows_before
        && stats().size >= end.size + (mapped() - mapped_before);
    if !gp.is_null() {
        unsafe { core::ptr::write_bytes(gp, 0x5c, grow_size) };
        for off in [0usize, grow_size / 2, grow_size - 1] {
            if unsafe { core::ptr::read_volatile(gp.add(off)) } != 0x5c {
                grew = false;
            }
        }
        // Der letzte Zeiger des Blocks muss ebenfalls noch im Fenster liegen.
        if !contains(unsafe { gp.add(grow_size - 1) }) {
            grew = false;
        }
        unsafe { ALLOCATOR.dealloc(gp, grow_l) };
    }
    let grown = stats();
    if grew && grown.used == end.used {
        ok += 1;
    }

    // 12 — Obergrenze. Jenseits von [`HEAP_MAX`] wird gar nicht erst
    //      nachgeladen: keine Seite mehr abgebildet, Anforderung abgelehnt.
    let mapped_at_limit = mapped();
    let fails_before = grow_failures();
    let over = Layout::from_size_align(HEAP_MAX + 1, 16).expect("gueltiges Layout");
    let op = unsafe { ALLOCATOR.alloc(over) };
    if !op.is_null() {
        unsafe { ALLOCATOR.dealloc(op, over) };
    }
    let limit_ok = op.is_null()
        && mapped() == mapped_at_limit
        && grow_failures() > fails_before
        && stats() == grown
        && headroom() == HEAP_MAX - mapped_at_limit;
    if limit_ok {
        ok += 1;
    }

    // 13 — die drei Absagegruende von [`grow`] wirklich ausloesen. Der Fall
    //      "Frame-Allocator gerade gesperrt" ist der wichtigste: dort wird
    //      bewusst NICHT gewartet, weil der Wartende die Heap-Sperre haelt.
    let leer = matches!(grow(0), Err(GrowError::AtLimit));
    let zu_gross = matches!(grow(HEAP_MAX + PAGE_SIZE), Err(GrowError::AtLimit));
    let besetzt = {
        let _halten = crate::mm::frame::lock();
        matches!(grow(GROW_CHUNK), Err(GrowError::AllocatorBusy))
    };
    let mapped_nach_fehlern = mapped();
    if leer && zu_gross && besetzt && mapped_nach_fehlern == mapped_at_limit {
        ok += 1;
    }

    klog!(
        "heap",
        "  Heap-Wachstum: {} B abgebildet (vorher {} B), {} Erweiterung(en), {} abgelehnt, Grenze {} B, Spielraum {} B",
        mapped(),
        mapped_before,
        grow_count(),
        grow_failures(),
        HEAP_MAX,
        headroom()
    );
    klog!(
        "heap",
        "  Heap-Wachstumsfehler nachweisbar: Groesse 0 {}, ueber der Grenze {}, Frame-Allocator besetzt {} ({})",
        if leer { "ok" } else { "FEHLER" },
        if zu_gross { "ok" } else { "FEHLER" },
        if besetzt { "ok" } else { "FEHLER" },
        GrowError::AllocatorBusy.describe()
    );
    end = grown;

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
        if unveraendert { "ja" } else { "nein" }
    );
    klog!(
        "heap",
        "  Heap-Randfaelle: Groesse 0 {}, Loch wiederverwendet {}, alle {} Zeiger im Fenster {:#018x}..{:#018x} {}",
        if zero_ok { "ok" } else { "nicht ok" },
        if reused { "ok" } else { "nicht ok" },
        N,
        HEAP_BASE,
        HEAP_BASE + mapped() as u64,
        if inside { "ok" } else { "nicht ok" }
    );
    (ok, total)
}
