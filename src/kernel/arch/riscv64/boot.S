.section ".boot", "ax"

.global boot
boot:
    mv ra, zero
    mv fp, zero
    la sp, __boot_stack_top
    jal boot_kernel
