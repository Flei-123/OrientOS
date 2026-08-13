//! Unprivilegierte Ebene von x86_64 — der Privilegwechsel selbst.
//!
//! Hier steht (und NUR hier):
//! * die Freischaltung des schnellen Systemaufrufpfads ueber die
//!   Modellregister STAR/LSTAR/SFMASK und das SCE-Bit in EFER,
//! * der Per-CPU-Block samt Basisregisterwechsel (`swapgs`) beim Ein- und
//!   Austritt,
//! * getrennte Kernel-/User-Stapel und der Ring-0-Eintrag im Aufgabensegment
//!   (`super::gdt::set_kernel_stack`),
//! * die Schutzbits SMEP/SMAP in CR4, sofern CPUID sie meldet, samt dem
//!   Fenster (`stac`/`clac`), durch das der Kernel unprivilegierte Daten
//!   trotzdem lesen darf,
//! * der Einsprung nach Ring 3 (`iretq`), der Ruecksprung aus einem
//!   Systemaufruf (`sysretq`) und der Notausstieg, wenn das Programm eine
//!   Ausnahme ausloest,
//! * der **Schutzwall**: fuenf weitere Einsprungpunkte im selben Bild, die je
//!   einen Ausbruchsversuch aus Ring 3 unternehmen (privilegierte Instruktion,
//!   Schreiben auf die eigene Codeseite, Sprung in den Kernelbereich, Zeiger
//!   ins Leere, Rechenschleife ohne Systemaufruf). Der Core kennt davon nur
//!   Versatz, Klartextname und erwarteten Ausgang
//!   ([`crate::kcore::user::UserProbe`]),
//! * das Beispielprogramm als reines Maschinenbild — Maschinencode ist
//!   Architektursache und darf deshalb in keiner core-Datei liegen. Der Core
//!   bekommt ihn ueber [`crate::kcore::user::ArchSupport`] als Bytes plus
//!   zwei Einsprungversaetzen und weiss nicht, was darin steht.
//!
//! **Nachweis der Schutzbits.** SMEP/SMAP werden nur gesetzt, wenn CPUID sie
//! meldet. Das QEMU-Standardmodell (`qemu64`) meldet sie NICHT — liefe jeder
//! Test damit, waere die CR4-Logik hier toter Code. `run-qemu.sh` startet
//! deshalb standardmaessig mit `-cpu max`, und im Boot-Log jedes Laufs steht:
//!
//! ```text
//! ==> [..] CPU-Schutz  : Schnellaufruf ja, Ausfuehrsperre ja,
//!                        Zugriffssperre ja, Per-CPU-Basis ja
//! ==> [..] Schutzwall  : 5/5 Uebergriffe aus Ring 3 abgewehrt
//! ```
//!
//! Der ANDERE Pfad (CPU ohne die Bits, Kernel ueberspringt sie und vermerkt
//! das) wird eigens geprueft: `tests/step-17-ring3.sh` startet dasselbe Abbild
//! ein zweites Mal mit `--cpu-basic` und erwartet dort
//! "Ausfuehrsperre nein (uebersprungen)" bei sonst gleichem Ergebnis.
//!
//! Mit gesetztem CR4.SMAP laufen alle Systemaufrufe, die unprivilegierten
//! Speicher anfassen, weiterhin durch — das Fenster (`stac`/`clac`) sitzt also
//! an den richtigen Stellen.
//!
//! **Ablauf eines Ausflugs nach Ring 3**
//!
//! ```text
//!   kcore::user::run
//!     -> enter()            Messwerte zuruecksetzen, Kernelstapel eintragen
//!        -> enter_ring3()   callee-saved retten, rsp merken, Rahmen bauen,
//!                           swapgs, iretq  ==> CPL 3
//!           ~ int3          #BP misst CS/RSP des Programms (echte Hardware)
//!           ~ syscall       syscall_entry: swapgs, Kernelstapel, Verteiler
//!           ~ Ausnahme      abort_user(): Programm beenden, Kernel lebt
//!        <- resume_kernel() rsp zurueck, callee-saved zurueck, `ret`
//! ```

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::{cpu, gdt, msr};
use crate::kcore::arch_iface::{CpuFeatures, UserExit};
use crate::kcore::mem::VirtAddr;
use crate::kcore::user::{ArchSupport, ProbeExpect, SyscallOutcome, UserProbe, MAX_PROBES};

// ---------------------------------------------------------------- Selektoren

