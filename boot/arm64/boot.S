.text
.section ".text.boot", "ax"

// https://www.kernel.org/doc/Documentation/arm64/booting.txt
linux_image_header:
    nop              // code0
    b boot           // code1
    .quad 0          // text_offset
    .quad 0          // image_size
    .quad 0          // flags
    .quad 0          // res2
    .quad 0          // res3
    .quad 0          // res4
    .ascii "ARM\x64" // magic
    .long 0          // res5 (PE/COFF offset)

.global boot
boot:
    // Set the stack pointer.
    adrp   x30, __boot_stack_top
    add    x30, x30, :lo12:__boot_stack_top
    mov sp, x30
    bl arm64_boot

    b .
