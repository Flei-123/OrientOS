; hello — erstes unprivilegiertes Programm.
;
; Statisch gebunden, ohne Laufzeitbibliothek, ohne Relokationen: genau das,
; was der ELF64-Lader des Kernels annimmt (ET_EXEC, zwei PT_LOAD-Segmente,
; .bss mit memsz > filesz).
;
; Aufrufkonvention der nativen ABI (libs/karst-abi-native/src/syscall.rs):
;   Nummer  in rax
;   Args    in rdi, rsi, rdx, r10, r8, r9
;   Ergebnis in rax (>= 0 Erfolg, < 0 Fehlerwert — KEIN errno)
;
; Beim Eintritt uebergibt der Kernel in rdi das Handle, ueber das das Programm
; schreiben darf. Es gibt keinen globalen Namensraum und keine festen
; Deskriptoren 0/1/2 — ohne dieses Handle kann das Programm nichts ausgeben.

BITS 64

%define SYS_VERSION       0
%define SYS_HANDLE_WRITE  2
%define SYS_PROCESS_EXIT 18

section .text
global _start
_start:
    mov     r12, rdi                ; uebergebenes Ausgabe-Handle merken

    ; 1) Lebenszeichen: ABI-Version erfragen.
    mov     rax, SYS_VERSION
    syscall
    mov     [abi_version], rax

    ; 2) Etwas ausgeben — ueber das uebergebene Handle, nicht ueber einen
    ;    vorgefundenen Deskriptor.
    mov     rax, SYS_HANDLE_WRITE
    mov     rdi, r12
    lea     rsi, [message]
    mov     rdx, message_len
    syscall

    ; 3) Beweis, dass das beschreibbare Segment wirklich beschreibbar ist:
    ;    in das genullte .bss schreiben und wieder lesen.
    mov     qword [scratch], 0x2b2b2b2b
    mov     rax, [scratch]

    ; 4) Sauber beenden. Der Kernel raeumt danach ab.
    mov     rax, SYS_PROCESS_EXIT
    xor     rdi, rdi
    syscall

    ; Falls der Kernel wider Erwarten zurueckkehrt: nicht weiterlaufen.
.halt:
    jmp     .halt

section .rodata
message:
    db  "hallo aus der unprivilegierten Ebene", 10
message_len equ $ - message

section .bss
    align 8
abi_version:
    resq 1
scratch:
    resq 1
