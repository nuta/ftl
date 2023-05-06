.section ".boot", "ax"

.global boot
boot:
    mv ra, zero
    mv fp, zero
    la sp, __boot_stack_top
    jal rust_entry
