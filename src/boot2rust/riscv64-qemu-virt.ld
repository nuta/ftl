ENTRY(boot)

SECTIONS {
    . = 0x80200000;

    .text : {
        KEEP(*(.boot));
        *(.text .text.*);
    }

    __data = .;
    .rodata : ALIGN(8) {
        *(.rodata .rodata.*);
        . = ALIGN(8);
        *(.srodata .srodata.*);
    }

    .data : ALIGN(8) {
        *(.data .data.*);
        . = ALIGN(8);
        *(.sdata .sdata.*);
    }

    .bss : ALIGN(8) {
        *(.bss .bss.*);
        . = ALIGN(8);
        *(.sbss .sbss.*);

        __boot_stack_bottom = .;
        . = . + 1024 * 1024;
        __boot_stack_top = .;
    }
}
