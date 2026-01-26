ENTRY(x64_boot)

KERNEL_BASE = 0xffff800000000000;

SECTIONS {
    . = 0x100000 + KERNEL_BASE;

    . = ALIGN(16);
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
}
