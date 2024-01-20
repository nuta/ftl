#![no_std]
#![no_main]

use core::arch::global_asm;

global_asm!(include_str!("boot.S"));

extern "C" {
    static __bss: u8;
    static __bss_end: u8;
}

#[no_mangle]
unsafe extern "C" fn riscv64_boot(_hartid: u64, _dtb_addr: u64) -> ! {
    // Clear bss section.
    let bss_start = &__bss as *const _ as usize;
    let bss_end = &__bss_end as *const _ as usize;
    core::ptr::write_bytes(bss_start as *mut u8, 0, bss_end - bss_start);

    ftl::boot::boot();
}
