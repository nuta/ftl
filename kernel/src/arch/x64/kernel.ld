ENTRY(x64_boot)

KERNEL_BASE = 0xffff800000000000;

SECTIONS {
    . = 0x100000 + KERNEL_BASE;
    __kernel_memory = .;

    . = ALIGN(16);
    .multiboot2 : AT(ADDR(.multiboot2) - KERNEL_BASE) {
        KEEP(*(.multiboot2));
    }

    .note.pvh : AT(ADDR(.note.pvh) - KERNEL_BASE) {
        KEEP(*(.note.pvh));
    }

    .text : AT(ADDR(.text) - KERNEL_BASE) {
        *(.text .text.*);
    }

    . = ALIGN(16);
    .rodata : AT(ADDR(.rodata) - KERNEL_BASE) {
        *(.rodata .rodata.*);
    }

    . = ALIGN(16);
    .data : AT(ADDR(.data) - KERNEL_BASE) {
        *(.data .data.*);
    }

    . = ALIGN(16);
    .bss : AT(ADDR(.bss) - KERNEL_BASE) {
        *(.bss .bss.*);
    }

    __kernel_memory_end = .;
}
