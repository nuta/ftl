//! Application loader.
use core::slice;

use ftl_utils::alignment::align_up;

use crate::address::VAddr;
use crate::arch::MIN_PAGE_SIZE;
use crate::arch::{self};
use crate::initfs;
use crate::initfs::InitFs;
use crate::memory::PAGE_ALLOCATOR;

#[repr(C)]
struct Ehdr64 {
    magic: [u8; 16],
    type_: u16,
    machine: u16,
    version: u32,
    entry: u64,
    phoff: u64,
    shoff: u64,
    flags: u32,
    ehsize: u16,
    phentsize: u16,
    phnum: u16,
    shentsize: u16,
    shnum: u16,
    shstrndx: u16,
}

#[repr(C)]
struct Phdr64 {
    type_: u32,
    flags: u32,
    offset: u64,
    vaddr: u64,
    paddr: u64,
    filesz: u64,
    memsz: u64,
    align: u64,
}

#[derive(Debug)]
pub enum ElfError {
    NotAnElfFile,
}

/// Loads an ELF file into memory.
///
/// Returns the entry point of the ELF file.
fn load_elf(file: &initfs::File) -> Result<VAddr, ElfError> {
    let ehdr = unsafe { &*(file.data.as_ptr() as *const Ehdr64) };
    if ehdr.magic[..4] != [0x7f, b'E', b'L', b'F'] {
        return Err(ElfError::NotAnElfFile);
    }

    println!("{}: ELF entry={:x}", file.name, ehdr.entry);
    let phdrs = unsafe {
        let ptr = file.data.as_ptr().add(ehdr.phoff as usize) as *const Phdr64;
        core::slice::from_raw_parts(ptr, ehdr.phnum as usize)
    };

    // Calculate the size of the image.
    let mut image_size = 0;
    for phdr in phdrs {
        image_size = image_size.max(phdr.vaddr + phdr.memsz);
    }

    // Allocate memory for the image.
    println!("allocating {} bytes for the image", image_size);
    let image_paddr = PAGE_ALLOCATOR
        .alloc(align_up(image_size as usize, MIN_PAGE_SIZE))
        .expect("failed to allocate memory for the image");
    let image_vaddr = arch::paddr2vaddr(image_paddr);
    let image = unsafe {
        slice::from_raw_parts_mut(image_vaddr.as_usize() as *mut u8, image_size as usize)
    };

    // Copy the image into the allocated memory.
    let elf_file = unsafe { slice::from_raw_parts(file.data.as_ptr(), file.data.len()) };
    for phdr in phdrs {
        let src_range = phdr.offset as usize..phdr.offset as usize + phdr.filesz as usize;
        let dst_range = phdr.vaddr as usize..phdr.vaddr as usize + phdr.filesz as usize;
        image[dst_range].copy_from_slice(&elf_file[src_range]);
    }

    println!("image loaded at {:?}", image_vaddr);
    let entry = VAddr::new(image_vaddr.as_usize() + ehdr.entry as usize);
    Ok(entry)
}

pub fn load(initfs: &InitFs) {
    for file in initfs.iter() {
        let entry = load_elf(&file).expect("failed to load ELF file");
        println!("{}: ELF file loaded at {:?}", file.name, entry);
    }
}
