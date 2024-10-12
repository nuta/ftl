#![allow(unused)]
use core::arch::asm;
use core::arch::global_asm;
use core::mem::offset_of;

use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;
use ftl_types::interrupt::Irq;
use thread::Context;

use crate::cpuvar::CpuId;
use crate::interrupt::Interrupt;
use crate::refcount::SharedRef;

mod cpuvar;
mod gdt;
mod idle;
mod idt;
mod init;
mod serial;
mod switch;
mod thread;
mod tss;
mod vmspace;

pub use cpuvar::get_cpuvar;
pub use cpuvar::set_cpuvar;
pub use cpuvar::CpuVar;
pub use idle::idle;
pub use init::early_init;
pub use init::init;
pub use switch::return_to_user;
pub use thread::Thread;
pub use vmspace::VmSpace;
pub use vmspace::USERSPACE_END;
pub use vmspace::USERSPACE_START;

const KERNEL_BASE: usize = 0xffff_8000_0000_0000;

pub fn halt() -> ! {
    warn!("entering halt");
    loop {}
}

pub fn paddr2vaddr(paddr: PAddr) -> Result<VAddr, FtlError> {
    Ok(VAddr::new(paddr.as_usize() + KERNEL_BASE))
}

pub fn vaddr2paddr(vaddr: VAddr) -> Result<PAddr, FtlError> {
    Ok(PAddr::new(vaddr.as_usize() - KERNEL_BASE))
}

pub fn console_write(bytes: &[u8]) {
    for ch in bytes {
        serial::SERIAL0.print_char(*ch);
    }
}

pub fn backtrace<F>(mut callback: F)
where
    F: FnMut(usize),
{
    println!("backtrace not implemented")
}

pub unsafe extern "C" fn kernel_syscall_entry(
    _a0: isize,
    _a1: isize,
    _a2: isize,
    _a3: isize,
    _a4: isize,
    _a5: isize,
) -> isize {
    asm!(
        r#"
            mov rax, gs:[{context_offset}]

            // Save general-purpose registers.
            mov [rax + {rbx_offset}], rbx
            mov [rax + {rcx_offset}], rcx
            mov [rax + {rdx_offset}], rdx
            mov [rax + {rsi_offset}], rsi
            mov [rax + {rdi_offset}], rdi
            mov [rax + {rbp_offset}], rbp
            mov [rax + {rsp_offset}], rsp
            mov [rax + {r8_offset}],  r8
            mov [rax + {r9_offset}],  r9
            mov [rax + {r10_offset}], r10
            mov [rax + {r11_offset}], r11
            mov [rax + {r12_offset}], r12
            mov [rax + {r13_offset}], r13
            mov [rax + {r14_offset}], r14
            mov [rax + {r15_offset}], r15

            // Get the return address.
            mov rax, [rsp]
            mov gs:[{rip_offset}], rax

            // Handle the system call.
            call {syscall_handler}

            // Save the return value in the thread context, and switch
            // to the next thread.
            mov rdi, gs:[{context_offset}]
            mov [rdi + {rax_offset}], rax
            jmp {switch_to_next}
        "#,
        context_offset = const offset_of!(crate::arch::CpuVar, context),
        rip_offset = const offset_of!(Context, rip),
        rax_offset = const offset_of!(Context, rax),
        rbx_offset = const offset_of!(Context, rbx),
        rcx_offset = const offset_of!(Context, rcx),
        rdx_offset = const offset_of!(Context, rdx),
        rsi_offset = const offset_of!(Context, rsi),
        rdi_offset = const offset_of!(Context, rdi),
        rbp_offset = const offset_of!(Context, rbp),
        rsp_offset = const offset_of!(Context, rsp),
        r8_offset = const offset_of!(Context, r8),
        r9_offset = const offset_of!(Context, r9),
        r10_offset = const offset_of!(Context, r10),
        r11_offset = const offset_of!(Context, r11),
        r12_offset = const offset_of!(Context, r12),
        r13_offset = const offset_of!(Context, r13),
        r14_offset = const offset_of!(Context, r14),
        r15_offset = const offset_of!(Context, r15),
        syscall_handler = sym crate::syscall::syscall_handler,
        switch_to_next = sym crate::thread::switch_thread,
        options(noreturn)
    )
}

pub fn interrupt_create(interrupt: &SharedRef<Interrupt>) -> Result<(), FtlError> {
    todo!()
}

pub fn interrupt_ack(irq: Irq) -> Result<(), FtlError> {
    todo!()
}

pub const PAGE_SIZE: usize = 4096;
pub const NUM_CPUS_MAX: usize = 8;
