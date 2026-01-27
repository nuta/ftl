use core::arch::naked_asm;

#[repr(C)]
struct Elf64Rela {
    offset: u64,
    info: u64,
    addend: i64,
}

const R_X86_64_RELATIVE: u32 = 8;

/// Applies PIE relocations. Called from asm with addresses passed as args
/// (not accessed via GOT).
#[inline(never)]
extern "C" fn apply_relocs(base: usize, rela: *const Elf64Rela, rela_end: *const Elf64Rela) {
    let mut rel = rela;
    while rel < rela_end {
        unsafe {
            let r_type = (*rel).info as u32;
            if r_type == R_X86_64_RELATIVE {
                let ptr = (base + (*rel).offset as usize) as *mut usize;
                *ptr = base + (*rel).addend as usize;
            }
            rel = rel.add(1);
        }
    }
}

/// PIE entry point. Minimal asm to get addresses via LEA, then call Rust.
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn start() -> ! {
    unsafe {
        naked_asm!(
            // Get addresses via RIP-relative LEA (no GOT)
            "lea rdi, [rip + __image_base]",
            "lea rsi, [rip + __rela_dyn]",
            "lea rdx, [rip + __rela_dyn_end]",
            // Apply relocations in Rust
            "call {apply_relocs}",
            // Now safe to call main (GOT is fixed)
            "call main",
            "2: hlt",
            "jmp 2b",
            apply_relocs = sym apply_relocs,
        )
    }
}
