use core::arch::asm;
use core::arch::global_asm;

use super::boot::GDT_KERNEL_CS;
use crate::address::VAddr;
use crate::arch::x64::vmspace::vaddr2paddr;
use crate::spinlock::SpinLock;

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
interrupt_common:
    pop  rdi
    pop  rsi
    call handle_interrupt
    hlt

.set INTERRUPT_HANDLER_SIZE, 16
.align INTERRUPT_HANDLER_SIZE
.global idt_handlers
idt_handlers:
.set i, 0
.rept 256
.if i == 8 || 10 <= i && i <= 14 || i == 17
    cli
    push i
    jmp interrupt_common
    .align INTERRUPT_HANDLER_SIZE
.else
    cli
    push 0 // error code
    push i
    jmp interrupt_common
    .align INTERRUPT_HANDLER_SIZE
.endif

.set i, i + 1
.endr
"#
);

#[unsafe(no_mangle)]
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

    panic!("interrupt ({vector}): {vector_str}, error_code={error_code:#x}");
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
