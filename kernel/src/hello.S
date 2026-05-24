.intel_syntax noprefix

.section .text
.global _start
_start:
    syscall
    syscall
    syscall
1:
    jmp 1b
