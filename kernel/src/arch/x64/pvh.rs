//! PVH boot protocol.

use ftl_arrayvec::ArrayVec;
use ftl_utils::formatter::ByteSize;

use super::vmspace::paddr2vaddr;
use crate::address::PAddr;
use crate::boot::BootInfo;
use crate::boot::FreeRam;
use crate::boot::Module;

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
            "invalid magic number in PVH start info: {:x?}",
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

    let mut modules = ArrayVec::<Module, 8>::new();
    for module in modlist {
        let start = PAddr::new(module.paddr as usize);
        let end = PAddr::new(module.paddr as usize + module.size as usize);
        if modules.try_push(Module { start, end }).is_err() {
            trace!("too many modules: {start} - {end}");
        }
    }

    let memmap_paddr = PAddr::new(start_info.memmap_paddr as usize);
    let memmap = unsafe {
        core::slice::from_raw_parts(
            paddr2vaddr(memmap_paddr).as_usize() as *const HvmMemoryMapEntry,
            start_info.memmap_entries as usize,
        )
    };

    let mut free_rams = ArrayVec::<FreeRam, 8>::new();
    for entry in memmap {
        if entry.type_ == HVM_MEMMAP_TYPE_RAM {
            let addr = PAddr::new(entry.addr as usize);
            let size = entry.size as usize;
            if free_rams.try_push(FreeRam { addr, size }).is_err() {
                trace!("too many free RAM regions: {addr} {}", ByteSize(size));
            }
        }
    }

    BootInfo {
        free_rams,
        modules: ArrayVec::new(),
    }
}
