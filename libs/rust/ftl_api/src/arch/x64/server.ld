ENTRY(init)

/*
 * Define the program headers explicitly to drop unused ones such as
 * PT_GNU_STACK.
 */
PHDRS {
    text PT_LOAD;
    rodata PT_LOAD;
    data PT_LOAD;
    bss PT_LOAD;
}

SECTIONS {
    .text : {
        *(.text .text.*)
    } : text

    .rodata : {
        *(.rodata .rodata.*)
    } : rodata

    .data : {
        *(.data .data.*)
    } : data

    .bss : {
        *(.bss .bss.*)
    } : bss

    ASSERT(DEFINED(init), "server entry symbol init must be defined")
    ASSERT(SIZEOF(.rel.dyn) == 0, "server image must not contain REL relocations")
    ASSERT(SIZEOF(.rela.dyn) == 0, "server image must not contain RELA relocations")
}