/// Codesegment-Selektor der unprivilegierten Ebene (Ring 3 = RPL 3).
const USER_CS: u64 = (gdt::USER_CODE | 3) as u64;
/// Datensegment-Selektor der unprivilegierten Ebene.
const USER_SS: u64 = (gdt::USER_DATA | 3) as u64;
/// Basis, aus der `sysretq` die Selektoren ableitet: Basis+8 = Daten,
/// Basis+16 = Code. Mit `USER_DATA = 0x18` und `USER_CODE = 0x20` ist das
/// genau `0x10` — deshalb darf die Reihenfolge in der GDT nicht wandern.
const SYSRET_BASE: u64 = gdt::USER_DATA as u64 - 8;
/// RFLAGS des Programms: reserviertes Bit 1 und freigegebene Interrupts.
const USER_RFLAGS: u64 = 0x202;

// ------------------------------------------------------------ Modellregister

/// `IA32_STAR` — Selektoren fuer `syscall`/`sysret`.
const IA32_STAR: u32 = 0xC000_0081;
/// `IA32_LSTAR` — Einsprungadresse von `syscall`.
const IA32_LSTAR: u32 = 0xC000_0082;
/// `IA32_FMASK` — Flags, die `syscall` loescht.
const IA32_FMASK: u32 = 0xC000_0084;
/// `IA32_GS_BASE` — Basis des Per-CPU-Blocks im Kernel.
const IA32_GS_BASE: u32 = 0xC000_0101;
/// `IA32_KERNEL_GS_BASE` — die von `swapgs` eingetauschte Gegenbasis.
const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;
/// SCE-Bit in `IA32_EFER`: ohne dieses Bit ist `syscall` eine ungueltige
/// Instruktion.
const EFER_SCE: u64 = 1;
/// Flags, die beim Systemaufruf geloescht werden: IF (kein Interrupt im
/// Einsprung), TF, DF und AC (die Zugriffssperre bleibt scharf).
const FMASK_BITS: u64 = 0x4_0700;

/// SMEP-Bit in CR4 — der Kernel darf keinen unprivilegierten Code ausfuehren.
const CR4_SMEP: u64 = 1 << 20;
/// SMAP-Bit in CR4 — der Kernel darf unprivilegierte Daten nur im offenen
/// Fenster (`stac`/`clac`) anfassen.
const CR4_SMAP: u64 = 1 << 21;

// --------------------------------------------------------------- Per-CPU

/// Per-CPU-Block, auf den das GS-Basisregister zeigt.
///
/// Feste Versaetze, weil der Einsprung sie als Konstanten braucht:
/// `gs:[0]` = Kernelstapel, `gs:[8]` = abgelegter unprivilegierter Stapel.
#[repr(C, align(64))]
struct PerCpu {
    kernel_rsp: u64,
    user_rsp: u64,
}

static mut PERCPU: PerCpu = PerCpu { kernel_rsp: 0, user_rsp: 0 };

/// Groesse des Kernelstapels, auf den ein Systemaufruf umschaltet.
const SYSCALL_STACK_SIZE: usize = 16 * 1024;

#[repr(C, align(16))]
struct SyscallStack([u8; SYSCALL_STACK_SIZE]);

static mut SYSCALL_STACK: SyscallStack = SyscallStack([0; SYSCALL_STACK_SIZE]);

// ----------------------------------------------------------------- Zustand

/// Ist der Weg nach Ring 3 eingerichtet?
static READY: AtomicBool = AtomicBool::new(false);
/// Laeuft gerade ein unprivilegiertes Programm?
static IN_USER: AtomicBool = AtomicBool::new(false);
/// Gemessener Codesegment-Selektor des Programms (0 = noch nichts gemessen).
static MEASURED_CS: AtomicU64 = AtomicU64::new(0);
/// Gemessener Stapelzeiger des Programms.
static MEASURED_RSP: AtomicU64 = AtomicU64::new(0);
/// Systemaufrufe des laufenden Programms.
static SYSCALLS: AtomicU64 = AtomicU64::new(0);
/// Beendigungscode des Programms.
static EXIT_CODE: AtomicU64 = AtomicU64::new(0);
/// Wurde das Programm wegen einer Ausnahme abgebrochen?
static ABORTED: AtomicBool = AtomicBool::new(false);
/// Ticks, die das laufende Programm bereits verbraucht hat (Wachhund).
static USER_TICKS: AtomicU64 = AtomicU64::new(0);
/// Gesicherter Kernelstapelzeiger von [`enter_ring3`].
static mut SAVED_RSP: u64 = 0;
/// Sind die Schutzbits SMEP/SMAP tatsaechlich eingeschaltet?
static SMAP_ON: AtomicBool = AtomicBool::new(false);
/// Darf das laufende unprivilegierte Programm verdraengt und SPAETER
/// FORTGESETZT werden?
///
/// Steht der Schalter, dann
/// * laesst der Zeitgeberpfad das Programm nicht abraeumen (kein Wachhund),
/// * liegt der Rahmen eines Privilegwechsels auf dem Kernelstapel des
///   AUFRUFENDEN Threads (siehe [`enter`]) statt auf dem gemeinsamen
///   Systemaufrufstapel — nur dann darf mitten im Rahmen umgeschaltet werden.
static PREEMPTIBLE: AtomicBool = AtomicBool::new(false);
/// Abstand zwischen dem Stapelzeiger des aufrufenden Threads und der Stelle,
/// an der die CPU den Rahmen eines Privilegwechsels ablegt. Muss groesser sein
/// als alles, was [`enter_ring3`] unterhalb von [`enter`] noch ablegt (dort
/// sind es 11 Worte).
const TRAP_GAP: u64 = 512;

