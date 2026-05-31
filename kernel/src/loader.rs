use core::slice;

use ftl_elf::Elf;
use ftl_elf::PhdrType;
use ftl_utils::alignment::align_up;

use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::boot::BootInfo;
use crate::initfs;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;

fn load_elf(elf_file: &[u8]) {
    let elf = match Elf::parse(elf_file, ftl_elf::ET_DYN) {
        Ok(elf) => elf,
        Err(e) => {
            error!("failed to parse ELF file: {:?}", e);
            return;
        }
    };

    // Find the end of the image to calculate the size of the memory it needs.
    let mut image_size = 0;
    for phdr in elf.phdrs {
        if phdr.p_type == PhdrType::Load as u32 {
            image_size = image_size.max(phdr.p_vaddr + phdr.p_memsz);
        }
    }

    let image_size = align_up(image_size as usize, MIN_PAGE_SIZE);
    let image_paddr = match PAGE_ALLOCATOR.alloc(image_size, PageType::Zeroed) {
        Some(paddr) => paddr,
        None => {
            error!("out of memory: {} bytes", image_size);
            return;
        }
    };

    let image_ptr: *mut u8 = arch::paddr2vaddr(image_paddr).as_mut_ptr();
    let image = unsafe { slice::from_raw_parts_mut(image_ptr, image_size) };

    // Load the segments into the allocated memory.
    for phdr in elf.phdrs {
        if phdr.p_type != PhdrType::Load as u32 {
            continue;
        }

        // Copy the file contents to the allocated memory.
        let src_off = phdr.p_offset as usize;
        let dst_off = phdr.p_vaddr as usize;
        let copy_len = phdr.p_filesz as usize;
        let src = &elf_file[src_off..src_off + copy_len];
        let dst = &mut image[dst_off..dst_off + copy_len];
        dst.copy_from_slice(src);

        // Zero the remaining memory.
        let zeroed_off = dst_off + copy_len;
        let zeroed_len = phdr.p_memsz as usize - copy_len;
        if zeroed_len > 0 {
            let zeroed_range = (zeroed_off)..(zeroed_off + zeroed_len);
            dst[zeroed_range].fill(0);
        }
    }

    let entry = elf.ehdr.e_entry;
    unsafe {
        let entry_fn = image.as_ptr().add(entry as usize);
        trace!("Calling entry point: {:p}", entry_fn);
        core::mem::transmute::<*const u8, extern "C" fn()>(entry_fn)();
        trace!("Entry point returned");
    }
}

pub fn init(bootinfo: &BootInfo) {
    for module in &bootinfo.modules {
        let initfs = initfs::InitFsLoader::new(module);
        for file in initfs {
            if file.name.starts_with(b"servers/") && file.name.ends_with(b".elf") {
                trace!("loading {}...", core::str::from_utf8(file.name).unwrap());
                load_elf(file.data);
            }
        }
    }
}
