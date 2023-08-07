#![no_std]

use core::mem::size_of;

#[cfg(target_pointer_width = "64")]
pub type Addr = u64;
#[cfg(target_pointer_width = "64")]
pub type Off = u64;

#[cfg(target_pointer_width = "32")]
pub type Addr = u32;
#[cfg(target_pointer_width = "32")]
pub type Off = u32;

#[derive(Debug)]
#[repr(C)]
pub struct Ehdr {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: Addr,
    pub e_phoff: Off,
    pub e_shoff: Off,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(u32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhdrType {
    Null = 0,
    Load = 1,
}

#[derive(Debug)]
#[repr(C)]
pub struct Phdr64 {
    pub p_type: PhdrType,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

#[cfg(target_pointer_width = "64")]
pub type Phdr = Phdr64;

impl Phdr {
    pub fn readable(&self) -> bool {
        self.p_flags & 0x4 != 0
    }
    pub fn writable(&self) -> bool {
        self.p_flags & 0x2 != 0
    }
    pub fn executable(&self) -> bool {
        self.p_flags & 0x1 != 0
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    BufferTooShort,
    InvalidMagic,
}

#[derive(Debug)]
pub struct Elf<'a> {
    pub ehdr: &'a Ehdr,
    pub phdrs: &'a [Phdr],
}

impl<'a> Elf<'a> {
    pub fn parse(buf: &[u8]) -> Result<Elf<'a>, ParseError> {
        if buf.len() < size_of::<Ehdr>() {
            return Err(ParseError::BufferTooShort);
        }

        let ehdr = unsafe { &*(buf.as_ptr() as *const Ehdr) };
        if ehdr.e_ident[0..4] != [0x7f, b'E', b'L', b'F'] {
            return Err(ParseError::InvalidMagic);
        }

        let phdrs_size = ehdr.e_phnum as usize * size_of::<Phdr>();
        if buf.len() < ehdr.e_phoff as usize + phdrs_size {
            return Err(ParseError::BufferTooShort);
        }

        let phdrs = unsafe {
            core::slice::from_raw_parts(
                buf.as_ptr().add(ehdr.e_phoff as usize) as *const Phdr,
                ehdr.e_phnum as usize,
            )
        };

        Ok(Elf { ehdr, phdrs })
    }
}
