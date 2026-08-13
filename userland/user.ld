/* Linkerskript fuer unprivilegierte Programme.
 *
 * Zwei PT_LOAD-Segmente mit UNTERSCHIEDLICHEN Rechten — genau daran zeigt der
 * Lader, dass er p_flags auswertet:
 *   text: R+X, ab 0x401000 (im unprivilegierten Bereich, siehe
 *         kernel/src/kcore/user.rs: USER_BASE = 0x400000)
 *   data: R+W (nicht ausfuehrbar), mit .bss dahinter, also memsz > filesz.
 * W^X gilt auch hier: kein Segment ist gleichzeitig schreib- und ausfuehrbar.
 */
ENTRY(_start)

PHDRS
{
    text PT_LOAD FLAGS(5);   /* R + X */
    data PT_LOAD FLAGS(6);   /* R + W */
}

SECTIONS
{
    . = 0x401000;

    .text : { *(.text*) } :text

    . = ALIGN(0x1000);

    .rodata : { *(.rodata*) } :data
    .data   : { *(.data*) }   :data
    .bss    : { *(.bss*) *(COMMON) } :data

    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
