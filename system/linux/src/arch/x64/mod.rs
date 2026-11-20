use core::arch::naked_asm;
use core::mem::offset_of;

use ftl_types::thread::Fault;
use ftl_utils::static_assert;

mod context;

pub use context::UserContext;

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

    pub fn set_retval(&mut self, retval: isize) {
        self.rax = retval as usize;
    }

    pub fn sp(&self) -> usize {
        self.rsp
    }

    pub fn set_sp(&mut self, sp: usize) {
        self.rsp = sp;
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

#[repr(C)]
pub struct FaultFrame {
    // Fields from ftl_types::thread::FaultFrame:
    pub rdi: usize,
    pub rip: usize,
    pub cs: usize,
    pub rflags: usize,
    pub rsp: usize,
    pub ss: usize,
    pub fault: Fault,
    pub addr: usize,
    pub info: usize,
    // Registers saved in the system's fault handler.
    pub rax: usize,
    pub rcx: usize,
    pub rdx: usize,
    pub rsi: usize,
    pub r8: usize,
    pub r9: usize,
    pub r10: usize,
    pub r11: usize,
    pub rbx: usize,
    pub rbp: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
}

static_assert!(offset_of!(FaultFrame, rdi) == 0);
static_assert!(offset_of!(FaultFrame, rax) == size_of::<ftl_types::thread::FaultFrame>());

pub const FAULT_REGS_SIZE: usize = size_of::<FaultFrame>() - offset_of!(FaultFrame, rax);

#[unsafe(naked)]
pub extern "C" fn fault_handler() -> ! {
    // RDI = The fault frame (also the top of the system stack).
    // RSP = The user (or system) RSP when the fault occurred.
    naked_asm!(
        // Save all registers for signal delivery.
        "mov [rdi + {rax}], rax",
        "mov [rdi + {rcx}], rcx",
        "mov [rdi + {rdx}], rdx",
        "mov [rdi + {rsi}], rsi",
        "mov [rdi + {r8}], r8",
        "mov [rdi + {r9}], r9",
        "mov [rdi + {r10}], r10",
        "mov [rdi + {r11}], r11",
        "mov [rdi + {rbx}], rbx",
        "mov [rdi + {rbp}], rbp",
        "mov [rdi + {r12}], r12",
        "mov [rdi + {r13}], r13",
        "mov [rdi + {r14}], r14",
        "mov [rdi + {r15}], r15",

        // Check if the fault happened while using the system stack (i.e. in the system's
        // system call handler).
        "mov rax, rdi",
        "sub rax, rsp", // system sp - faulted rsp
        "cmp rax, {system_stack_size}",

        // If RAX >= system_stack_size, RSP was not in the system stack. Then,
        // set the RDI (top of the system stack) to RSP.
        "cmovae rsp, rdi",

        "and rsp, -16", // align down to 16 bytes
        "call {handle_fault}",

        // Restore the user registers from the frame.
        "mov rsp, rax",
        "mov r11, [rsp + {r11}]",
        "mov r10, [rsp + {r10}]",
        "mov r9, [rsp + {r9}]",
        "mov r8, [rsp + {r8}]",
        "mov rsi, [rsp + {rsi}]",
        "mov rdx, [rsp + {rdx}]",
        "mov rcx, [rsp + {rcx}]",
        "mov rax, [rsp + {rax}]",
        "pop rdi",

        // Restore user RIP, RFLAGS, and RSP at once.
        "iretq",
        handle_fault = sym crate::fault::handle_fault,
        system_stack_size = const crate::thread::SYSTEM_STACK_SIZE,
        rax = const offset_of!(FaultFrame, rax),
        rcx = const offset_of!(FaultFrame, rcx),
        rdx = const offset_of!(FaultFrame, rdx),
        rsi = const offset_of!(FaultFrame, rsi),
        r8 = const offset_of!(FaultFrame, r8),
        r9 = const offset_of!(FaultFrame, r9),
        r10 = const offset_of!(FaultFrame, r10),
        r11 = const offset_of!(FaultFrame, r11),
        rbx = const offset_of!(FaultFrame, rbx),
        rbp = const offset_of!(FaultFrame, rbp),
        r12 = const offset_of!(FaultFrame, r12),
        r13 = const offset_of!(FaultFrame, r13),
        r14 = const offset_of!(FaultFrame, r14),
        r15 = const offset_of!(FaultFrame, r15),
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
        "pop rax", // return value
        "pop rcx", // user RSP
        "pop r11", // user RIP
        "mov rsp, rcx",
        "jmp r11",
    )
}
