use core::arch::asm;
use core::arch::global_asm;
use core::arch::naked_asm;
use core::mem::MaybeUninit;
use core::mem::offset_of;

use super::vmspace::BOOT_PDPT;
use super::vmspace::BOOT_PML4;
use super::vmspace::KERNEL_BASE;
use crate::address::PAddr;
use crate::address::VAddr;
use crate::arch::NUM_CPUS_MAX;
use crate::arch::x64::console::SERIAL_IRQ;
use crate::arch::x64::io_apic::use_ioapic;
use crate::arch::x64::pvh;
use crate::arch::x64::vmspace::vaddr2paddr;

pub(super) const NUM_GDT_ENTRIES: usize = 8;
pub(super) const GDT_KERNEL_CS: u16 = 8;
const GDT_TSS: u16 = 6 * 8;

#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base: u64,
}

#[repr(C, packed)]
pub(super) struct Tss {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iomap_offset: u16,
    /// The I/O permission map.
    ///
    /// - Each bit corresponds to an I/O port. If set, the port is not accessible.
    /// - The last byte must be `0xff`.
    io_permission_map: [u8; 8192],
}

static mut GDT_ENTRIES: [MaybeUninit<[u64; NUM_GDT_ENTRIES]>; NUM_CPUS_MAX] =
    [const { MaybeUninit::uninit() }; NUM_CPUS_MAX];

static mut TSS_ENTRIES: [MaybeUninit<Tss>; NUM_CPUS_MAX] =
    [const { MaybeUninit::uninit() }; NUM_CPUS_MAX];

extern "C" fn rust_boot(start_info: PAddr) -> ! {
    super::console::init();

    println!("\nBooting FTL...");

    // Enable FS/GS base.
    unsafe {
        asm!(
            "mov rax, cr4",
            "or rax, 1 << 16",
            "mov cr4, rax",
            out("rax") _,
        );
    }

    let cpu_id = 0; // FIXME:

    // Build a TSS.
    let tss_vaddr = unsafe {
        let tss = &mut TSS_ENTRIES[cpu_id];
        tss.write(Tss {
            reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved1: 0,
            ist: [0; 7],
            reserved2: 0,
            reserved3: 0,
            iomap_offset: offset_of!(Tss, io_permission_map) as u16,
            io_permission_map: [0xff; 8192],
        });

        VAddr::new(tss.as_ptr() as usize)
    };

    // Build a 64-bit TSS descriptor.
    let tss_paddr = vaddr2paddr(tss_vaddr).as_u64();
    let mut tss_low = 0x0000890000000000;
    tss_low |= (size_of::<Tss>() - 1) as u64; // limit (size - 1)
    tss_low |= (tss_paddr & 0x00ff_ffff) << 16; // base[0:23]
    tss_low |= (tss_paddr & 0xff00_0000) << 32; // base[24:31]
    let tss_high = tss_paddr >> 32; // base[32:63]

    // Build a GDT.
    let gdt_vaddr = unsafe {
        let table = &mut GDT_ENTRIES[cpu_id];
        table.write([
            0x0000000000000000, // null
            0x00af9a000000ffff, // kernel_cs
            0x00af92000000ffff, // kernel_ds
            0x0000000000000000, // user_cs32
            0x008ff2000000ffff, // user_ds
            0x00affa000000ffff, // user_cs64
            tss_low,            // tss_low
            tss_high,           // tss_high
        ]);
        VAddr::new(table.as_ptr() as usize)
    };

    // Build a GDTR.
    let gdt_paddr = vaddr2paddr(gdt_vaddr).as_u64();
    let gdtr = Gdtr {
        limit: (NUM_GDT_ENTRIES * size_of::<u64>() - 1) as u16,
        base: gdt_paddr,
    };

    unsafe {
        asm!("lgdt [{}]", in(reg) &gdtr);
        asm!("ltr ax", in("ax") GDT_TSS);
    }

    super::idt::init();
    super::pic::init();
    super::mp_table::init();

    use_ioapic(|ioapic| {
        ioapic
            .enable_irq(SERIAL_IRQ)
            .expect("failed to enable serial IRQ");
    });

    let bootinfo = pvh::parse_start_info(start_info);
    crate::boot::boot(&bootinfo);
}

// Defines a temporary GDT to boot a CPU. Another per-CPU GDT will be set up later.
//
// This is written in assembly because it needs some pointer arithmetic which is not
// allowed in Rust's static initialization.
global_asm!(
    r#"
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
"#
);

/// The per-CPU kernel stack size.
pub(super) const KERNEL_STACK_SIZE: usize = 1024 * 1024;

#[repr(align(16))]
struct Stack(#[allow(unused)] [u8; KERNEL_STACK_SIZE]);

#[unsafe(link_section = ".data")]
static BSP_STACK: MaybeUninit<Stack> = MaybeUninit::uninit();

#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn x64_boot() -> ! {
    naked_asm!(
        // The entry point. The kernel boots from this assembly code.
        //
        // - PVH boot protocol: EBX = HvmStartInfo
        // - 32-bit protected mode
        // - paging disabled
        // - EIP is in physical address
        //
        // Important: Symbols are in the high virtual address space
        //            (KERNEL_BASE). Be careful when using symbols!
        ".code32",
        "cli",
        "cld",

        // Prepare arguments for rust_boot. Do not modify edi in this code!
        "mov edi, ebx",

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
