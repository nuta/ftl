// The interrupt/exception/system call handler entry point. `stvec` is set to
// this address.
//
// This function address must be aligned to 4 bytes not to accidentally set
// MODE field in stvec.
.balign 4
.global switch_to_kernel
switch_to_kernel:
    call interrupt_handler
    j .
