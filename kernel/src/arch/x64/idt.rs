use core::arch::asm;
use core::arch::global_asm;
use core::arch::naked_asm;
use core::mem::offset_of;

use ftl_utils::spinlock::SpinLock;

use super::gdt::GDT_KERNEL_CS;
use super::io_apic::IRQ_VECTOR_BASE;
use super::syscall::syscall_copy_recover;
use super::thread::Thread;
use super::thread::XSTATE_MASK;
use super::timer::TIMER_IRQ;
use crate::address::VAddr;
use crate::cpuvar::CpuVar;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    offset0: u16,
    selector: u16,
    ist: u8,
    gate_type: u8,
    offset1: u16,
    offset2: u32,
    reserved: u32,
}

#[repr(C, packed)]
pub struct Idtr {
    limit: u16,
    base: u64,
}

unsafe extern "C" {
    static idt_handlers: u8;
    static usercopy0: u8;
    static usercopy0_recover: u8;
    static usercopy1: u8;
    static usercopy1_recover: u8;
    static syscall_copy0: u8;
    static syscall_copy1: u8;
    static syscall_copy2: u8;
}

const NUM_IDT_ENTRIES: usize = 256;
const INTERRUPT_HANDLER_SIZE: u64 = 16;

const IDT_ENTRY_DEFAULT: IdtEntry = IdtEntry {
    offset0: 0,
    selector: 0,
    ist: 0,
    gate_type: 0,
    offset1: 0,
    offset2: 0,
    reserved: 0,
};

static IDT: SpinLock<[IdtEntry; NUM_IDT_ENTRIES]> =
    SpinLock::new([IDT_ENTRY_DEFAULT; NUM_IDT_ENTRIES]);

const EXCEPTION_PAGE_FAULT: u8 = 14;

#[repr(C)]
struct InterruptFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    vector: u64,
    error_code: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

// Define interrupt handlers.
global_asm!(
    r#"
.set INTERRUPT_HANDLER_SIZE, 16
.align INTERRUPT_HANDLER_SIZE
.global idt_handlers
idt_handlers:
.set i, 0
.rept 256
.if i == 8 || 10 <= i && i <= 14 || i == 17
    cli
    push i
    jmp interrupt_entry
    .align INTERRUPT_HANDLER_SIZE
.else
    cli
    push 0 // error code
    push i
    jmp interrupt_entry
    .align INTERRUPT_HANDLER_SIZE
.endif

.set i, i + 1
.endr
"#
);

/// The entry point of interrupt handlers.
///
/// # Stack frame
///
/// - vector
/// - error code (0 if it's not an exception w/ error)
/// - RIP
/// - CS
/// - RFLAGS
/// - RSP
/// - SS
///
/// Since we don't support 32-bit mode, SS and RSP are always there as per the
/// SDM:
///
/// > 64-bit mode also pushes SS:RSP unconditionally, rather than only on a CPL
/// > change.
/// >
/// > 7.14.2 64-Bit Mode Stack Frame
#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn interrupt_entry() -> ! {
    naked_asm!(
        // RFLAGS are already saved in the IRET frame. It's safe to change it.
        "cld",

        // Happened in user mode?
        "test byte ptr [rsp + 24], 3",
        "jnz 2f",

        // An exception from the kernel mode.
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // The argument for handle_kernel_interrupt.
        "mov rdi, rsp",

        // Save RSP to RBX (a callee-saved register), since we need to align the
        // RSP to 16-bytes as mandated by ABI.
        "mov rbx, rsp",
        "and rsp, 0xfffffffffffffff0",

        "call {handle_kernel_interrupt}",

        // Restore RSP, and other registers.
        "mov rsp, rbx",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "pop rax",
        "add rsp, 16", // Skip vector and error code
        "iretq",

        // An exception from the user mode.
        "2:",
        "cli",
        "swapgs",
        "push rax",

        // thread = CpuVar.current_thread
        "mov rax, gs:[{current_thread_offset}]",

        // Save registers to the thread.
        "mov [rax + {rbx_offset}], rbx",
        "mov [rax + {rcx_offset}], rcx",
        "mov [rax + {rdx_offset}], rdx",
        "mov [rax + {rdi_offset}], rdi",
        "mov [rax + {rsi_offset}], rsi",
        "mov [rax + {rbp_offset}], rbp",
        "mov [rax + {r8_offset}], r8",
        "mov [rax + {r9_offset}], r9",
        "mov [rax + {r10_offset}], r10",
        "mov [rax + {r11_offset}], r11",
        "mov [rax + {r12_offset}], r12",
        "mov [rax + {r13_offset}], r13",
        "mov [rax + {r14_offset}], r14",
        "mov [rax + {r15_offset}], r15",

        // Save the user XSTATE.
        "push rax",
        "mov rdi, [rax + {xsave_ptr_offset}]",
        "mov eax, {xstate_mask_lo}",
        "mov edx, {xstate_mask_hi}",
        "xsaveopt64 [rdi]",
        "pop rax",

        // Save RAX too.
        "pop rbx",
        "mov [rax + {rax_offset}], rbx",

        // Pop arguments for the interrupt handler.
        "pop rdi", // vector
        "pop rsi", // error code

        // Pop IRET frame and save it to the thread.
        "pop rbx",
        "mov [rax + {rip_offset}], rbx",
        "pop rbx", // Drop CS
        "pop rbx",
        "mov [rax + {rflags_offset}], rbx",
        "pop rbx",
        "mov [rax + {rsp_offset}], rbx",
        "pop rbx",

        "jmp {handle_user_interrupt}",
        current_thread_offset = const offset_of!(CpuVar, current_thread),
        xstate_mask_lo = const XSTATE_MASK & 0xffff_ffff,
        xstate_mask_hi = const XSTATE_MASK >> 32,
        xsave_ptr_offset = const offset_of!(Thread, xsave_ptr),
        rip_offset = const offset_of!(Thread, rip),
        rflags_offset = const offset_of!(Thread, rflags),
        rax_offset = const offset_of!(Thread, rax),
        rbx_offset = const offset_of!(Thread, rbx),
        rcx_offset = const offset_of!(Thread, rcx),
        rdx_offset = const offset_of!(Thread, rdx),
        rsi_offset = const offset_of!(Thread, rsi),
        rdi_offset = const offset_of!(Thread, rdi),
        rsp_offset = const offset_of!(Thread, rsp),
        rbp_offset = const offset_of!(Thread, rbp),
        r8_offset = const offset_of!(Thread, r8),
        r9_offset = const offset_of!(Thread, r9),
        r10_offset = const offset_of!(Thread, r10),
        r11_offset = const offset_of!(Thread, r11),
        r12_offset = const offset_of!(Thread, r12),
        r13_offset = const offset_of!(Thread, r13),
        r14_offset = const offset_of!(Thread, r14),
        r15_offset = const offset_of!(Thread, r15),
        handle_kernel_interrupt = sym handle_kernel_interrupt,
        handle_user_interrupt = sym handle_user_interrupt,
    )
}

