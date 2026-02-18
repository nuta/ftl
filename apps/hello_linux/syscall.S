.global _start
.intel_syntax noprefix

.section .text
_start:
    mov     rax, 1          // SYS_write
    mov     rdi, 1          // stdout
    lea     rsi, [rip+msg]
    mov     rdx, msg_len
    syscall

    mov     rax, 60         // SYS_exit
    xor     rdi, rdi
    syscall

.section .rodata
msg:    .ascii  "Hello from Linux!\n"
.set msg_len, . - msg
