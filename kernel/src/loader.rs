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

fn load_file(file: initfs::File) {
    let ehdr = unsafe { &*(file.data.as_ptr() as *const Ehdr64) };
    if ehdr.magic[..4] != [0x7f, b'E', b'L', b'F'] {
        // Not an ELF file. Ignore it.
        println!("{}: not an ELF file", file.name);
        return;
    }

    println!("{}: ELF entry={:x}", file.name, ehdr.entry);
}

pub fn load(initfs: &InitFs) {
    for file in initfs.iter() {
        load_file(file);
    }
}
