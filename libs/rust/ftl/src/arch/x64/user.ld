ENTRY(start)

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

    .rela.dyn : ALIGN(4096) {
        __rela_dyn = .;
        *(.rela.dyn);
        __rela_dyn_end = .;
    }

    . = ALIGN(16);
    .bss : {
        *(.bss .bss.*);
    }
}
