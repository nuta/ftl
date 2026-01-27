use core::arch::naked_asm;

#[repr(C)]
struct Elf64Rela {
    offset: u64,
    info: u64,
    addend: i64,
}

unsafe extern "C" {
    static __image_base: u64;
    static __rela_dyn: u8;
    static __rela_dyn_end: u8;
    fn main();
}

#[unsafe(no_mangle)]
extern "C" fn start() -> ! {
    let image_base = unsafe { &raw const __image_base as u64 };
    let relocs = unsafe { &raw const __rela_dyn as *const Elf64Rela };
    let relocs_end = unsafe { &raw const __rela_dyn_end as *const Elf64Rela };

    let mut rel = relocs;
    while rel < relocs_end {
        unsafe {
            let r_offset = (*rel).offset;
            let r_addend = (*rel).addend;
            let p = (image_base + r_offset) as *mut u64;
            *p = image_base + r_addend as u64;
            rel = rel.offset(1);
        }
    }

    unsafe { main() };
    loop {}
}
