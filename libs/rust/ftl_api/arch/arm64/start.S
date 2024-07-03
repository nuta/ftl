.text
.global _start
_start:
    // Set the stack pointer.
    ldr  x0, =__stack_top
    mov  sp, x0

    // Initialize the user library and run main().
    bl arm64_boot

.global halt
halt:
    b halt

.section .bss  // .bss, not .data to avoid having zeroes in the ELF file
.balign 16
__stack_bottom:
    .space 256 * 1024
__stack_top:
