.text
.section ".text.boot", "ax"
.global boot
boot:

    ldr  x0, =0x30d00800 // Lttile Endian, MMU disabled, I/D-cache disabled
    msr  sctlr_el1, x0
    ldr  x0, =0x80000000 // EL1 is AArch64.
    msr  hcr_el2, x0
    ldr  x0, =0x000001c5 // Switch to EL1h, exceptions are masked.
    msr  spsr_el2, x0

    adr  x0, el1_mode
    msr  elr_el2, x0
    eret

el1_mode:

// "D8.3.1 VMSAv8-64 descriptor formats"
build_page_table:
    // Level-0 table: Only 1 entry, pointing to the L1 table.
    adrp x10, __page_table_l0
    add  x10, x10, :lo12:__page_table_l0
    adrp x11, __page_table_l1
    add  x11, x11, :lo12:__page_table_l1
    mov  x12, #0x00000403 // nG (not global), AttrIndex = 0, Table
    orr  x11, x11, x12
    str  x11, [x10], #8

    // Level-1 table: Only 1 entry, pointing to the L2 table.
    adrp x10, __page_table_l1
    add  x10, x10, :lo12:__page_table_l1
    adrp x11, __page_table_l2_0
    add  x11, x11, :lo12:__page_table_l2_0
    mov  x12, #0x00000b03 // nG (not global), AttrIndex = 0, Table
    orr  x11, x11, x12
    str  x11, [x10], #8

    adrp x11, __page_table_l2_1
    add  x11, x11, :lo12:__page_table_l2_1
    mov  x12, #0x00000b03 // nG (not global), AttrIndex = 0, Table
    orr  x11, x11, x12
    str  x11, [x10], #8

    // Level-2 table: 0x00000000-0x3fffffff (1GB)
    adrp x10, __page_table_l2_0
    add  x10, x10, :lo12:__page_table_l2_0
    mov x11, #0x00000000 // paddr base
    mov x12, #0x00000401 // nG (not global), AttrIndex = 0, Block
    orr x11, x11, x12
    mov x12, #512        // 512 entries
    mov x13, #0x200000   // 2MB block size
1:
    str x11, [x10], #8
    add x11, x11, x13
    subs x12, x12, #1
    bne 1b

    // Level-2 table: 0x40000000-0x7fffffff (1GB)
    adrp x10, __page_table_l2_1
    add  x10, x10, :lo12:__page_table_l2_1
    mov x11, #0x40000000 // paddr base
    mov x12, #0x00000401 // nG (not global), AttrIndex = 0, Block
    orr x11, x11, x12
    mov x12, #512        // 512 entries
    mov x13, #0x200000   // 2MB block size
2:
    str x11, [x10], #8
    add x11, x11, x13
    subs x12, x12, #1
    bne 2b

enable_mmu:
    // Configure paging.
    ldr  x10, =0x000000ff // AttrIndx == 0: Normal memory, cache enabled
                          // AttrIndx == 1: Device-nGnRnE memory
    msr  mair_el1, x10

    ldr  x10, =0x35103510 // 4KiB pages, the region size is 2^48.
    msr  tcr_el1, x10

    adrp x10, __page_table_l0
    add  x10, x10, :lo12:__page_table_l0
    msr  ttbr1_el1, x10
    msr  ttbr0_el1, x10

    ldr  x10, =0x30d00801 // Lttile Endian, MMU enabled, Don't disable I/D-cache
    dsb ish
    isb
    msr  sctlr_el1, x10
    isb

jump_to_rust:
    // Set the stack pointer.
    ldr  x10, =__boot_stack_top
    mov  sp, x10

    // Initialize the user library and run main().
    bl arm64_boot

    b .

.section ".bss"
.balign 4096
__page_table_l0:
    .space 4096

.balign 4096
__page_table_l1:
    .space 4096

.balign 4096
__page_table_l2_0:
    .space 4096

.balign 4096
__page_table_l2_1:
    .space 4096
