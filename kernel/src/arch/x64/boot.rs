use core::arch::asm;
use core::arch::naked_asm;

use super::multiboot;
use super::pvh;
use super::vmspace::BOOT_PDPT;
use super::vmspace::BOOT_PML4;
use super::vmspace::KERNEL_BASE;
use crate::address::PAddr;

extern "C" fn rust_boot(multiboot_magic: u32, start_info: u32) -> ! {
    super::console::init();

    // SeaBIOS prints an escape sequence which disables line wrapping, and messes up
    // your terminal. Revert it.
    println!("\x1b[?7h");

    trace!("Booting FTL...");
    enable_fsgsbase();

    let cpu_id = 0;
    super::gdt::init(cpu_id);
    super::idt::init();
    super::pic::init();
    super::syscall::init();
    super::mp_table::init();
    super::timer::init();

    let bootinfo = if multiboot_magic == 0x36d76289 {
        multiboot::parse_multiboot2_info(PAddr::new(start_info as usize))
    } else {
        pvh::parse_start_info(PAddr::new(start_info as usize))
    };

    crate::boot::boot(bootinfo);
}

fn enable_fsgsbase() {
    // TODO: CPUID check
    unsafe {
        asm!(
            "mov rax, cr4",
            "or rax, 1 << 16",
            "mov cr4, rax",
            out("rax") _,
        );
    }
}

/// The per-CPU kernel stack size.
pub(super) const KERNEL_STACK_SIZE: usize = 1024 * 1024;

/// A kernel stack. Aligned to 16 bytes for ABI.
#[repr(align(16))]
struct Stack(#[allow(dead_code)] [u8; KERNEL_STACK_SIZE]);

#[unsafe(link_section = ".bss")]
static BSP_STACK: Stack = Stack([0; KERNEL_STACK_SIZE]);

pub(super) fn bsp_stack_top() -> u64 {
    BSP_STACK.0.as_ptr() as u64 + KERNEL_STACK_SIZE as u64
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn x64_boot() -> ! {
    naked_asm!(
        // The entry point. The kernel boots from this assembly code.
        //
        // This boot code supports 2 boot protocols:
        //
        // - PVH boot protocol: EBX = HvmStartInfo
        // - Multiboot2 boot protocol: EBX = Multiboot2BootInfoHeader
        //
        // In both protocols, the CPU is in the following state:
        //
        // - 32-bit protected mode
        // - paging disabled
        // - EIP is in physical address
        //
        // Important: Symbols are in the high virtual address space
        //            (KERNEL_BASE). Be careful when using symbols!
        ".code32",
        "cli",
        "cld",

        // Prepare the arguments for rust_boot. Do not touch EDI/ESI in this function!
        "mov edi, eax", // multiboot2 magic
        "mov esi, ebx", // Multiboot2BootInfoHeader or HvmStartInfo

        // Initialize the stack for this bootstrap processor (BSP).
        "lea esp, [{BSP_STACK_BOTTOM} + {KERNEL_STACK_SIZE} - {KERNEL_BASE}]",

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

        // Get the physical addresses of main and the stack.
        "lea rax, [{pvh_main} - {KERNEL_BASE}]",
        "lea rsp, [{BSP_STACK_BOTTOM} + {KERNEL_STACK_SIZE} - {KERNEL_BASE}]",

        // Convert them into virtual addresses.
        "mov rbx, {KERNEL_BASE}",
        "or  rax, rbx",
        "or  rsp, rbx",

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
        BSP_STACK_BOTTOM = sym BSP_STACK,
        BOOT_PML4 = sym BOOT_PML4,
        BOOT_PDPT = sym BOOT_PDPT,
        KERNEL_STACK_SIZE = const KERNEL_STACK_SIZE,
        KERNEL_BASE = const KERNEL_BASE,
    );
}
