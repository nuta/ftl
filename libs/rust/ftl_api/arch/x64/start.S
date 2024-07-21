// rdi: *const VsyscallPage
.text
.global start
start:
    // Set the stack pointer.
    lea rsp, [rip + __stack_top]

    // Initialize the user library and run main().
    call ftl_app_main

    // TODO: Call process_exit system call instead
1:
    jmp 1b

.section .bss  // .bss, not .data to avoid having zeroes in the ELF file
.balign 16
__stack_bottom:
    .space 256 * 1024
__stack_top:
