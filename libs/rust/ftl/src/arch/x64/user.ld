ENTRY(start)

SECTIONS {
    __image_base = .;

    . = ALIGN(16);
    .text : {
        // Bootstrap's flat binary (objcopy -Obinary) assumes that the entry
        // point is at the beginning of the executable.
        *(.text.start);
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
