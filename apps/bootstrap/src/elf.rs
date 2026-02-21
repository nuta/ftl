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
pub enum ElfError {
    NotAnElfFile,
    InvalidElf,
}

pub struct Elf<'a> {
    pub entry: usize,
    pub image_size: usize,
    pub phdrs: &'a [Phdr64],
}

impl<'a> Elf<'a> {
    pub fn parse(file: &[u8]) -> Result<Self, ElfError> {
        let ehdr = unsafe { &*(file.as_ptr() as *const Ehdr64) };
        if ehdr.magic[..4] != [0x7f, b'E', b'L', b'F'] {
            return Err(ElfError::NotAnElfFile);
        }

        if ehdr.phentsize as usize != size_of::<Phdr64>() {
            return Err(ElfError::InvalidElf);
        }

        let phentsize = ehdr.phentsize as usize;
        let phnum = ehdr.phnum as usize;
        let phoff = ehdr.phoff as usize;
        let phdr_end = phoff
            .checked_add(phentsize.checked_mul(phnum).ok_or(ElfError::InvalidElf)?)
            .ok_or(ElfError::InvalidElf)?;
        if phdr_end > file.len() {
            return Err(ElfError::InvalidElf);
        }

        let phdrs = unsafe {
            let ptr = file.as_ptr().add(phoff) as *const Phdr64;
            slice::from_raw_parts(ptr, phnum)
        };

        let mut image_size = 0usize;
        for phdr in phdrs.iter().filter(|phdr| phdr.type_ == PT_LOAD) {
            if phdr.filesz > phdr.memsz {
                return Err(ElfError::InvalidElf);
            }

            let src_start = phdr.offset as usize;
            let src_end = src_start
                .checked_add(phdr.filesz as usize)
                .ok_or(ElfError::InvalidElf)?;
            if src_end > file.len() {
                return Err(ElfError::InvalidElf);
            }

            let seg_end = (phdr.vaddr as usize)
                .checked_add(phdr.memsz as usize)
                .ok_or(ElfError::InvalidElf)?;
            image_size = image_size.max(seg_end);
        }

        if image_size == 0 {
            return Err(ElfError::InvalidElf);
        }

        Ok(Elf {
            entry: ehdr.entry as usize,
            image_size,
            phdrs,
        })
    }
}
