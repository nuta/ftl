use core::arch::naked_asm;

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn start() -> ! {
    unsafe {
        naked_asm!(
            "mov rax, 0x000000000000000000000000000",
            "hlt",
            "call main",
            "ud2"
        )
    }
}