/// Hoechstzahl Zeitgeber-Ticks, die ein Programm laufen darf, bevor der
/// Kernel es abraeumt. Ohne diese Grenze wuerde eine Endlosschleife im
/// unprivilegierten Programm den ganzen Startvorgang aufhalten.
const USER_TICK_LIMIT: u64 = 10;

// --------------------------------------------------- Beispielprogramm (Ring 3)

core::arch::global_asm!(
    r#"
    .section .rodata.userdemo,"a"
    .balign 16
    .globl __user_demo_start
__user_demo_start:
    /* Einsprung 1: brav. Meldet sich, gibt etwas aus, beendet sich.
       Der Kernel uebergibt das Handle des Ausgabestroms im ersten
       Argumentregister — es gibt keine "immer vorhandene" Ausgabe, ohne
       Handle kein Zugriff. */
    .globl __user_demo_main
__user_demo_main:
    mov r12, rdi                          /* Handle des Ausgabestroms (rdi)    */
    int3                                  /* Kernel misst CS/CPL/RSP am Rahmen */
    xor eax, eax                          /* 0 = version (Lebenszeichen)       */
    syscall
    mov eax, 2                            /* 2 = handle_write                  */
    mov rdi, r12
    lea rsi, [rip + __user_demo_msg]      /* lageunabhaengig                   */
    lea rdx, [rip + __user_demo_msg_end]  /* Laenge lageunabhaengig rechnen    */
    sub rdx, rsi
    syscall
    mov eax, 2                            /* Probe: Kernelzeiger als Puffer    */
    mov rdi, r12
    movabs rsi, 0xffffffff80000000
    mov edx, 8
    syscall                               /* muss abgewiesen werden            */
    mov eax, 18                           /* 18 = process_exit                 */
    xor edi, edi                          /* eigener Prozess                   */
    xor esi, esi                          /* Beendigungscode 0                 */
    syscall
9:  jmp 9b                                /* wird nie erreicht                 */

    /* Einsprung 3: Messsprungbrett fuer FREMDE Programme.
       Der Kernel legt die echte Einsprungadresse als oberstes Stapelwort ab
       und springt hierher. Das `int3` gibt ihm einen echten Ausnahmerahmen —
       damit sind Selektor, Privilegstufe und Stapel eines beliebigen
       Programms an der Hardware gemessen und nicht behauptet. Danach geht es
       ohne Umweg in das Programm. */
    .balign 16
    .globl __user_demo_trampoline
__user_demo_trampoline:
    pop rax                               /* echte Einsprungadresse            */
    int3                                  /* Messpunkt                         */
    jmp rax

    /* Einsprung 2: Negativtest. Greift auf eine Kerneladresse zu. */
    .balign 16
    .globl __user_demo_fault
__user_demo_fault:
    movabs rax, 0xffffffff80000000
    mov rax, [rax]                        /* muss #PF ausloesen                */
8:  jmp 8b

    /* --------------------------------------------------------- Schutzwall
       Vier weitere Uebergriffe, jeder in einem eigenen Einsprung. Keiner
       davon darf gelingen, keiner darf den Kernel anhalten. Sie decken die
       vier Wege ab, auf denen ein unprivilegiertes Programm ausbrechen
       koennte: privilegierte Instruktion, Schreiben auf die eigene
       Codeseite (W^X), Ausfuehren von Kernelcode und ein Zeiger ins Leere. */

    .balign 16
    .globl __user_demo_priv
__user_demo_priv:
    cli                                   /* privilegiert -> #GP bei CPL 3     */
7:  jmp 7b

    .balign 16
    .globl __user_demo_wcode
__user_demo_wcode:
    lea rax, [rip + __user_demo_start]
    mov byte ptr [rax], 0x90              /* eigene RX-Seite beschreiben       */
6:  jmp 6b

    .balign 16
    .globl __user_demo_kjump
__user_demo_kjump:
    movabs rax, 0xffffffff80000000
    jmp rax                               /* Instruktionsabruf im Kernel       */

    .balign 16
    .globl __user_demo_hole
__user_demo_hole:
    movabs rax, 0x0000000000600000
    mov rax, [rax]                        /* nicht abgebildet -> #PF           */
5:  jmp 5b

    /* Wachhund-Probe: rechnet lange vor sich hin, ohne je einen
       Systemaufruf abzusetzen. Der Zeitgeber muss dem Programm die CPU
       wegnehmen und der Kernel es abraeumen, BEVOR die Schleife durch ist.
       Laeuft sie doch durch, beendet sich das Programm mit einem eigenen
       Code — dann ist der Wachhund nachweislich nicht angesprungen und der
       Kernel meldet das ehrlich, statt haengen zu bleiben. */
    .balign 16
    .globl __user_demo_spin
__user_demo_spin:
    movabs rcx, 2000000000
4:  dec rcx
    jnz 4b
    mov eax, 18                           /* 18 = process_exit                 */
    xor edi, edi                          /* eigener Prozess                   */
    mov esi, 7                            /* 7 = "Wachhund hat geschlafen"     */
    syscall
3:  jmp 3b

    /* Verdraengungs-Probe: rechnet lange, ohne je einen Systemaufruf
       abzusetzen — beendet sich am Ende aber REGULAER mit Code 0. Sie ist der
       Gegenpol zur Wachhund-Probe: hier soll der Zeitgeber das Programm
       NICHT abraeumen, sondern nur unterbrechen und spaeter fortsetzen.
       Kommt Code 0 zurueck, hat das Programm jede Unterbrechung ueberlebt. */
    .balign 16
    .globl __user_demo_pspin
__user_demo_pspin:
    movabs rcx, 30000000
2:  dec rcx
    jnz 2b
    mov eax, 18                           /* 18 = process_exit                 */
    xor edi, edi                          /* eigener Prozess                   */
    xor esi, esi                          /* Beendigungscode 0                 */
    syscall
1:  jmp 1b

    .balign 8
__user_demo_msg:
    .ascii "Gruesse aus Ring 3"
__user_demo_msg_end:
    .balign 16
    .globl __user_demo_end
__user_demo_end:
    "#
);

