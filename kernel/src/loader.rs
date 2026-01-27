//! Application loader.
use crate::initfs;
use crate::initfs::InitFs;

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

fn load_file(file: initfs::File) {
    let ehdr = unsafe { &*(file.data.as_ptr() as *const Ehdr64) };
    if ehdr.magic[..4] != [0x7f, b'E', b'L', b'F'] {
        // Not an ELF file. Ignore it.
        println!("{}: not an ELF file", file.name);
        return;
    }

    println!("{}: ELF entry={:x}", file.name, ehdr.entry);
    let phdrs = unsafe {
        let ptr = (file.data.as_ptr().add(ehdr.phoff as usize) as *const Phdr64);
        core::slice::from_raw_parts(ptr, ehdr.phnum as usize)
    };

    for phdr in phdrs {
        println!("{}: PHDR type={:x}, flags={:x}, offset={:x}, vaddr={:x}, paddr={:x}, filesz={:x}, memsz={:x}, align={:x}",
            file.name,
            phdr.type_,
            phdr.flags,
            phdr.offset,
            phdr.vaddr,
            phdr.paddr,
        );
}

pub fn load(initfs: &InitFs) {
    for file in initfs.iter() {
        load_file(file);
    }
}
