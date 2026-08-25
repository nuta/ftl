use core::arch::naked_asm;

#[unsafe(naked)]
pub extern "C" fn syscall_handler() -> ! {
    naked_asm!(
        "pop r11", // cookie from syscall frame

        // Save caller-saved registers except for syscall-related ones
        // (rax, rcx, r11).
        "push rdi",
        "push rsi",
        "push rdx",
        "push r8",
        "push r9",
        "push r10",
        "push rbx",

        // Align the stack to 16 bytes.
        "mov rbx, rsp",
        "and rsp, -16",

        "mov rcx, r10", // arg3
        "push r11", // cookie (the second stack argument)
        "push rax", // syscall number (the last argument)
        "call {handle_syscall}",

        // Restore the stack pointer, and others.
        "mov rsp, rbx",
        "pop rbx",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r11", // user RFLAGS (from syscall frame)
        "pop rcx", // user RIP (from syscall frame)

        // Restore user RFLAGS.
        "push r11",
        "popfq",

        // Restore user RSP.
        "lea rsp, [rsp + 128]", // red zone

        // Go back to the application code.
        "jmp rcx",
        handle_syscall = sym crate::syscall::handle_syscall,
    )
}