unsafe extern "C" {
    static __user_demo_start: u8;
    static __user_demo_main: u8;
    static __user_demo_fault: u8;
    static __user_demo_trampoline: u8;
    static __user_demo_priv: u8;
    static __user_demo_wcode: u8;
    static __user_demo_kjump: u8;
    static __user_demo_hole: u8;
    static __user_demo_spin: u8;
    static __user_demo_pspin: u8;
    static __user_demo_end: u8;
}

/// Beendigungscode, mit dem sich die Wachhund-Probe selbst beendet, wenn ihr
/// niemand die CPU wegnimmt. Sieht der Core diesen Code, hat der Wachhund
/// nicht ausgeloest.
const SPIN_SELF_EXIT: u64 = 7;

/// Das Beispielprogramm als Bytes plus seine vier Einsprungversaetze.
fn demo_image() -> (&'static [u8], usize, usize, usize, usize) {
    let start = &raw const __user_demo_start as usize;
    let end = &raw const __user_demo_end as usize;
    let main = &raw const __user_demo_main as usize;
    let fault = &raw const __user_demo_fault as usize;
    // SAFETY: Die Symbole umschliessen einen zusammenhaengenden, nur lesbaren
    // Block im Kernelabbild; er wird ausschliesslich kopiert.
    let tramp = &raw const __user_demo_trampoline as usize;
    let pspin = &raw const __user_demo_pspin as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    (bytes, main - start, fault - start, tramp - start, pspin - start)
}

/// Die Uebergriffe, die das Bild anbietet, samt erwartetem Ausgang.
///
/// Die Namen sind Klartext fuer das Boot-Log; welche Instruktion dahinter
/// steckt, bleibt in dieser Datei.
fn demo_probes() -> ([UserProbe; MAX_PROBES], usize) {
    let start = &raw const __user_demo_start as usize;
    let off = |sym: *const u8| sym as usize - start;
    let mut p = [UserProbe::NONE; MAX_PROBES];
    p[0] = UserProbe {
        offset: off(&raw const __user_demo_priv),
        name: "privilegierte Instruktion",
        expect: ProbeExpect::Abgewiesen,
    };
    p[1] = UserProbe {
        offset: off(&raw const __user_demo_wcode),
        name: "Schreiben auf die eigene Codeseite",
        expect: ProbeExpect::Abgewiesen,
    };
    p[2] = UserProbe {
        offset: off(&raw const __user_demo_kjump),
        name: "Sprung in den Kernelbereich",
        expect: ProbeExpect::Abgewiesen,
    };
    p[3] = UserProbe {
        offset: off(&raw const __user_demo_hole),
        name: "Zeiger auf eine nicht abgebildete Seite",
        expect: ProbeExpect::Abgewiesen,
    };
    p[4] = UserProbe {
        offset: off(&raw const __user_demo_spin),
        name: "Rechenschleife ohne Systemaufruf",
        expect: ProbeExpect::Wachhund { selbstende: SPIN_SELF_EXIT },
    };
    (p, 5)
}

