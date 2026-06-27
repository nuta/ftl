use core::mem::size_of;
use core::slice;

use ftl_api::start::StartInfo;
use ftl_elf::DT_NULL;
use ftl_elf::DT_RELA;
use ftl_elf::DT_RELASZ;
use ftl_elf::Dyn;
use ftl_elf::Elf;
use ftl_elf::PhdrType;
use ftl_elf::Rela;
use ftl_utils::alignment::align_up;

use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;

pub type EntryFn = extern "Rust" fn(start_info: *const StartInfo);

pub struct LoadedElf {
    pub image: *const u8,
    pub entry_fn: EntryFn,
}

#[derive(Debug)]
pub enum Error {
    ParseElf,
    OutOfMemory,
    BadRelocType,
    BadRelocOffset,
    BadRelocSize,
}

pub fn load_elf(elf_file: &[u8]) -> Result<LoadedElf, Error> {
    let elf = Elf::parse(elf_file, ftl_elf::ET_DYN).map_err(|_| Error::ParseElf)?;

    // Find the end of the image to calculate the size of the memory it needs.
    let mut image_size = 0;
    for phdr in elf.phdrs {
        if phdr.p_type == PhdrType::Load as u32 {
            image_size = image_size.max(phdr.p_vaddr + phdr.p_memsz);
        }
    }

    let image_size = align_up(image_size as usize, MIN_PAGE_SIZE);
    let image_paddr = PAGE_ALLOCATOR
        .alloc(image_size, PageType::Zeroed)
        .ok_or(Error::OutOfMemory)?;

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
            image[zeroed_range].fill(0);
        }
    }

    // Find the relocation table.
    let mut rela_addr = 0;
    let mut rela_size = 0;
    for phdr in elf.phdrs {
        if phdr.p_type != PhdrType::Dynamic as u32 {
            continue;
        }

        let dynamic = unsafe {
            slice::from_raw_parts(
                image.as_ptr().add(phdr.p_vaddr as usize) as *const Dyn,
                phdr.p_memsz as usize / size_of::<Dyn>(),
            )
        };

        for entry in dynamic {
            match entry.d_tag {
                DT_NULL => break,
                DT_RELA => rela_addr = entry.d_val as usize,
                DT_RELASZ => rela_size = entry.d_val as usize,
                _ => {}
            }
        }
    }

    // Apply relocations.
    if rela_addr != 0 && rela_size > 0 {
        if rela_size % size_of::<Rela>() != 0 {
            return Err(Error::BadRelocSize);
        }

        let relocations = unsafe {
            slice::from_raw_parts(
                image.as_ptr().add(rela_addr) as *const Rela,
                rela_size / size_of::<Rela>(),
            )
        };

        for rela in relocations {
            let target_off = rela.r_offset as usize;
            if rela.r_sym() != 0 {
                return Err(Error::BadRelocType);
            }

            #[cfg(target_arch = "x86_64")]
            if rela.r_type() != ftl_elf::R_X86_64_RELATIVE {
                return Err(Error::BadRelocType);
            }

            let target_end = match target_off.checked_add(size_of::<u64>()) {
                Some(end) if end <= image.len() => end,
                _ => return Err(Error::BadRelocOffset),
            };

            let base = image.as_ptr() as u64;
            let value = base.wrapping_add(rela.r_addend as u64);
            image[target_off..target_end].copy_from_slice(&value.to_le_bytes());
        }
    }

    let entry_fn = unsafe {
        let entry_ptr = image.as_ptr().add(elf.ehdr.e_entry as usize);
        core::mem::transmute::<*const u8, extern "Rust" fn(start_info: *const StartInfo)>(entry_ptr)
    };

    Ok(LoadedElf {
        image: image.as_ptr(),
        entry_fn,
    })
}
