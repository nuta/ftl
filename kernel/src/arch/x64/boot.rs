use core::mem::MaybeUninit;
use core::arch::naked_asm;
use core::arch::global_asm;

use super::vmspace::{BOOT_PML4, BOOT_PDPT};
use super::KERNEL_BASE;

/// The per-CPU kernel stack size.
const KERNEL_STACK_SIZE: usize = 512 * 1024;

#[repr(align(4096))]
struct Stack(#[allow(unused)] [u8; KERNEL_STACK_SIZE]);

#[unsafe(link_section = ".data")]
static BSP_STACK: MaybeUninit<Stack> = MaybeUninit::uninit();

extern "C" fn rust_boot() -> ! {
    super::console::init();

    println!("\nHello world!\n");
    loop {}
}

// Defines a temporary GDT to boot a CPU. Another per-CPU GDT will be set up later.
//
// This is written in assembly because it needs some pointer arithmetic which is not
// allowed in Rust's static initialization.
global_asm!(r#"
.pushsection .rodata

// GDTR.
.align 8
.global boot_gdtr
boot_gdtr:
    .word gdt_end - gdt - 1
    .quad gdt

// Global Descriptor Table (GDT).
.align 8
gdt:
    .quad 0                  // 0:  null segment
    .quad 0x00af9a000000ffff // 8:  64-bit code segment (kernel)
    .quad 0x00cf92000000ffff // 16: 64-bit data segment (kernel)
gdt_end:

.popsection
"#);

#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn x64_boot() -> ! {
    naked_asm!(
        ".code32",
        "cli",
        "cld",

        // Initialize the stack for this bootstrap processor (BSP).
        "lea esp, [{BSP_STACK_TOP} + {KERNEL_STACK_SIZE} - {KERNEL_BASE}]",

        // Enable CPU features.
        "mov eax, cr4",
        "or  eax, 1 << 5 | 1 << 7", // PAE, Global page
        "mov cr4, eax",

        // Fill the page table (PML4).
        "lea ebx, [{BOOT_PML4} - {KERNEL_BASE}]", // EBX = physical address of BOOT_PML4
        "lea eax, [{BOOT_PDPT} - {KERNEL_BASE}]", // EAX = physical address of BOOT_PDPT
        "or  eax, 1",                           // PTE_V
        "mov [ebx], eax",                       // Entry 0: maps 0 (identity mapping for x64_boot)
        "mov [ebx + 256 * 8], eax",             // Entry 256: maps KERNEL_BASE

        // Set the page table.
        "mov cr3, ebx",

        // Enable Long Mode.
        "mov ecx, 0xc0000080", // EFER MSR
        "rdmsr",
        "or eax, 1 << 8", // Long Mode Enable
        "wrmsr",

        // Enable paging.
        "mov eax, cr0",
        "or eax, 1 << 31", // Paging
        "mov cr0, eax",

        // Prepare for RETF.
        "push 8",
        "lea eax, [2f - {KERNEL_BASE}]",
        "push eax",

        // Enter long mode.
        "lgdt [boot_gdtr - {KERNEL_BASE}]",
        "retf",

        // 64-bit mode from now on (still at physical addresses).
        ".code64",
        "2:",
        "mov ax, 0",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",

        // Use the high virtual address in RSP.
        "mov rax, {KERNEL_BASE}",
        "add rsp, rax",

        // Get the virtual address of main.
        "lea rax, [{pvh_main} - {KERNEL_BASE}]",
        "mov rbx, {KERNEL_BASE}",
        "or  rax, rbx",

        // Jump to main.
        "jmp rax",

        // The ELF note for PVH boot protocol.
        ".pushsection .note.pvh, \"a\", @note",
        ".long 4",                        // n_namesz (4 bytes)
        ".long 4",                        // n_descsz (4 bytes)
        ".long 18",                       // n_type (XEN_ELFNOTE_PHYS32_ENTRY)
        ".asciz \"Xen\"",                 // name
        ".long x64_boot - {KERNEL_BASE}", // desc (physical address of the entry point)
        ".popsection",

        pvh_main = sym rust_boot,
        BSP_STACK_TOP = sym BSP_STACK,
        BOOT_PML4 = sym BOOT_PML4,
        BOOT_PDPT = sym BOOT_PDPT,
        KERNEL_STACK_SIZE = const KERNEL_STACK_SIZE,
        KERNEL_BASE = const KERNEL_BASE,
    );
}