// ------------------------------------------------------------------ Merkmale

/// Was die CPU an Schutzmerkmalen meldet.
pub fn features() -> CpuFeatures {
    let (max_ext, _, _, _) = cpu::cpuid_count(0x8000_0000, 0);
    let ext_edx = if max_ext >= 0x8000_0001 { cpu::cpuid_count(0x8000_0001, 0).3 } else { 0 };
    let (max_leaf, _, _, _) = cpu::cpuid_count(0, 0);
    let leaf7_ebx = if max_leaf >= 7 { cpu::cpuid_count(7, 0).1 } else { 0 };
    CpuFeatures {
        // Bit 11: SYSCALL/SYSRET im 64-Bit-Modus.
        fast_syscall: ext_edx & (1 << 11) != 0,
        // Bit 7 in Blatt 7: SMEP.
        exec_guard: leaf7_ebx & (1 << 7) != 0,
        // Bit 20 in Blatt 7: SMAP.
        access_guard: leaf7_ebx & (1 << 20) != 0,
        // Bit 29: Long Mode — dann gibt es die GS-Basisregister und `swapgs`.
        percpu_base: ext_edx & (1 << 29) != 0,
    }
}

// -------------------------------------------------------------- Einrichtung

/// Richtet den Systemaufrufpfad ein.
///
/// # Safety
/// Genau einmal im Boot, nach der CPU-Einrichtung.
pub unsafe fn init() -> bool {
    let f = features();
    if !f.user_capable() {
        return false;
    }
    unsafe {
        // Kernelstapel fuer Systemaufrufe und Ausnahmen aus Ring 3.
        let top = (&raw const SYSCALL_STACK as u64 + SYSCALL_STACK_SIZE as u64) & !0xF;
        let pc = &raw mut PERCPU;
        (*pc).kernel_rsp = top;
        (*pc).user_rsp = 0;
        gdt::set_kernel_stack(top);

        // Per-CPU-Basis: im Kernel zeigt GS auf den Block, in Ring 3 auf 0.
        msr::wrmsr(IA32_GS_BASE, pc as u64);
        msr::wrmsr(IA32_KERNEL_GS_BASE, 0);

        // Selektoren: [47:32] fuer `syscall` (Kernelcode), [63:48] fuer
        // `sysret` (Basis der unprivilegierten Selektoren).
        msr::wrmsr(IA32_STAR, (SYSRET_BASE << 48) | ((gdt::KERNEL_CODE as u64) << 32));
        let entry: unsafe extern "C" fn() = syscall_entry;
        msr::wrmsr(IA32_LSTAR, entry as usize as u64);
        msr::wrmsr(IA32_FMASK, FMASK_BITS);
        let efer = msr::rdmsr(msr::IA32_EFER);
        msr::wrmsr(msr::IA32_EFER, efer | EFER_SCE);

        // Schutzbits, soweit vorhanden. Fehlen sie, wird das nur vermerkt.
        let mut cr4 = cpu::read_cr4();
        if f.exec_guard {
            cr4 |= CR4_SMEP;
        }
        if f.access_guard {
            cr4 |= CR4_SMAP;
        }
        if cr4 != cpu::read_cr4() {
            cpu::write_cr4(cr4);
        }
        SMAP_ON.store(f.access_guard, Ordering::Release);
    }

    READY.store(true, Ordering::Release);

    // Dem Core sagen, womit er arbeiten kann. Der Core kennt nur Bytes und
    // zwei Versaetze — kein Maschinendetail wandert nach oben.
    let (bytes, main_off, fault_off, tramp_off, pspin_off) = demo_image();
    let (probes, probe_count) = demo_probes();
    crate::kcore::user::register_arch_support(ArchSupport {
        program: bytes,
        entry_offset: main_off,
        fault_offset: fault_off,
        trampoline_offset: tramp_off,
        preempt_offset: pspin_off,
        open_window: open_user_window,
        close_window: close_user_window,
        probes,
        probe_count,
    });
    true
}

/// Ist der Weg in die unprivilegierte Ebene fertig verdrahtet?
pub fn ready() -> bool {
    READY.load(Ordering::Acquire)
}

