//! Multiboot2 support.
//!
//! <https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html#Boot-information-format>

use core::arch::global_asm;
use core::mem::size_of;

use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::arch::x64::bootinfo::exclude_reserved_regions;
use crate::arch::x64::bootinfo::reserved_regions_with_initfs;
use crate::boot::BootInfo;
use crate::boot::FreeRam;

#[derive(Debug)]
#[repr(C, packed)]
struct Multiboot2BootInfoHeader {
    total_size: u32,
    reserved: u32,
}

#[derive(Debug)]
#[repr(C, packed)]
struct Multiboot2TagHeader {
    type_: u32,
    size: u32,
}

#[repr(C, packed)]
struct Multiboot2MemoryMapHeader {
    header: Multiboot2TagHeader,
    entry_size: u32,
    entry_version: u32,
}

#[repr(C, packed)]
struct Multiboot2MemoryMapEntry {
    base_addr: u64,
    length: u64,
    type_: u32,
    reserved: u32,
}

#[repr(C, packed)]
struct Multiboot2Module {
    header: Multiboot2TagHeader,
    /// The start physical address.
    mod_start: u32,
    /// The end physical address.
    mod_end: u32,
}

pub const MULTIBOOT2_TAG_MODULE: u32 = 3;
pub const MULTIBOOT2_TAG_MEMORY_MAP: u32 = 6;

#[derive(Clone, Copy)]
struct MemoryRegion {
    start: u64,
    end: u64,
    type_: u32,
}

// The multiboot2 header.
//https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html#Header-layout
global_asm!(
    r#"
.set MULTIBOOT2_MAGIC, 0xe85250d6
.set MULTIBOOT2_ARCH, 0 // i386
.set MULTIBOOT2_LENGTH, multiboot2_end - multiboot2

.pushsection .multiboot2, "a"
multiboot2:
    .long MULTIBOOT2_MAGIC
    .long MULTIBOOT2_ARCH
    .long MULTIBOOT2_LENGTH  // header_length
    .long -(MULTIBOOT2_MAGIC + MULTIBOOT2_ARCH + MULTIBOOT2_LENGTH) // checksum

    // terminator
    .word 0 // type
    .word 0 // flags
    .word 0 // checksum
multiboot2_end:

.popsection
"#
);

unsafe fn paddr2ptr<T>(paddr: usize) -> &'static T {
    let vaddr = super::paddr2vaddr(PAddr::new(paddr));
    unsafe { &*(vaddr.as_usize() as *const T) }
}

pub(super) fn parse_multiboot2_info(info_addr: PAddr) -> BootInfo {
    let info_addr = info_addr.as_usize();

    let header: &Multiboot2BootInfoHeader = unsafe { paddr2ptr(info_addr) };
    let total_size = header.total_size as usize;

    let mut offset = size_of::<Multiboot2BootInfoHeader>();
    let mut memory_regions = ArrayVec::<MemoryRegion, 64>::new();
    let mut initfs = None;
    let mut initfs_range = None;
    while offset < total_size {
        let tag_header: &Multiboot2TagHeader = unsafe { paddr2ptr(info_addr + offset) };

        match tag_header.type_ {
            MULTIBOOT2_TAG_MEMORY_MAP => {
                let mmap: &Multiboot2MemoryMapHeader = unsafe { paddr2ptr(info_addr + offset) };

                let num_entries = (mmap.header.size / mmap.entry_size) as usize;
                for i in 0..num_entries {
                    let entry: &Multiboot2MemoryMapEntry = unsafe {
                        paddr2ptr(
                            info_addr
                                + offset
                                + size_of::<Multiboot2MemoryMapHeader>()
                                + i * mmap.entry_size as usize,
                        )
                    };
                    let addr = entry.base_addr;
                    let length = entry.length;
                    let type_ = entry.type_;
                    let Some(end) = addr.checked_add(length) else {
                        trace!("memory map entry overflows: {addr:x} + {length:x}");
                        continue;
                    };
                    trace!(
                        "memory map: {:08x} - {:08x} ({} MB - {})",
                        addr,
                        end,
                        length / (1024 * 1024),
                        type_
                    );
                    if memory_regions
                        .try_push(MemoryRegion {
                            start: addr,
                            end,
                            type_,
                        })
                        .is_err()
                    {
                        trace!("too many memory map entries, ignoring {:x}", addr);
                        break;
                    }
                }
            }
            MULTIBOOT2_TAG_MODULE => {
                let module: &Multiboot2Module = unsafe { paddr2ptr(info_addr + offset) };
                let start = module.mod_start;
                let end = module.mod_end;
                let slice = unsafe {
                    core::slice::from_raw_parts(paddr2ptr(start as usize), (end - start) as usize)
                };
                assert!(initfs.is_none(), "multiple modules found");
                initfs = Some(slice);
                initfs_range = Some(start as u64..end as u64);
            }
            _ => {
                let type_ = tag_header.type_;
                trace!("unknown tag type: {:08x}", type_);
            }
        }

        // Advance to next tag (8-byte aligned)
        offset += (tag_header.size as usize + 7) & !7;
    }

    let initfs = initfs.expect("initfs module not found");
    let initfs_range = initfs_range.expect("initfs module range not found");

    let reserved_regions = reserved_regions_with_initfs(initfs_range);

    let mut free_rams = ArrayVec::new();
    for region in memory_regions.iter() {
        if region.type_ != 1 {
            continue;
        }

        exclude_reserved_regions(
            region.start,
            region.end,
            reserved_regions.as_ref(),
            |start, end| {
                let start = PAddr::new(start as usize);
                let end = PAddr::new(end as usize);
                let size = end.as_usize() - start.as_usize();
                trace!("RAM: {start} - {end} ({} KiB)", size / 1024);
                if free_rams.try_push(FreeRam { start, end }).is_err() {
                    trace!("too many free RAM regions: {start} - {end}");
                }
            },
        );
    }

    BootInfo { free_rams, initfs }
}
