.intel_syntax noprefix

.section .rodata
msg:
    .ascii "Hello, World!\n"
.equ msg_len, 14

.section .text
.global _start
_start:
    // write(1, msg, msg_len)
    mov rax, 1
    mov rdi, 1
    lea rsi, [rip + msg]
    mov edx, msg_len
    syscall

    // exit(0)
    mov rax, 60
    xor rdi, rdi
    syscall

1:
    jmp 1b
