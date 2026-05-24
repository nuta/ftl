use core::arch::asm;
use core::arch::global_asm;
use core::mem::MaybeUninit;
use core::mem::offset_of;

use super::NUM_CPUS_MAX;
use crate::address::VAddr;

const NUM_GDT_ENTRIES: usize = 8;
pub(super) const GDT_KERNEL_CS: u16 = 8;
pub(super) const GDT_USER_DS: u16 = (4 * 8) | 3;
pub(super) const GDT_USER_CS: u16 = (5 * 8) | 3;
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

// TODO: Build TSS at compile time.
fn write_tss(cpu_id: usize) -> u64 {
    unsafe {
        let tss = &mut TSS_ENTRIES[cpu_id];
        tss.write(Tss {
            reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved1: 0,
            ist: [super::boot::bsp_stack_top(), 0, 0, 0, 0, 0, 0],
            reserved2: 0,
            reserved3: 0,
            iomap_offset: offset_of!(Tss, io_permission_map) as u16,
            io_permission_map: [0xff; 8192],
        });

        let vaddr = VAddr::new(tss.as_ptr() as usize);
        vaddr.as_usize() as u64
    }
}

pub(super) fn init(cpu_id: usize) {
    let tss_base = write_tss(cpu_id);

    // Build a 64-bit TSS descriptor.
    let mut tss_low = 0x0000890000000000;
    tss_low |= (size_of::<Tss>() - 1) as u64; // limit (size - 1)
    tss_low |= (tss_base & 0x00ff_ffff) << 16; // base[0:23]
    tss_low |= (tss_base & 0xff00_0000) << 32; // base[24:31]
    let tss_high = tss_base >> 32; // base[32:63]

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
    let gdt_base = gdt_vaddr.as_usize() as u64;
    let gdtr = Gdtr {
        limit: (NUM_GDT_ENTRIES * size_of::<u64>() - 1) as u16,
        base: gdt_base,
    };

    unsafe {
        asm!("lgdt [{}]", in(reg) &gdtr);
        asm!("ltr ax", in("ax") GDT_TSS);
    }
}
