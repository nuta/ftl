//! Application loader.
use core::mem::MaybeUninit;
use core::mem::size_of;
use core::slice;

use ftl_types::environ::StartInfo;
use ftl_utils::alignment::align_up;

use crate::address::VAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::initfs;
use crate::initfs::InitFs;
use crate::isolation::INKERNEL_ISOLATION;
use crate::memory::PAGE_ALLOCATOR;
use crate::process::Process;
use crate::scheduler::SCHEDULER;
use crate::thread::Thread;

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

const PT_LOAD: u32 = 1;

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
    // TODO: Is data guaranteed to be aligned?
    assert!(
        file.data.len() >= size_of::<Ehdr64>(),
        "ELF file too small: {}",
        file.name
    );
    let ehdr = unsafe { &*(file.data.as_ptr() as *const Ehdr64) };
    if ehdr.magic[..4] != [0x7f, b'E', b'L', b'F'] {
        return Err(ElfError::NotAnElfFile);
    }

    // TODO: More checks: file type, bound checking, etc.

    let phentsize = ehdr.phentsize as usize;
    assert_eq!(phentsize, size_of::<Phdr64>());

    let phnum = ehdr.phnum as usize;
    let phoff = ehdr.phoff as usize;
    let phdr_end = phoff + phentsize * phnum;
    assert!(phdr_end <= file.data.len());

    let phdrs = unsafe {
        let ptr = file.data.as_ptr().add(phoff) as *const Phdr64;
        core::slice::from_raw_parts(ptr, ehdr.phnum as usize)
    };

    // Calculate the size of the image.
    let mut image_size = 0;
    for phdr in phdrs {
        if phdr.type_ != PT_LOAD {
            continue;
        }
        image_size = image_size.max(phdr.vaddr + phdr.memsz);
    }

    // Allocate memory for the image.
    println!(
        "{}: Loading an ELF file in initfs: entry={:x}, image_size={}",
        file.name, ehdr.entry, image_size
    );
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
        if phdr.type_ != PT_LOAD {
            continue;
        }
        assert!(phdr.filesz <= phdr.memsz);
        let src_start = phdr.offset as usize;
        let dst_start = phdr.vaddr as usize;
        let src_end = src_start + phdr.filesz as usize;
        let dst_end = dst_start + phdr.filesz as usize;

        println!(
            "{}: phdr: vaddr={:x}, filesz={:x}, memsz={:x}",
            file.name, phdr.vaddr, phdr.filesz, phdr.memsz
        );
        let src_range = src_start..src_end;
        let dst_range = dst_start..dst_end;

        if !src_range.is_empty() {
            image[dst_range].copy_from_slice(&elf_file[src_range]);
        }

        // Clear the .bss section (filesz < range < memsz).
        let zeroed_range = dst_end..(dst_start + phdr.memsz as usize);
        if phdr.filesz < phdr.memsz {
            image[zeroed_range].fill(0);
        }
    }

    let entry = VAddr::new(image_vaddr.as_usize() + ehdr.entry as usize);
    Ok(entry)
}

pub fn load(initfs: &InitFs) {
    for file in initfs.iter() {
        let entry = load_elf(&file).expect("failed to load ELF file");

        let stack_size = 1024 * 1024;
        let stack_bottom_paddr = PAGE_ALLOCATOR
            .alloc(align_up(stack_size, MIN_PAGE_SIZE))
            .expect("failed to allocate stack");
        let stack_bottom_vaddr = arch::paddr2vaddr(stack_bottom_paddr);
        let sp = stack_bottom_vaddr.as_usize() + stack_size;

        use alloc::boxed::Box;

        let info_uninit = Box::leak(Box::new(MaybeUninit::<StartInfo>::uninit()));
        info_uninit.write(StartInfo {
            syscall: arch::direct_syscall_handler,
            min_page_size: arch::MIN_PAGE_SIZE,
        });
        let start_info = info_uninit.as_ptr() as usize;

        let process = Process::new(INKERNEL_ISOLATION.clone()).expect("failed to create process");
        let thread = Thread::new(process, entry.as_usize(), sp, start_info)
            .expect("failed to create thread");
        SCHEDULER.push(thread);
    }
}