/// Laeuft gerade ein unprivilegiertes Programm?
pub fn in_user() -> bool {
    IN_USER.load(Ordering::Acquire)
}

/// Schaltet um, ob das NAECHSTE unprivilegierte Programm verdraengbar ist.
///
/// Liefert `true`, weil diese Architektur es anbietet. Der Aufrufer muss aus
/// einem Thread mit eigenem Kernelstapel kommen — sonst laege der Rahmen des
/// Privilegwechsels auf einem Stapel, der beim Umschalten weiterbenutzt wird.
pub fn set_preemptible(on: bool) -> bool {
    PREEMPTIBLE.store(on, Ordering::Release);
    true
}

/// Darf das laufende Programm verdraengt und fortgesetzt werden?
pub fn preemptible() -> bool {
    PREEMPTIBLE.load(Ordering::Acquire)
}

/// Hinterlegt den Kernelstapel fuer den naechsten Privilegwechsel.
pub fn set_kernel_stack(top: VirtAddr) {
    let t = top.as_u64() & !0xF;
    // SAFETY: reine Zuweisung im Per-CPU-Block bzw. im TSS; beide gehoeren
    // dieser Datei.
    unsafe {
        (*(&raw mut PERCPU)).kernel_rsp = t;
        gdt::set_kernel_stack(t);
    }
}

/// Oeffnet das Fenster, durch das der Kernel unprivilegierte Daten lesen darf.
///
/// Ohne SMAP passiert nichts — dann steht das Fenster ohnehin offen.
fn open_user_window() {
    if SMAP_ON.load(Ordering::Relaxed) {
        // SAFETY: setzt nur das AC-Flag; wird von `close_user_window` wieder
        // geloescht und vom Systemaufrufpfad ohnehin maskiert.
        unsafe { core::arch::asm!("stac", options(nomem, nostack)) };
    }
}

/// Schliesst das Fenster wieder.
fn close_user_window() {
    if SMAP_ON.load(Ordering::Relaxed) {
        // SAFETY: loescht nur das AC-Flag.
        unsafe { core::arch::asm!("clac", options(nomem, nostack)) };
    }
}

/// Codesegment-Selektor des laufenden Kontrollflusses.
pub fn code_selector() -> u16 {
    let cs: u16;
    // SAFETY: Lesezugriff auf ein Segmentregister.
    unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags)) };
    cs
}

// ----------------------------------------------------------- Messung/Abbruch

/// Nimmt einen Ausnahmerahmen entgegen, der aus Ring 3 stammt, und merkt sich
/// die gemessenen Werte. Das ist der Beleg, dass wirklich unprivilegiert
/// gelaufen wurde: Selektor und Stapel kommen von der Hardware, nicht aus
/// einer Behauptung des Kernels.
pub fn note_user_frame(cs: u64, rsp: u64) {
    MEASURED_CS.store(cs, Ordering::Relaxed);
    MEASURED_RSP.store(rsp, Ordering::Relaxed);
}

/// Zaehlt einen Zeitgebertick, der ein unprivilegiertes Programm getroffen hat.
/// Liefert `true`, wenn das Programm sein Zeitbudget ueberschritten hat.
pub fn note_user_tick() -> bool {
    // Ein verdraengbares Programm wird unterbrochen, nicht abgeraeumt — der
    // Wachhund haette dort nichts zu suchen.
    IN_USER.load(Ordering::Acquire)
        && !PREEMPTIBLE.load(Ordering::Acquire)
        && USER_TICKS.fetch_add(1, Ordering::Relaxed) + 1 >= USER_TICK_LIMIT
}

/// Bricht das laufende unprivilegierte Programm ab und kehrt in den Kernel
/// zurueck. Wird aus einem Ausnahme- oder Interruptpfad gerufen, der in Ring 3
/// unterbrochen hat.
///
/// # Safety
/// Nur gueltig, solange [`in_user`] wahr ist: der Aufruf verlaesst den
/// laufenden Ausnahmepfad und setzt den Kernel dort fort, wo
/// [`enter_ring3`] ihn verlassen hat.
pub unsafe fn abort_user(code: u64) -> ! {
    ABORTED.store(true, Ordering::Release);
    EXIT_CODE.store(code, Ordering::Release);
    IN_USER.store(false, Ordering::Release);
    // SAFETY: `SAVED_RSP` wurde von `enter_ring3` gesetzt und ist gueltig,
    // solange `IN_USER` galt. Die Gegenbasis wird zurueckgetauscht, weil der
    // Ausnahmepfad aus Ring 3 kam.
    unsafe { resume_kernel_after_fault(SAVED_RSP) }
}

// --------------------------------------------------------------- Einsprung

