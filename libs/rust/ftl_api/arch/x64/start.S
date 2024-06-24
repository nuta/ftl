// rdi: vsyscall_page
.section .text
.align 4
.global start
start:
    xor rbp, rbp
    lea rsp, [__stack_top]
    call start_ftl_api
    int3

do_send:
    ud2

.section .bss  // .bss, not .data to avoid having zeroes in the ELF file
.align 16       // 16-bytes as required by System-V ABI
__stack_bottom:
    .space 256 * 1024
__stack_top:
