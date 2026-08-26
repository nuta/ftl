use core::arch::naked_asm;

#[repr(C)]
pub struct SyscallFrame {
    pub r15: usize,
    pub r14: usize,
    pub r13: usize,
    pub r12: usize,
    pub rbp: usize,
    pub rbx: usize,
    pub r10: usize,
    pub r9: usize,
    pub r8: usize,
    pub rdx: usize,
    pub rsi: usize,
    pub rdi: usize,
    /// The system call number.
    pub nr: usize,
    pub cookie: usize,
    pub rflags: usize,
    pub rip: usize,
}

impl SyscallFrame {
    pub fn arg0(&self) -> usize {
        self.rdi
    }

    pub fn arg1(&self) -> usize {
        self.rsi
    }

    pub fn arg2(&self) -> usize {
        self.rdx
    }
}

#[unsafe(naked)]
pub extern "C" fn syscall_handler() -> ! {
    naked_asm!(
        "push rax", // nr (system call number)

        // Complete the register frame already started by the kernel.
        "push rdi",
        "push rsi",
        "push rdx",
        "push r8",
        "push r9",
        "push r10",
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Align the stack to 16 bytes.
        "mov rbx, rsp",
        "mov rdi, rsp", // handle_syscall argument
        "and rsp, -16",
        "call {handle_syscall}",

        // Restore the stack pointer, and others.
        "mov rsp, rbx",
        "jmp {restore_regs}",
        handle_syscall = sym crate::syscall::handle_syscall,
        restore_regs = sym restore_regs,
    )
}

#[unsafe(naked)]
extern "C" fn restore_regs() -> ! {
    naked_asm!(
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "add rsp, 16", // Skip system call number and cookie
        "pop r11",     // user RFLAGS
        "pop rcx",     // user RIP
        "push r11",
        "popfq",
        "add rsp, 128", // red zone
        "jmp rcx",
    )
}

/// The entry point for the child process.
///
/// RSP points to the user stack when entering `handle_syscall`, and it
/// contains copied `SyscallFrame`.
#[unsafe(naked)]
pub extern "C" fn fork_child_entry() -> ! {
    naked_asm!(
        "xor eax, eax", // the return value of fork(2)
        "jmp {restore_regs}",
        restore_regs = sym restore_regs,
    )
}