/// Startet `entry` unprivilegiert auf `stack_top`.
///
/// `stack_top` wird UNVERAENDERT uebernommen — der Aufrufer bestimmt damit
/// auch, was das Programm dort vorfindet (z. B. ein uebergebenes Handle als
/// oberstes Wort). Um die Ausrichtung kuemmert sich der Aufrufer.
///
/// # Safety
/// Beide Adressen muessen im aktiven Adressraum unprivilegiert erreichbar
/// abgebildet sein.
pub unsafe fn enter(entry: VirtAddr, stack_top: VirtAddr) -> UserExit {
    if !ready() {
        return UserExit {
            reason: "unprivilegierte Ebene nicht eingerichtet",
            ..UserExit::default()
        };
    }
    MEASURED_CS.store(0, Ordering::Relaxed);
    MEASURED_RSP.store(0, Ordering::Relaxed);
    SYSCALLS.store(0, Ordering::Relaxed);
    EXIT_CODE.store(0, Ordering::Relaxed);
    USER_TICKS.store(0, Ordering::Relaxed);
    ABORTED.store(false, Ordering::Relaxed);
    IN_USER.store(true, Ordering::Release);

    // Verdraengbarer Lauf: der Rahmen, den die CPU beim Wechsel von Ring 3
    // nach Ring 0 ablegt, muss auf dem Kernelstapel GENAU DIESES Threads
    // landen. Nur dann darf der Zeitgeberpfad mitten im Rahmen auf einen
    // anderen Thread umschalten — der Rahmen bleibt dann liegen, bis dieser
    // Thread wieder an der Reihe ist, und `iretq` setzt das Programm fort.
    // Auf dem gemeinsamen Systemaufrufstapel waere er dagegen beim naechsten
    // Wechsel ueberschrieben worden.
    let voriger_stapel = if PREEMPTIBLE.load(Ordering::Acquire) {
        let sp: u64;
        // SAFETY: liest nur den eigenen Stapelzeiger.
        unsafe {
            core::arch::asm!("mov {}, rsp", out(reg) sp, options(nomem, nostack, preserves_flags))
        };
        // SAFETY: liest ein Wort aus dem Per-CPU-Block dieser Datei.
        let alt = unsafe { (*(&raw const PERCPU)).kernel_rsp };
        set_kernel_stack(VirtAddr::new(sp - TRAP_GAP));
        Some(alt)
    } else {
        None
    };

    // SAFETY: Der Aufrufer buergt fuer die Abbildungen; der Ruecksprung laeuft
    // ueber `resume_kernel*` und stellt genau diesen Stapel wieder her.
    unsafe {
        enter_ring3(
            entry.as_u64(),
            stack_top.as_u64(),
            &raw mut SAVED_RSP,
            crate::kcore::user::entry_arg(),
        )
    };

    IN_USER.store(false, Ordering::Release);
    if let Some(alt) = voriger_stapel {
        set_kernel_stack(VirtAddr::new(alt));
    }
    let cs = MEASURED_CS.load(Ordering::Relaxed);
    UserExit {
        entered: cs != 0 || SYSCALLS.load(Ordering::Relaxed) > 0,
        code: EXIT_CODE.load(Ordering::Relaxed),
        code_selector: cs as u16,
        privilege_level: (cs & 3) as u8,
        stack_pointer: MEASURED_RSP.load(Ordering::Relaxed),
        syscalls: SYSCALLS.load(Ordering::Relaxed),
        reason: if ABORTED.load(Ordering::Relaxed) {
            "durch eine Ausnahme abgeraeumt"
        } else {
            "regulaer beendet"
        },
    }
}

/// Wurde der letzte Ausflug durch eine Ausnahme beendet?
pub fn aborted() -> bool {
    ABORTED.load(Ordering::Acquire)
}

