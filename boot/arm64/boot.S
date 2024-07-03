.text
.global _start
_start:
    // Set the stack pointer.
    ldr  x0, =__boot_stack_top
    mov  sp, x0

    // Initialize the user library and run main().
    bl arm64_boot

.global halt
halt:
    b halt
