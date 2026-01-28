use core::arch::asm;
use core::arch::global_asm;
use core::arch::naked_asm;
use core::mem::offset_of;

use super::boot::GDT_KERNEL_CS;
use crate::address::VAddr;
use crate::arch::Thread;
use crate::arch::get_cpuvar;
use crate::arch::x64::console;
use crate::arch::x64::console::SERIAL_IRQ;
use crate::arch::x64::vmspace::vaddr2paddr;
use crate::cpuvar::CpuVar;
use crate::spinlock::SpinLock;
use crate::thread::return_to_user;

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
}

const NUM_IDT_ENTRIES: usize = 256;
const INTERRUPT_HANDLER_SIZE: u64 = 16;

const IDT_ENTRY_DEFAULT: IdtEntry = IdtEntry {
    offset0: 0,
    selector: GDT_KERNEL_CS,
    ist: 0,
    gate_type: 0x8e, // interrupt gate
    offset1: 0,
    offset2: 0,
    reserved: 0,
};

static IDT: SpinLock<[IdtEntry; NUM_IDT_ENTRIES]> =
    SpinLock::new([IDT_ENTRY_DEFAULT; NUM_IDT_ENTRIES]);

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

#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn interrupt_entry() -> ! {
    unsafe {
        naked_asm!(
            "cli",
            "cld",

            // TODO: SWAPGS is tricky and error-prone. If a double exception
            //       occurs, SWAPGS will switch to the user's one. Typically,
            //       a interrupt handler checks RFLAGS.CPL or CS, but in FTL
            //       applications can run in the kernel mode.
            //
            //       We could use RFLAGS.IF to determine if we are in the
            //       kernel context (assuming interrupts are disabled).
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

            "jmp {handle_interrupt}",
            current_thread_offset = const offset_of!(CpuVar, current_thread),
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
            handle_interrupt = sym handle_interrupt,
        )
    }
}

extern "C" fn handle_interrupt(vector: u8, error_code: u64) -> ! {
    let vector_str = match vector {
        0 => "Divide Error",
        1 => "Debug",
        3 => "Breakpoint",
        4 => "Overflow",
        5 => "BOUND Range Exceeded",
        6 => "Invalid Opcode",
        7 => "Device Not Available",
        8 => "Double Fault",
        10 => "Invalid TSS",
        11 => "Segment Not Present",
        12 => "Stack Segment Fault",
        13 => "General Protection",
        14 => "Page Fault",
        16 => "Floating-Point Error",
        17 => "Alignment Check",
        18 => "Machine Check",
        19 => "SIMD Floating-Point Numeric Error",
        _ => "Unknown Exception",
    };

    let cpuvar = get_cpuvar();
    if vector == 32 + SERIAL_IRQ as u8 {
        console::handle_interrupt(cpuvar);
    } else {
        panic!("unhandled interrupt ({vector}): {vector_str}, error_code={error_code:#x}");
    }

    return_to_user();
}

pub(super) fn init() {
    let handlers_vaddr = VAddr::new(unsafe { &idt_handlers as *const u8 as usize });
    let handler_base = vaddr2paddr(handlers_vaddr).as_u64();

    let mut idt = IDT.lock();
    for i in 0..NUM_IDT_ENTRIES {
        let handler = handler_base + i as u64 * INTERRUPT_HANDLER_SIZE;
        idt[i].offset0 = handler as u16;
        idt[i].offset1 = (handler >> 16) as u16;
        idt[i].offset2 = (handler >> 32) as u32;
    }

    // Build an IDTR.
    let idt_vaddr = VAddr::new(idt.as_ptr() as usize);
    let idt_paddr = vaddr2paddr(idt_vaddr).as_u64();
    let idtr = Idtr {
        limit: (NUM_IDT_ENTRIES * size_of::<IdtEntry>() - 1) as u16,
        base: idt_paddr,
    };

    unsafe {
        asm!("lidt [{}]", in(reg) &idtr);
    }
}
