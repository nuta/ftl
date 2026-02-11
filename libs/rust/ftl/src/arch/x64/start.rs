use core::arch::asm;
use core::arch::naked_asm;

#[repr(C)]
struct Elf64Rela {
    offset: u64,
    info: u64,
    addend: i64,
}

fn apply_relocations(image_base: u64, relocs: *const Elf64Rela, relocs_end: *const Elf64Rela) {
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
}
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn start() -> ! {
    unsafe {
        naked_asm!(
            // Do not jump into rust_start directly. Rust (x86-64 ABI) expects
            // the stack to be 16-byte aligned "just before" calling a function,
            // before CALL pushes 8 bytes to the stack.
            "call {rust_start}",
            rust_start = sym rust_start,
        );
    }
}

extern "C" fn rust_start() -> ! {
    let image_base: u64;
    let relocs: *const Elf64Rela;
    let relocs_end: *const Elf64Rela;
    unsafe {
        asm!(
            "lea {0}, [rip + __image_base]",
            "lea {1}, [rip + __rela_dyn]",
            "lea {2}, [rip + __rela_dyn_end]",
            out(reg) image_base,
            out(reg) relocs,
            out(reg) relocs_end,
            options(nostack, nomem)
        );
    }

    apply_relocations(image_base, relocs, relocs_end);

    crate::log::init();
    crate::allocator::init();

    unsafe {
        asm!("call main", "ud2", options(noreturn));
    }
}
