ENTRY(server_start)

/*
 * Define the program headers explicitly to drop unused ones such as
 * PT_GNU_STACK.
 */
PHDRS {
    text PT_LOAD;
    rodata PT_LOAD;
    data PT_LOAD;
    bss PT_LOAD;
    dynamic PT_DYNAMIC;
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

    .dynamic : {
        *(.dynamic)
    } : bss : dynamic
}
