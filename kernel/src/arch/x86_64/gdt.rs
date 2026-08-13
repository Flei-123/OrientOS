//! GDT und TSS.
//!
//! Im long mode sind Segmente weitgehend entkernt — die GDT wird aber weiterhin
//! fuer den Privilegwechsel und vor allem fuer das **TSS** gebraucht, das die
//! IST-Stapel traegt. Genau darauf laeuft der Double-Fault-Handler: ein eigener,
//! garantiert gueltiger Stack, damit ein kaputter Kernelstack nicht zum Triple
//! Fault fuehrt.

use core::arch::asm;
use core::mem::size_of;
use core::ptr::{addr_of, addr_of_mut};

/// Selektor des Kernel-Codesegments.
pub const KERNEL_CODE: u16 = 0x08;
/// Selektor des Kernel-Datensegments.
pub const KERNEL_DATA: u16 = 0x10;
/// Selektor des User-Datensegments (fuer `sysret`-Reihenfolge vorgesehen).
pub const USER_DATA: u16 = 0x18;
/// Selektor des User-Codesegments.
pub const USER_CODE: u16 = 0x20;
/// Selektor des TSS.
pub const TSS_SEL: u16 = 0x28;

/// IST-Index des Double-Fault-Stapels (1-basiert, wie in der IDT kodiert).
pub const IST_DOUBLE_FAULT: u8 = 1;
/// IST-Index des Page-Fault-Stapels.
pub const IST_PAGE_FAULT: u8 = 2;
/// IST-Index des NMI-Stapels.
pub const IST_NMI: u8 = 3;

const IST_STACK_SIZE: usize = 4096 * 5;

#[repr(C, align(16))]
struct IstStack([u8; IST_STACK_SIZE]);

static mut DF_STACK: IstStack = IstStack([0; IST_STACK_SIZE]);
static mut PF_STACK: IstStack = IstStack([0; IST_STACK_SIZE]);
static mut NMI_STACK: IstStack = IstStack([0; IST_STACK_SIZE]);

/// Task State Segment im 64-Bit-Format (104 Byte).
#[repr(C, packed(4))]
pub struct Tss {
    reserved0: u32,
    /// Stapelzeiger fuer Ring 0..2.
    pub rsp: [u64; 3],
    reserved1: u64,
    /// Interrupt Stack Table.
    pub ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iomap_base: u16,
}

static mut TSS: Tss = Tss {
    reserved0: 0,
    rsp: [0; 3],
    reserved1: 0,
    ist: [0; 7],
    reserved2: 0,
    reserved3: 0,
    iomap_base: size_of::<Tss>() as u16,
};

/// GDT: null, kcode, kdata, udata, ucode, tss(2 Slots).
static mut GDT: [u64; 7] = [0; 7];

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

/// Flat-Descriptor-Konstanten (Basis 0, Limit 4 GiB; im long mode ignoriert,
/// aber die Typ- und L-Bits zaehlen).
const K_CODE: u64 = 0x00AF_9A00_0000_FFFF;
const K_DATA: u64 = 0x00CF_9200_0000_FFFF;
const U_DATA: u64 = 0x00CF_F200_0000_FFFF;
const U_CODE: u64 = 0x00AF_FA00_0000_FFFF;

/// Baut GDT und TSS auf, laedt beide und schaltet die Segmentregister um.
///
/// # Safety
/// Genau einmal im Boot, mit gesperrten Interrupts.
pub unsafe fn init() {
    unsafe {
        // IST-Stapel eintragen (Stack waechst nach unten -> oberes Ende).
        let df = addr_of!(DF_STACK) as u64 + IST_STACK_SIZE as u64;
        let pf = addr_of!(PF_STACK) as u64 + IST_STACK_SIZE as u64;
        let nmi = addr_of!(NMI_STACK) as u64 + IST_STACK_SIZE as u64;
        let tss = addr_of_mut!(TSS);
        (*tss).ist[(IST_DOUBLE_FAULT - 1) as usize] = df & !0xF;
        (*tss).ist[(IST_PAGE_FAULT - 1) as usize] = pf & !0xF;
        (*tss).ist[(IST_NMI - 1) as usize] = nmi & !0xF;
        (*tss).iomap_base = size_of::<Tss>() as u16;

        let gdt = addr_of_mut!(GDT);
        (*gdt)[0] = 0;
        (*gdt)[1] = K_CODE;
        (*gdt)[2] = K_DATA;
        (*gdt)[3] = U_DATA;
        (*gdt)[4] = U_CODE;

        // 16-Byte-System-Deskriptor fuer das TSS.
        let base = tss as u64;
        let limit = (size_of::<Tss>() - 1) as u64;
        (*gdt)[5] = (limit & 0xFFFF)
            | ((base & 0x00FF_FFFF) << 16)
            | (0x89u64 << 40) // present, type = 64-bit TSS available
            | (((limit >> 16) & 0xF) << 48)
            | (((base >> 24) & 0xFF) << 56);
        (*gdt)[6] = base >> 32;

        let dtr = DescriptorTablePointer {
            limit: (size_of::<[u64; 7]>() - 1) as u16,
            base: gdt as u64,
        };
        asm!("lgdt [{}]", in(reg) &dtr, options(readonly, nostack, preserves_flags));

        // CS laesst sich nicht mit `mov` laden — Fernruecksprung auf ein Label.
        asm!(
            "push {sel}",
            "lea {tmp}, [rip + 55f]",
            "push {tmp}",
            "retfq",
            "55:",
            sel = in(reg) KERNEL_CODE as u64,
            tmp = lateout(reg) _,
            options(preserves_flags),
        );

        // Datensegmente auf das flache Kernel-Datensegment.
        asm!(
            "mov ds, {0:x}",
            "mov es, {0:x}",
            "mov ss, {0:x}",
            "mov fs, {0:x}",
            "mov gs, {0:x}",
            in(reg) KERNEL_DATA,
            options(nostack, preserves_flags),
        );

        // TSS laden.
        asm!("ltr {0:x}", in(reg) TSS_SEL, options(nostack, preserves_flags));
    }
}

/// Oberes Ende des Double-Fault-Stapels — nur fuer Diagnoseausgaben.
pub fn double_fault_stack_top() -> u64 {
    addr_of!(DF_STACK) as u64 + IST_STACK_SIZE as u64
}
