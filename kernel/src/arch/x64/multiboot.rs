//! Multiboot2 support.
//!
//! <https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html#Boot-information-format>

use core::arch::global_asm;
use core::mem::size_of;
use core::slice;

use ftl_arrayvec::ArrayVec;
use ftl_utils::formatter::ByteSize;

use crate::address::PAddr;
use crate::boot::BootInfo;
use crate::boot::FreeRam;
use crate::boot::Module;

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

const MULTIBOOT2_TAG_CMDLINE: u32 = 1;
const MULTIBOOT2_TAG_MODULE: u32 = 3;
const MULTIBOOT2_TAG_MEMORY_MAP: u32 = 6;
const MULTIBOOT2_MEMORY_AVAILABLE: u32 = 1;

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
    .long 8 // size
multiboot2_end:

.popsection
"#
);

unsafe fn paddr2ptr<T>(paddr: usize) -> &'static T {
    let vaddr = super::vmspace::paddr2vaddr(PAddr::new(paddr));
    unsafe { &*(vaddr.as_usize() as *const T) }
}

pub(super) fn parse_multiboot2_info(info_addr: PAddr) -> BootInfo {
    let info_addr = info_addr.as_usize();

    let header: &Multiboot2BootInfoHeader = unsafe { paddr2ptr(info_addr) };
    let total_size = header.total_size as usize;

    let mut offset = size_of::<Multiboot2BootInfoHeader>();
    let mut free_rams = ArrayVec::<FreeRam, 8>::new();
    let mut modules = ArrayVec::<Module, 8>::new();
    let mut cmdline: &'static [u8] = b"";
    while offset < total_size {
        let tag_header: &Multiboot2TagHeader = unsafe { paddr2ptr(info_addr + offset) };

        match tag_header.type_ {
            MULTIBOOT2_TAG_CMDLINE => {
                let start = info_addr + offset + size_of::<Multiboot2TagHeader>();
                let len =
                    (tag_header.size as usize).saturating_sub(size_of::<Multiboot2TagHeader>());
                let cmdline_with_null = unsafe { slice::from_raw_parts(paddr2ptr(start), len) };
                cmdline = if let Some(null_index) = cmdline_with_null.iter().position(|b| *b == 0) {
                    &cmdline_with_null[..null_index]
                } else {
                    cmdline_with_null
                };
            }
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
                    let addr = PAddr::new(entry.base_addr as usize);
                    let size = entry.length as usize;
                    let type_ = entry.type_;

                    if type_ != MULTIBOOT2_MEMORY_AVAILABLE {
                        // Not a free RAM region.
                        continue;
                    }

                    if free_rams.try_push(FreeRam { addr, size }).is_err() {
                        trace!("too many free RAM regions: {addr} {}", ByteSize(size));
                    }
                }
            }
            MULTIBOOT2_TAG_MODULE => {
                let module: &Multiboot2Module = unsafe { paddr2ptr(info_addr + offset) };
                let start = module.mod_start;
                let end = module.mod_end;

                if modules
                    .try_push(Module {
                        start: PAddr::new(start as usize),
                        end: PAddr::new(end as usize),
                    })
                    .is_err()
                {
                    trace!("too many modules: {start} - {end}");
                }
            }
            _ => {
                let type_ = tag_header.type_;
                trace!("unknown tag type: {:08x}", type_);
            }
        }

        // Advance to next tag (8-byte aligned)
        offset += (tag_header.size as usize + 7) & !7;
    }

    BootInfo {
        cmdline,
        free_rams,
        modules,
    }
}
