use alloc::vec;
use alloc::vec::Vec;
use core::arch::asm;
use core::mem::offset_of;
use core::mem::size_of;

use ftl_utils::byte_size::ByteSize;

use super::cpuvar::CpuVar;
use crate::scheduler::GLOBAL_SCHEDULER;

const KERNEL_STACK_SIZE: ByteSize = ByteSize::from_kib(64);

/// The entry point of kernel threads.
#[no_mangle]
#[naked]
extern "C" fn kernel_entry() -> ! {
    unsafe {
        asm!(
            r#"
                mov rdi, r13    // The argument for the thread.
                jmp r12
            "#,
            options(noreturn)
        )
    }
}

fn switch_to_next() {
    GLOBAL_SCHEDULER.yield_cpu();
}

/// # Why `#[naked]`?
///
/// - To get the correct return address from ra. #[naked] prevents inlining.
/// - To eliminate the needless prologue.
#[naked]
pub extern "C" fn yield_cpu() {
    unsafe {
        asm!(
            r#"
                pushfq
                rdgsbase rax
                mov rax, [rax + {context_offset}]
                mov rsp, [rax + {rsp_offset}]
                mov rbp, [rax + {rbp_offset}]
                mov rbx, [rax + {rbx_offset}]
                mov r12, [rax + {r12_offset}]
                mov r13, [rax + {r13_offset}]
                mov r14, [rax + {r14_offset}]
                mov r15, [rax + {r15_offset}]
                jmp {switch_to_next}
            "#,
            context_offset = const offset_of!(CpuVar, context),
            rsp_offset = const offset_of!(Context, rsp),
            rbp_offset = const offset_of!(Context, rbp),
            rbx_offset = const offset_of!(Context, rbx),
            r12_offset = const offset_of!(Context, r12),
            r13_offset = const offset_of!(Context, r13),
            r14_offset = const offset_of!(Context, r14),
            r15_offset = const offset_of!(Context, r15),
            switch_to_next = sym switch_to_next,
            options(noreturn)
        )
    }
}

/// Resumes a thread.
fn resume(next: *mut Context) -> ! {
    unsafe {
        asm!(
            r#"
                // Update CpuVar.arch.context
                mov rax, {next_context}
                rdgsbase rdi
                mov  [rax + {context_offset}], rax

                mov rsp, [rax + {rsp_offset}]
                mov rbp, [rax + {rbp_offset}]
                mov rbx, [rax + {rbx_offset}]
                mov r12, [rax + {r12_offset}]
                mov r13, [rax + {r13_offset}]
                mov r14, [rax + {r14_offset}]
                mov r15, [rax + {r15_offset}]
                popfq
                ret
            "#,
            next_context = in (reg) next as usize,
            context_offset = const offset_of!(CpuVar, context),
            rsp_offset = const offset_of!(Context, rsp),
            rbp_offset = const offset_of!(Context, rbp),
            rbx_offset = const offset_of!(Context, rbx),
            r12_offset = const offset_of!(Context, r12),
            r13_offset = const offset_of!(Context, r13),
            r14_offset = const offset_of!(Context, r14),
            r15_offset = const offset_of!(Context, r15),
            options(noreturn)
        )
    }
}

/// Context of a thread.
///
/// Only callee-saved registers are stored because caller-saved registers are
/// already saved on the stack.
#[derive(Debug, Default)]
#[repr(C)]
pub struct Context {
    rsp: usize,
    rbp: usize,
    rbx: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
}

unsafe fn push_stack(rsp: usize, value: u64) -> usize {
    let new_rsp = rsp - 8;
    (new_rsp as *mut u64).write(value);
    new_rsp
}

// TODO: static assert to ensure usize == u64

pub struct Thread {
    pub(super) context: Context,
    /// The ownership of the stack area. To be freeed when the thread is destroyed.
    #[allow(dead_code)]
    stack: Option<Vec<u128>>,
}

impl Thread {
    pub fn new_idle() -> Thread {
        Thread {
            stack: None,
            context: Default::default(),
        }
    }

    pub fn new_kernel(pc: usize, arg: usize) -> Thread {
        let stack_size = 64 * 1024;

        // Use Vec<u128> to ensure 16-byte alignment as specified in the RISC-V calling convention:
        //
        // > In the standard RISC-V calling convention, the stack grows downward and the stack pointer is
        // > always kept 16-byte aligned.
        //
        // TODO: Avoid initializing the stack with zeros.
        let stack: Vec<u128> = vec![0; KERNEL_STACK_SIZE.in_bytes() / size_of::<u128>()];

        let mut rsp = (stack.as_ptr() as usize) + stack_size;
        unsafe {
            rsp = push_stack(rsp, kernel_entry as u64); // return address
            rsp = push_stack(rsp, 0x02); // RFLAGS (interrupts disabled).
        }

        Thread {
            stack: Some(stack),
            context: Context {
                rsp,
                rbp: 0,
                r12: pc,
                r13: arg,
                ..Default::default()
            },
        }
    }

    pub fn resume(&self) -> ! {
        resume(&self.context as *const _ as *mut _);
    }
}
