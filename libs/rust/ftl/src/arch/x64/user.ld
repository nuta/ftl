ENTRY(main)

SECTIONS {
    . = ALIGN(16);
    .text : {
        *(.text .text.*);
    }

    . = ALIGN(16);
    .rodata : {
        *(.rodata .rodata.*);
    }

    . = ALIGN(16);
    .data : {
        *(.data .data.*);
    }

    . = ALIGN(16);
    .bss : {
        *(.bss .bss.*);
    }
}
