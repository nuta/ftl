//! PVH boot protocol.

use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::arch::paddr2vaddr;
use crate::arch::x64::bootinfo::exclude_reserved_regions;
use crate::arch::x64::bootinfo::reserved_regions_with_initfs;
use crate::boot::BootInfo;
use crate::boot::FreeRam;

/// Xen boot information.
///
/// <https://xenbits.xen.org/docs/unstable/hypercall/x86_64/include,public,arch-x86,hvm,start_info.h.html>
#[repr(C, packed)]
struct HvmStartInfo {
    magic: [u8; 4],
    version: u32,
    flags: u32,
    nr_modules: u32,
    modlist_paddr: u64,
    cmdline_paddr: u64,
    rsdp_paddr: u64,
    memmap_paddr: u64,
    memmap_entries: u32,
    reserved: u32,
}

const HVM_MEMMAP_TYPE_RAM: u32 = 1;

#[repr(C, packed)]
struct HvmModuleEntry {
    paddr: u64,
    size: u64,
    cmdline_paddr: u64,
    reserved: u64,
}

#[repr(C, packed)]
struct HvmMemoryMapEntry {
    addr: u64,
    size: u64,
    type_: u32,
    reserved: u32,
}

pub fn parse_start_info(start_info: PAddr) -> BootInfo {
    let start_info = unsafe { &*(paddr2vaddr(start_info).as_usize() as *const HvmStartInfo) };
    if start_info.magic != [b'x', b'E' | 0x80, b'n', b'3'] {
        panic!(
            "Invalid magic number in PVH start info: {:x?}",
            start_info.magic
        );
    }

    let modlist_paddr = PAddr::new(start_info.modlist_paddr as usize);
    let modlist = unsafe {
        core::slice::from_raw_parts(
            paddr2vaddr(modlist_paddr).as_usize() as *const HvmModuleEntry,
            start_info.nr_modules as usize,
        )
    };

    if modlist.len() != 1 {
        panic!(
            "unexpected number of modules ({} found, expected 1 for initfs)",
            modlist.len()
        );
    }

    let initfs_module = &modlist[0];
    let paddr = PAddr::new(initfs_module.paddr as usize);
    let size = initfs_module.size as usize;
    let initfs =
        unsafe { core::slice::from_raw_parts(paddr2vaddr(paddr).as_usize() as *const u8, size) };

    println!("found a module (initfs): paddr={}, size={}", paddr, size);

    let memmap_paddr = PAddr::new(start_info.memmap_paddr as usize);
    let memmap = unsafe {
        core::slice::from_raw_parts(
            paddr2vaddr(memmap_paddr).as_usize() as *const HvmMemoryMapEntry,
            start_info.memmap_entries as usize,
        )
    };

    // QEMU does not exclude module regions from the free RAM regions. Exclude
    // them manually so that the kernel won't try to allocate from them.
    let initfs_range = initfs_module.paddr..initfs_module.paddr + initfs_module.size;
    let reserved_regions = reserved_regions_with_initfs(initfs_range);

    let mut free_rams = ArrayVec::new();
    for entry in memmap {
        let start = entry.addr;
        let size = entry.size;
        let type_ = entry.type_;
        if type_ == HVM_MEMMAP_TYPE_RAM {
            let Some(end) = entry.addr.checked_add(size) else {
                println!("the size of the memory region overflows: {start:x} + {size:x}");
                continue;
            };

            exclude_reserved_regions(start, end, reserved_regions.as_ref(), |start, end| {
                let start = PAddr::new(start as usize);
                let end = PAddr::new(end as usize);
                let size = end.as_usize() - start.as_usize();
                println!("RAM: {start} - {end} ({} KiB)", size / 1024);
                if free_rams.try_push(FreeRam { start, end }).is_err() {
                    println!("too many free RAM regions: {start} - {end}");
                }
            });
        }
    }

    BootInfo { free_rams, initfs }
}
