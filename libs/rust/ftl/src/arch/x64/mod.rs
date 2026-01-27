use core::arch::asm;

use ftl_types::environ::StartInfo;

#[repr(C)]
struct Elf64Rela {
    offset: u64,
    info: u64,
    addend: i64,
}

pub fn get_start_info() -> &'static StartInfo {
    unsafe {
        let start_info: *const StartInfo;
        asm!("rdgsbase {}", out(reg) start_info);
        &*(start_info as *const StartInfo)
    }
}

#[unsafe(no_mangle)]
extern "C" fn start() -> ! {
    let image_base: u64;
    let relocs: *const Elf64Rela;
    let relocs_end: *const Elf64Rela;
    let main_addr: u64;

    // Use RIP-relative LEA to get addresses without needing relocations
    unsafe {
        asm!(
            "lea {0}, [rip + __image_base]",
            "lea {1}, [rip + __rela_dyn]",
            "lea {2}, [rip + __rela_dyn_end]",
            "lea {3}, [rip + main]",
            out(reg) image_base,
            out(reg) relocs,
            out(reg) relocs_end,
            out(reg) main_addr,
            options(nostack, nomem)
        );
    }

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

    // Call main via function pointer (RIP-relative address already computed)
    unsafe {
        let main_fn: extern "C" fn() = core::mem::transmute(main_addr);
        main_fn();
    }
    loop {}
}
