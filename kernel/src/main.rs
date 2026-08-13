//! # karst — der Kernel von Karstos
//!
//! Modularer Monolith in Rust (`no_std`). Der Bootweg steht bewusst komplett in
//! dieser Datei, damit die Reihenfolge der Inbetriebnahme an einer Stelle
//! nachvollziehbar ist:
//!
//! 1. serielle Konsole (damit ab Sekunde eins etwas sichtbar ist)
//! 2. Daten des Bootloaders einsammeln und in kerneleigene Typen kopieren
//! 3. Framebuffer-Konsole
//! 4. CPU-Grundstrukturen: GDT/TSS, IDT, Ausnahmen, Interruptcontroller
//! 5. Frame-Allocator aus der Memory-Map
//! 6. EIGENER Adressraum + Umschalten (der Bootloader ist danach entbehrlich)
//! 7. Kernel-Heap, `alloc` funktioniert
//! 8. Zeitgeber und Interrupts freigeben
//!
//! Die Schichten selbst sind streng getrennt, siehe ARCHITECTURE.md.

#![no_std]
#![no_main]
// Begruendung: `extern "x86-interrupt"` ist der einzige Weg, Ausnahmehandler
// ohne handgeschriebene Assembler-Stubs korrekt aufzurufen (Fehlercode,
// Rueckkehr aus der Ausnahme).
#![feature(abi_x86_interrupt)]
// Begruendung: erlaubt einen EIGENEN Handler fuer fehlgeschlagene Allokationen
// (siehe [`mm::heap`]), der Heap-Kennzahlen statt einer nichtssagenden
// Standardmeldung ausgibt.
#![feature(alloc_error_handler)]
#![warn(missing_docs)]

extern crate alloc;

pub mod abi;
pub mod arch;
pub mod boot;
pub mod drivers;
pub mod kcore;
pub mod mm;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use kcore::arch_iface::{AddressSpaceOps, TimerOps};
use kcore::mem::{FrameAllocator, MapFlags, VirtAddr, PAGE_SIZE};

/// Virtuelle Adresse, an der der Selbsttest der Seitentabellen arbeitet.
/// Liegt im selben Eintrag der obersten Tabellenebene wie der Heap, aber weit
/// dahinter.
const PAGING_PROBE: u64 = 0xffff_ff00_1000_0000;

fn fmt_bytes(b: u64) -> (u64, &'static str) {
    if b >= 1 << 30 {
        (b >> 30, "GiB")
    } else if b >= 1 << 20 {
        (b >> 20, "MiB")
    } else if b >= 1 << 10 {
        (b >> 10, "KiB")
    } else {
        (b, "B")
    }
}

/// Nennt die einkompilierten Fehler-Selbsttests im Klartext. Beim normalen Boot
/// ist keiner davon aktiv — das steht dann auch genau so im Log, damit ein
/// Testlauf nicht versehentlich fuer einen normalen Boot gehalten wird.
fn active_selftests() -> &'static str {
    // Genau ein --test-*-Feature ist sinnvoll; mehrere gleichzeitig loesen den
    // erstgenannten aus. Die Aufzaehlung ist bewusst eine feste Kette, damit
    // sie ohne Heap (der zu diesem Zeitpunkt noch fehlt) auskommt.
    if cfg!(feature = "test-pagefault") {
        "test-pagefault"
    } else if cfg!(feature = "test-rodata") {
        "test-rodata"
    } else if cfg!(feature = "test-nx") {
        "test-nx"
    } else if cfg!(feature = "test-gp") {
        "test-gp"
    } else if cfg!(feature = "test-ud") {
        "test-ud"
    } else if cfg!(feature = "test-doublefault") {
        "test-doublefault"
    } else if cfg!(feature = "test-panic") {
        "test-panic"
    } else {
        "keine (normaler Boot)"
    }
}

