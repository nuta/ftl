ENTRY(boot)

SECTIONS {
    . = 0x80200000;

    .text : {
        KEEP(*(.boot));
        *(.text .text.*);
    }

    __data = .;
    .rodata : ALIGN(4) {
        *(.rodata .rodata.*);
        . = ALIGN(4);
        *(.srodata .srodata.*);
    }

    .data : ALIGN(4) {
        *(.data .data.*);
        . = ALIGN(4);
        *(.sdata .sdata.*);
    }

    .bss : ALIGN(4) {
        *(.bss .bss.*);
        . = ALIGN(4);
        *(.sbss .sbss.*);

        __boot_stack_bottom = .;
        . = . + 1024 * 1024;
        __boot_stack_top = .;
    }
}
