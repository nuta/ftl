use core::slice;

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
pub struct Phdr64 {
    pub type_: u32,
    pub flags: u32,
    pub offset: u64,
    pub vaddr: u64,
    pub paddr: u64,
    pub filesz: u64,
    pub memsz: u64,
    pub align: u64,
}

const PT_LOAD: u32 = 1;

impl Phdr64 {
    pub fn is_load(&self) -> bool {
        self.type_ == PT_LOAD
    }
}

#[derive(Debug)]
pub enum Error {
    TooShort,
    InvalidMagic,
}

pub struct Elf {
    entry: usize,
    phdrs_offset: usize,
    phdrs_size: usize,
}

impl Elf {
    pub fn new(input: &[u8]) -> Result<Self, Error> {
        let ehdr = unsafe { &*(input.as_ptr() as *const Ehdr64) };
        if ehdr.magic[..4] != [0x7f, b'E', b'L', b'F'] {
            return Err(Error::InvalidMagic);
        }

        Ok(Self {
            entry: ehdr.entry as usize,
            phdrs_offset: ehdr.phoff as usize,
            phdrs_size: ehdr.phnum as usize * size_of::<Phdr64>(),
        })
    }

    pub fn parse_program_headers(&self, input: &[u8]) -> Result<&[Phdr64], Error> {
        if input.len() < self.phdrs_size {
            return Err(Error::TooShort);
        }

        let phdrs = unsafe {
            let ptr = &*(input.as_ptr() as *const Phdr64);
            slice::from_raw_parts(ptr, self.phdrs_size)
        };

        Ok(phdrs)
    }
}
