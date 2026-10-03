use core::arch::naked_asm;
use core::mem::offset_of;

use ftl_types::thread::Fault;

#[derive(Clone, Copy)]
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
    pub rax: usize,
    pub cookie: usize,
    pub reserved: usize,
    pub rsp: usize,
    pub rip: usize,
}

impl SyscallFrame {
    pub fn nr(&self) -> usize {
        self.rax
    }

    pub fn arg0(&self) -> usize {
        self.rdi
    }

    pub fn arg1(&self) -> usize {
        self.rsi
    }

    pub fn arg2(&self) -> usize {
        self.rdx
    }

    pub fn arg3(&self) -> usize {
        self.r10
    }

    pub fn arg4(&self) -> usize {
        self.r8
    }

    pub fn arg5(&self) -> usize {
        self.r9
    }

    pub fn retval(&self) -> isize {
        self.rax as isize
    }

    pub fn set_retval(&mut self, retval: isize) {
        self.rax = retval as usize;
    }

    pub fn set_sp(&mut self, sp: usize) {
        self.rsp = sp;
    }

    pub unsafe fn enter_signal(&mut self, signal: usize, handler: usize, restorer: usize) {
        // Use the beginning of the frame as the restorer, the return address
        // for RET instruction in signal handler.
        //
        // This means when entering the signal handler, R15 (unnecessarily)
        // points to the restorer, but it shouldn't matter anyway.
        let sp = (self as *mut Self).addr();

        // Make sure sp will be 16-bytes aligned, after PUSH RBP in the signal
        // handler. LX pushes odd number of registers, so this always holds,
        // ... I hope.
        debug_assert_eq!(sp % 16, 8);

        self.rsp = sp;
        self.r15 = restorer;

        self.rdi = signal; // the argument for the signal handler
        self.rip = handler;
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
        "mov rdi, rsp", // handle_syscall argument
        "and rsp, -16",
        "call {handle_syscall}",

        // Restore the user registers from the frame.
        "mov rsp, rax",
        "jmp {restore_regs}",
        handle_syscall = sym crate::syscall::handle_syscall,
        restore_regs = sym restore_regs,
    )
}

#[repr(C, packed)]
pub struct FaultFrame {
    pub r11: usize,
    pub r10: usize,
    pub r9: usize,
    pub r8: usize,
    pub rdi: usize,
    pub rsi: usize,
    pub rdx: usize,
    pub rcx: usize,
    pub rax: usize,

    // Fields from ftl_types::thread::FaultFrame:
    pub rflags: usize,
    pub rip: usize,
    pub cookie: usize,
    pub fault: Fault,
    pub addr: usize,
    pub info: usize,
}

const RED_ZONE_SIZE: usize = 128;

#[unsafe(naked)]
pub extern "C" fn fault_handler() -> ! {
    naked_asm!(
        // Save caller-saved registers. Callee-saved registers are preserved by
        // handle_fault.
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",

        // Align the stack to 16 bytes.
        "mov rdi, rsp", // handle_fault argument
        "and rsp, -16",
        "call {handle_fault}",

        // Restore the user registers from the frame.
        "mov rsp, rax",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "popfq", // user RFLAGS

        // Pop user RIP, and then skip the rest of the fault frame & red zone
        // to move RSP to the user RSP.
        "ret {skip_size}",
        handle_fault = sym crate::fault::handle_fault,
        skip_size = const (size_of::<FaultFrame>() - offset_of!(FaultFrame, cookie)) + RED_ZONE_SIZE,
    )
}

#[unsafe(naked)]
pub extern "C" fn restore_regs() -> ! {
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
        "pop rax",     // return value
        "add rsp, 16", // Skip cookie and reserved
        "pop rcx",     // user RSP
        "pop r11",     // user RIP
        "mov rsp, rcx",
        "jmp r11",
    )
}
