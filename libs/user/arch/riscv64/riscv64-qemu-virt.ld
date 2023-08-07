ENTRY(start)

SECTIONS {
    . = 0x10000000;

    .text : {
        *(.text .text.*);
    }

    .rodata : ALIGN(4096) {
        *(.rodata .rodata.*);
        . = ALIGN(8);
        *(.srodata .srodata.*);
    }

    .data : ALIGN(4096) {
        *(.data .data.*);
        . = ALIGN(8);
        *(.sdata .sdata.*);
    }

    .bss : ALIGN(4096) {
        *(.bss .bss.*);
        . = ALIGN(8);
        *(.sbss .sbss.*);

        __stack = .;
        . = . + 2 * 1024 * 1024;
        __stack_top = .;
    }
}
