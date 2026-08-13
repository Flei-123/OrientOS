/* karst — Linkerskript fuer x86_64.
 *
 * Higher-half Kernel bei -2 GiB. Diese Adresse ist kein Zufall: das
 * "kernel" code model von LLVM erzeugt nur dann RIP-relative Zugriffe mit
 * 32-Bit-Verschiebung, wenn der Code in den obersten 2 GiB liegt.
 *
 * Die __*_start/__*_end-Symbole braucht mm/, um das Kernelabbild in den
 * SELBST gebauten Adressraum mit korrekten Rechten zu uebernehmen:
 * Text ausfuehrbar, rodata nur lesbar, data/bss beschreibbar aber NX.
 */
OUTPUT_FORMAT(elf64-x86-64)
ENTRY(kmain)

PHDRS
{
    text    PT_LOAD;
    rodata  PT_LOAD;
    data    PT_LOAD;
}

SECTIONS
{
    . = 0xffffffff80000000;

    __kernel_start = .;

    __text_start = .;
    .text : {
        *(.text .text.*)
    } :text
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __text_end = .;

    __rodata_start = .;
    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    /* Die Limine-Anfragen muessen als zusammenhaengender, markierter Block
       im Abbild liegen, damit der Bootloader sie findet. */
    .limine_requests : {
        KEEP(*(.limine_requests_start))
        KEEP(*(.limine_requests))
        KEEP(*(.limine_requests_end))
    } :rodata
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __rodata_end = .;

    __data_start = .;
    .data : {
        *(.data .data.*)
        /* Ohne diese Zeilen legt der Linker .got als "Orphan" HINTER __data_end
           ab — der Kernel bildet sie dann nicht ab und faellt beim ersten
           Zugriff auf eine globale Adresse in einen Page Fault. */
        *(.got .got.plt)
        *(.data.rel.ro .data.rel.ro.*)
        *(.dynamic)
    } :data

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } :data
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __data_end = .;

    __kernel_end = .;

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}