/// Checks if the kernel page fault is caused by touching a user address.
///
/// Returns the kernel RIP to recover from the fault.
fn recover_from_kernel_page_fault(rip: u64) -> Option<u64> {
    // User pointer access in FTL system calls.
    {
        let addr = &raw const usercopy0 as u64;
        let recover = &raw const usercopy0_recover as u64;
        if rip == addr {
            return Some(recover);
        }
    }

    {
        let addr = &raw const usercopy1 as u64;
        let recover = &raw const usercopy1_recover as u64;
        if rip == addr {
            return Some(recover);
        }
    }

    // System call frame writes.
    {
        let addr0 = &raw const syscall_copy0 as u64;
        let addr1 = &raw const syscall_copy1 as u64;
        let addr2 = &raw const syscall_copy2 as u64;
        let recover = syscall_copy_recover as *const () as u64;
        if rip == addr0 || rip == addr1 || rip == addr2 {
            return Some(recover);
        }
    }

    None
}

extern "C" fn handle_kernel_interrupt(frame: &mut InterruptFrame) {
    match frame.vector as u8 {
        EXCEPTION_PAGE_FAULT => {
            if let Some(recover_rip) = recover_from_kernel_page_fault(frame.rip) {
                frame.rip = recover_rip;
                frame.rax = 1;
                return;
            }

            // Unknown kernel page fault. This is a bug.
            let cr2: u64;
            unsafe {
                asm!("mov {cr2}, cr2", cr2 = out(reg) cr2);
            }
            panic!(
                "kernel page fault (CR2={cr2:#x}, RIP={:#x}, error_code={:#x})",
                frame.rip, frame.error_code
            );
        }
        vector if vector >= IRQ_VECTOR_BASE => {
            let irq = vector - IRQ_VECTOR_BASE;
            if irq == TIMER_IRQ {
                super::timer::handle_interrupt();
            } else if crate::net::is_irq(irq) {
                crate::net::handle_interrupt();
                super::io_apic::interrupt_acknowledge(irq);
                crate::scheduler::return_to_user();
            } else {
                panic!("unhandled kernel interrupt ({vector})");
            }
        }
        vector => {
            panic!(
                "unhandled kernel exception ({vector}), error_code={:#x}",
                frame.error_code
            )
        }
    }
}

extern "C" fn handle_user_interrupt(vector: u8, error_code: u64) -> ! {
    match vector {
        EXCEPTION_PAGE_FAULT => {
            let cr2: u64;
            unsafe {
                asm!("mov {cr2}, cr2", cr2 = out(reg) cr2);
            }

            trace!("Page Fault (CR2={:x})", cr2);
        }
        vector if vector >= IRQ_VECTOR_BASE => {
            let irq = vector - IRQ_VECTOR_BASE;
            if irq == TIMER_IRQ {
                // trace!("timer interrupt");
                super::timer::handle_interrupt();
            } else if crate::net::is_irq(irq) {
                crate::net::handle_interrupt();
                super::io_apic::interrupt_acknowledge(irq);
            } else {
                trace!("unhandled interrupt ({vector}), error_code={error_code:#x}");
            }
        }
        _ => {
            panic!("unhandled exception ({vector}), error_code={error_code:#x}");
        }
    }

    crate::scheduler::return_to_user();
}

pub(super) fn init() {
    let handlers_vaddr = VAddr::new(unsafe { &idt_handlers as *const u8 as usize });
    let handler_base = handlers_vaddr.as_usize() as u64;

    let mut idt = IDT.lock();
    for i in 0..NUM_IDT_ENTRIES {
        let handler = handler_base + i as u64 * INTERRUPT_HANDLER_SIZE;
        idt[i].offset0 = handler as u16;
        idt[i].selector = GDT_KERNEL_CS;
        idt[i].ist = 0;
        idt[i].gate_type = 0x8e; // Interrupt gate.
        idt[i].offset1 = (handler >> 16) as u16;
        idt[i].offset2 = (handler >> 32) as u32;
    }

    // Build an IDTR.
    let idt_vaddr = VAddr::new(idt.as_ptr() as usize);
    let idtr = Idtr {
        limit: (NUM_IDT_ENTRIES * size_of::<IdtEntry>() - 1) as u16,
        base: idt_vaddr.as_usize() as u64,
    };

    unsafe {
        asm!("lidt [{}]", in(reg) &idtr);
    }
}
