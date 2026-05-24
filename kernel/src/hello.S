.intel_syntax noprefix

.section .text
.global _start
_start:
    syscall
    ud2