/// Wechsel nach Ring 3.
///
/// Rettet die callee-saved Register, merkt den Stapelzeiger in `*save`, baut
/// den Rahmen fuer den Ruecksprung aus einer Ausnahme (SS, RSP, RFLAGS, CS,
/// RIP), tauscht die Per-CPU-Basis und springt hinaus. Alle Register, die
/// nicht Teil des Rahmens sind, werden geloescht — das Programm soll nichts
/// aus dem Kernel geschenkt bekommen. Einzige Ausnahme ist `arg`: der
/// Uebergabewert des Kernels landet im ersten Argumentregister (`rdi`), so
/// wie ein Systemaufruf sein erstes Argument erwartet.
///
/// # Safety
/// Nur aus [`enter`] aufrufen.
#[unsafe(naked)]
unsafe extern "C" fn enter_ring3(entry: u64, stack_top: u64, save: *mut u64, arg: u64) {
    core::arch::naked_asm!(
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdx], rsp",
        "push {ss}",
        "push rsi",
        "push {flags}",
        "push {cs}",
        "push rdi",
        // Viertes Argument (`arg`) steht nach SysV in rcx; es wandert als
        // einziger Wert mit nach Ring 3 und liegt dort im ersten
        // Argumentregister.
        "mov rdi, rcx",
        "xor eax, eax",
        "xor ebx, ebx",
        "xor ecx, ecx",
        "xor edx, edx",
        "xor esi, esi",
        "xor ebp, ebp",
        "xor r8d, r8d",
        "xor r9d, r9d",
        "xor r10d, r10d",
        "xor r11d, r11d",
        "xor r12d, r12d",
        "xor r13d, r13d",
        "xor r14d, r14d",
        "xor r15d, r15d",
        "swapgs",
        "iretq",
        ss = const USER_SS,
        cs = const USER_CS,
        flags = const USER_RFLAGS,
    )
}

/// Setzt den Kernel dort fort, wo [`enter_ring3`] ihn verlassen hat.
///
/// # Safety
/// `saved_rsp` muss der von [`enter_ring3`] gemerkte Stapelzeiger sein, und
/// die Per-CPU-Basis muss bereits die des Kernels sein.
#[unsafe(naked)]
unsafe extern "C" fn resume_kernel(saved_rsp: u64) -> ! {
    core::arch::naked_asm!(
        "mov rsp, rdi",
        "xor eax, eax",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "ret",
    )
}

/// Wie [`resume_kernel`], aber aus einem Ausnahmepfad heraus, der in Ring 3
/// unterbrochen hat: dort zeigt die Per-CPU-Basis noch auf die des Programms
/// und muss zuerst zurueckgetauscht werden.
///
/// # Safety
/// Wie [`resume_kernel`], zusaetzlich: der Aufruf muss aus einem Pfad kommen,
/// der Ring 3 unterbrochen hat.
#[unsafe(naked)]
unsafe extern "C" fn resume_kernel_after_fault(saved_rsp: u64) -> ! {
    core::arch::naked_asm!(
        "swapgs",
        "mov rsp, rdi",
        "xor eax, eax",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "ret",
    )
}

// -------------------------------------------------------------- Systemaufruf

/// Rust-Seite eines Systemaufrufs.
///
/// # Safety
/// Wird ausschliesslich von [`syscall_entry`] mit einem gueltigen Zeiger auf
/// die sechs Argumentregister auf dem Kernelstapel aufgerufen.
unsafe extern "C" fn syscall_dispatch(nr: u64, args: *const [u64; 6]) -> u64 {
    // SAFETY: `args` zeigt auf sechs Worte auf dem eigenen Kernelstapel.
    let a = unsafe { *args };
    SYSCALLS.fetch_add(1, Ordering::Relaxed);
    match crate::kcore::user::syscall(nr, a) {
        SyscallOutcome::Weiter(v) => v,
        SyscallOutcome::Ende(code) => {
            EXIT_CODE.store(code, Ordering::Release);
            IN_USER.store(false, Ordering::Release);
            // SAFETY: Der Systemaufrufpfad hat die Per-CPU-Basis bereits
            // zurueckgetauscht; `SAVED_RSP` stammt aus `enter_ring3`.
            unsafe { resume_kernel(SAVED_RSP) }
        }
    }
}

/// Einsprung des schnellen Systemaufrufs (Ziel von `IA32_LSTAR`).
///
/// Beim Eintritt liegt die Ruecksprungadresse des Programms in RCX und dessen
/// RFLAGS in R11 — beide muessen bis zum `sysretq` unversehrt bleiben. Der
/// unprivilegierte Stapel wird im Per-CPU-Block abgelegt, gearbeitet wird auf
/// dem Kernelstapel. Nach den acht Pushes ist der Stapel 16-ausgerichtet, der
/// `call` stellt damit genau die vom SysV-ABI erwartete Lage her.
///
/// # Safety
/// Nur als Ziel von `IA32_LSTAR` eintragen, nie direkt aufrufen.
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() {
    core::arch::naked_asm!(
        "swapgs",
        "mov qword ptr gs:[8], rsp",
        "mov rsp, qword ptr gs:[0]",
        "push rcx",
        "push r11",
        "push r9",
        "push r8",
        "push r10",
        "push rdx",
        "push rsi",
        "push rdi",
        "mov rdi, rax",
        "mov rsi, rsp",
        "call {dispatch}",
        "add rsp, 48",
        "pop r11",
        "pop rcx",
        "mov rsp, qword ptr gs:[8]",
        "swapgs",
        "sysretq",
        dispatch = sym syscall_dispatch,
    )
}