/// Einsprungpunkt. Der Bootloader uebergibt hier im 64-Bit-Modus.
#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    unsafe { arch::init_serial() };
    serial_println!("");
    serial_println!("================================================================");
    klog!("boot", "karst v{} — Kernel von Karstos", env!("CARGO_PKG_VERSION"));
    klog!("boot", "Architektur : {} (Basisseite {} B)", arch::NAME, arch::PAGE_SIZE);
    klog!("boot", "ABI         : {}", abi::enabled());
    klog!(
        "boot",
        "Konfiguration: Bauart {}, POSIX {}, Fehler-Selbsttest {}",
        if cfg!(debug_assertions) { "debug" } else { "release" },
        if cfg!(feature = "posix") { "ja" } else { "nein" },
        active_selftests()
    );

    let mut brand = [0u8; 64];
    let n = arch::cpu_brand(&mut brand);
    klog!("boot", "CPU         : {}", core::str::from_utf8(&brand[..n]).unwrap_or("?"));

    // ---------------------------------------------------------------- Bootloader
    let (bl_name, bl_ver) = boot::limine::bootloader_name();
    klog!("boot", "Bootloader  : {} {}", bl_name, bl_ver);
    if !boot::limine::base_revision_ok() {
        klog!("boot", "WARNUNG: Basisrevision 3 nicht bestaetigt");
    }
    klog!(
        "boot",
        "Stapel      : {} KiB angefordert, {}",
        boot::limine::STACK_SIZE / 1024,
        if boot::limine::stack_granted() { "gewaehrt" } else { "abgelehnt" }
    );

    let Some(hhdm) = boot::limine::hhdm_offset() else {
        panic!("Bootloader lieferte kein HHDM-Fenster — ohne das ist kein Speicherzugriff moeglich");
    };
    kcore::mem::set_hhdm_offset(hhdm);
    klog!("mm", "HHDM-Fenster: virt = phys + {:#018x}", hhdm);

    let Some((kphys, kvirt)) = boot::limine::kernel_base() else {
        panic!("Bootloader lieferte keine Ladeadresse des Kernels");
    };
    let sections = mm::kernel_sections();
    klog!("mm", "Kernelabbild: phys {:#018x} -> virt {:#018x}", kphys, kvirt);
    klog!(
        "mm",
        "  .text  {:#018x}..{:#018x}  ({} KiB, R+X)",
        sections.text.0,
        sections.text.1,
        (sections.text.1 - sections.text.0) / 1024
    );
    klog!(
        "mm",
        "  .rodata{:#018x}..{:#018x}  ({} KiB, R)",
        sections.rodata.0,
        sections.rodata.1,
        (sections.rodata.1 - sections.rodata.0) / 1024
    );
    klog!(
        "mm",
        "  .data  {:#018x}..{:#018x}  ({} KiB, R+W, NX)",
        sections.data.0,
        sections.data.1,
        (sections.data.1 - sections.data.0) / 1024
    );

    klog!(
        "mm",
        "  gesamt {} KiB geladenes Abbild ({} Seiten a {} B)",
        (sections.data.1 - sections.text.0) / 1024,
        (sections.data.1 - sections.text.0).div_ceil(PAGE_SIZE as u64),
        PAGE_SIZE
    );

    // ------------------------------------------------------------- Framebuffer
    match boot::limine::framebuffer() {
        Some(fb) => {
            let w = fb.width;
            let h = fb.height;
            let bpp = fb.bpp;
            let (cols, rows) = unsafe { drivers::fbcon::init(fb) };
            klog!("video", "Framebuffer : {}x{} @ {} bpp -> Textkonsole {}x{}", w, h, bpp, cols, rows);
        }
        None => klog!("video", "kein Framebuffer gemeldet — nur serielle Ausgabe"),
    }

    // -------------------------------------------------------------------- CPU
    unsafe { arch::init_cpu() };
    klog!("cpu", "GDT + TSS geladen, Datensegmente flach");
    klog!("cpu", "IDT: 256 Tore, Ausnahmen 0..21 belegt");
    klog!("cpu", "#DF laeuft auf eigenem IST-Stapel ({:#018x})", arch::x86_64::gdt::double_fault_stack_top());
    klog!("cpu", "NX aktiv, CR0.WP aktiv");
    klog!(
        "irq",
        "{} initialisiert, alle Leitungen maskiert",
        <arch::IntCtl as kcore::arch_iface::InterruptCtlOps>::name()
    );

    // Trap-Tore beweisen: int3 muss zurueckkehren.
    arch::selftest::breakpoint();
    klog!("cpu", "Selbsttest: #BP ausgeloest und sauber fortgesetzt");

    // ------------------------------------------------------------- Memory-Map
    let regions = unsafe { boot::limine::take_memory_map() };
    klog!("mm", "Memory-Map ({} Eintraege):", regions.len());
    for r in regions.iter() {
        let (v, u) = fmt_bytes(r.len);
        klog!(
            "mm",
            "  {:#014x}..{:#014x}  {:>5} {:<3}  {}",
            r.start,
            r.end(),
            v,
            u,
            r.kind.name()
        );
    }

    let stats = match unsafe { mm::frame::init(regions) } {
        Ok(s) => s,
        Err(e) => panic!("Frame-Allocator konnte nicht starten: {e}"),
    };
    let (tv, tu) = fmt_bytes(stats.summary.total_bytes);
    let (uv, uu) = fmt_bytes(stats.summary.usable_bytes);
    klog!("mm", "Speicher    : {} {} gesamt, davon {} {} nutzbar", tv, tu, uv, uu);
    klog!(
        "mm",
        "Frames      : {} verwaltet, {} frei ({} MiB)",
        stats.total_frames,
        stats.free_frames,
        (stats.free_frames * PAGE_SIZE) >> 20
    );
    klog!(
        "mm",
        "Frame-Bitmap: {} KiB bei phys {:#014x}",
        stats.bitmap_bytes / 1024,
        stats.bitmap_at.as_u64()
    );

    // ------------------------------------------------- eigener Adressraum
    let (mut space, sp) = match unsafe { mm::build_kernel_space(regions, hhdm, kphys, kvirt) } {
        Ok(v) => v,
        Err(e) => panic!("Adressraum konnte nicht gebaut werden: {e:?}"),
    };
    let (hv, hu) = fmt_bytes(sp.hhdm_bytes);
    klog!("mm", "Eigener Adressraum aktiv (CR3 = {:#014x})", space.root().as_u64());
    klog!("mm", "  HHDM   : {} {} als {} grosse Seiten", hv, hu, sp.hhdm_large_pages);
    klog!("mm", "  Abbild : {} Seiten mit getrennten Rechten", sp.image_pages);
    klog!("mm", "  Stapel : {} Seiten vom Bootloader uebernommen", sp.stack_pages);

    // Beweis, dass die Seitentabellen-Abstraktion wirklich arbeitet.
    let paging_ok = {
        let mut g = mm::frame::lock();
        let fa = g.as_mut().expect("Frame-Allocator");
        let frame = fa.alloc_frame().expect("kein Frame frei");
        let v = VirtAddr(PAGING_PROBE);
        unsafe { space.map(v, frame, MapFlags::KERNEL_DATA, &mut *fa) }.expect("map fehlgeschlagen");
        let p = v.as_ptr::<u64>();
        unsafe { core::ptr::write_volatile(p, 0x4B41_5253_544F_5321) };
        let back = unsafe { core::ptr::read_volatile(p) };
        let resolved = space.translate(v);
        let freed = unsafe { space.unmap(v) }.expect("unmap fehlgeschlagen");
        let after = space.translate(v);
        fa.free_frame(freed);
        klog!(
            "mm",
            "Selbsttest Paging: map {:#014x} -> {:#014x}, schreiben/lesen {}, translate {}, unmap {}",
            PAGING_PROBE,
            frame.as_u64(),
            if back == 0x4B41_5253_544F_5321 { "ok" } else { "FEHLER" },
            if resolved == Some(frame) { "ok" } else { "FEHLER" },
            if after.is_none() { "ok" } else { "FEHLER" }
        );
        (back == 0x4B41_5253_544F_5321) as usize
            + (resolved == Some(frame)) as usize
            + after.is_none() as usize
    };

    // Fehlerfaelle der Adressraum-Abstraktion nachweisen (mm/selftest.rs).
    let maperr = unsafe { mm::selftest::map_errors(&mut space) };
    // Zusagen des Frame-Allocators nachweisen (mm/frame.rs).
    let frames_st = mm::frame::selftest();

    // ------------------------------------------------------------------- Heap
    let heap_bytes = {
        let mut g = mm::frame::lock();
        let fa = g.as_mut().expect("Frame-Allocator");
        match unsafe { mm::heap::init(&mut space, &mut *fa) } {
            Ok(n) => n,
            Err(e) => panic!("Heap konnte nicht eingerichtet werden: {e:?}"),
        }
    };
    klog!(
        "heap",
        "Kernel-Heap : {} KiB bei virt {:#018x} (eigener Free-List-Allocator)",
        heap_bytes / 1024,
        mm::heap::HEAP_BASE
    );

    // Nachweis, dass `alloc` wirklich funktioniert.
    {
        let boxed = Box::new(0xDEADBEEFu64);
        let mut v: Vec<u64> = Vec::new();
        for i in 0..1024u64 {
            v.push(i * i);
        }
        let s = String::from("Karstos") + " lebt";
        let sum: u64 = v.iter().sum();
        klog!("heap", "Testallokation: Box@{:p} = {:#x}", &*boxed, *boxed);
        klog!(
            "heap",
            "Testallokation: Vec mit {} Elementen, Summe {}, String \"{}\"",
            v.len(),
            sum,
            s
        );
        klog!("heap", "belegt {} B von {} B", mm::heap::used(), mm::heap::size());
    }
    klog!("heap", "nach Freigabe: belegt {} B von {} B", mm::heap::used(), mm::heap::size());
    // Zusagen des Heaps nachweisen (mm/heap.rs).
    let heap_st = mm::heap::selftest();

    // -------------------------------------------------------------------- ABI
    let (ver, handles, stale) = abi::native::self_test();
    klog!("abi", "karst-native: Version={}, Handles={}, altes Handle -> {}", ver, handles, stale);
    #[cfg(feature = "posix")]
    klog!(
        "abi",
        "posix-Schicht aktiv: read(2) auf unbekannten Fd -> {} (erwartet -9 = -EBADF)",
        abi::posix::self_test()
    );
    #[cfg(not(feature = "posix"))]
    klog!("abi", "posix-Schicht NICHT einkompiliert — Core laeuft ohne sie");

    // ------------------------------------------------------------------ Timer
    unsafe { <arch::Timer as TimerOps>::start(100) };
    arch::enable_interrupts();
    klog!(
        "irq",
        "Zeitgeber laeuft mit {} Hz, Interrupts frei ({})",
        <arch::Timer as TimerOps>::hz(),
        if arch::interrupts_enabled() { "IF gesetzt" } else { "FEHLER: IF nicht gesetzt" }
    );

    // Auf echte Ticks warten — der Beweis, dass Interrupts ankommen.
    for expected in 1..=5u64 {
        let target = expected * <arch::Timer as TimerOps>::hz() as u64;
        while <arch::Timer as TimerOps>::ticks() < target {
            arch::wait_for_interrupt();
        }
        klog!(
            "irq",
            "Tick {} — {} s Laufzeit, {} Ticks gezaehlt",
            expected,
            expected,
            <arch::Timer as TimerOps>::ticks()
        );
    }

    // ------------------------------------------------------------- Scheduler
    // Phase 2: Kontextwechsel. Meldet ehrlich, ob eine Implementierung vorhanden
    // ist (kcore/sched.rs), und laeuft in jedem Fall nach kmain zurueck.
    kcore::sched::boot_selftest();

    // ------------------------------------------------------------- Startbilanz
    // Eine einzige, maschinell pruefbare Zeile mit dem Ergebnis aller
    // Selbsttests dieses Boots (run-qemu.sh --check prueft sie).
    // Gezaehlt werden: #BP-Rueckkehr (1), Paging-Selbsttest (3 Zusagen),
    // MapError-Faelle, Frame-Zusagen, Heap-Zusagen.
    let bestanden = 1 + paging_ok + maperr.0 + frames_st.0 + heap_st.0;
    let gesamt = 1 + 3 + maperr.1 + frames_st.1 + heap_st.1;
    klog!(
        "boot",
        "Selbsttestbilanz: {}/{} bestanden (#BP 1/1, Paging {}/3, MapError {}/{}, Frames {}/{}, Heap {}/{})",
        bestanden,
        gesamt,
        paging_ok,
        maperr.0,
        maperr.1,
        frames_st.0,
        frames_st.1,
        heap_st.0,
        heap_st.1
    );
    klog!(
        "boot",
        "Startbilanz : {} Frames frei ({} belegt), Heap {} / {} B belegt, {} Ticks",
        mm::frame::free_frames(),
        mm::frame::used_frames(),
        mm::heap::used(),
        mm::heap::size(),
        <arch::Timer as TimerOps>::ticks()
    );
    if bestanden != gesamt {
        klog!("boot", "WARNUNG: {} Selbsttest(s) NICHT bestanden", gesamt - bestanden);
    }

    klog!("boot", "Startvorgang abgeschlossen. karst laeuft.");
    serial_println!("================================================================");

    // ------------------------------------------------------- Fehler-Selbsttests
    #[cfg(feature = "test-pagefault")]
    {
        klog!("test", "loese absichtlich einen Page Fault aus ...");
        unsafe { arch::selftest::page_fault() };
    }
    #[cfg(feature = "test-rodata")]
    {
        klog!("test", "versuche in .rodata zu schreiben (muss #PF ausloesen) ...");
        unsafe { arch::selftest::rodata_write() };
    }
    #[cfg(feature = "test-nx")]
    {
        klog!("test", "versuche Daten in .data auszufuehren (NX muss #PF ausloesen) ...");
        unsafe { arch::selftest::nx_exec() };
    }
    #[cfg(feature = "test-gp")]
    {
        klog!("test", "greife auf eine nicht kanonische Adresse zu (muss #GP ausloesen) ...");
        unsafe { arch::selftest::general_protection() };
    }
    #[cfg(feature = "test-ud")]
    {
        klog!("test", "fuehre eine ungueltige Instruktion aus (muss #UD ausloesen) ...");
        unsafe { arch::selftest::invalid_opcode() };
    }
    #[cfg(feature = "test-doublefault")]
    {
        klog!("test", "loese absichtlich einen Double Fault aus ...");
        unsafe { arch::selftest::double_fault() };
    }
    #[cfg(feature = "test-panic")]
    {
        klog!("test", "loese absichtlich ein panic! aus ...");
        panic!("Selbsttest des Panic-Handlers");
    }

    idle()
}

/// Leerlauf: warten, bis etwas passiert, und jede Sekunde ein Lebenszeichen.
fn idle() -> ! {
    let hz = <arch::Timer as TimerOps>::hz() as u64;
    let mut next = <arch::Timer as TimerOps>::ticks() + hz * 5;
    loop {
        arch::wait_for_interrupt();
        let t = <arch::Timer as TimerOps>::ticks();
        if hz > 0 && t >= next {
            klog!("idle", "Laufzeit {} s, Heap {} / {} B belegt", t / hz, mm::heap::used(), mm::heap::size());
            next = t + hz * 5;
        }
    }
}
