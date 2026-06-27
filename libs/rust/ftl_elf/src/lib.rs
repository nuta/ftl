#![no_std]

#[cfg(target_pointer_width = "64")]
pub type Addr = u64;
#[cfg(target_pointer_width = "64")]
pub type Off = u64;

#[cfg(target_pointer_width = "32")]
pub type Addr = u32;
#[cfg(target_pointer_width = "32")]
pub type Off = u32;

#[cfg(target_arch = "x86_64")]
const EM_NATIVE: u16 = 62;
#[cfg(target_arch = "aarch64")]
const EM_NATIVE: u16 = 183;

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

pub const ET_EXEC: u16 = 2;
pub const ET_DYN: u16 = 3;

#[repr(u32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhdrType {
    Null = 0,
    Load = 1,
    Dynamic = 2,
    Interp = 3,
    Note = 4,
    ShLib = 5,
    Phdr = 6,
    Tls = 7,
}

/// Executable segment.
pub const PF_X: u32 = 0x1;
/// Writable segment.
pub const PF_W: u32 = 0x2;
/// Readable segment.
pub const PF_R: u32 = 0x4;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Dyn64 {
    pub d_tag: i64,
    pub d_val: u64,
}

#[cfg(target_pointer_width = "64")]
pub type Dyn = Dyn64;

pub const DT_NULL: i64 = 0;
pub const DT_RELA: i64 = 7;
pub const DT_RELASZ: i64 = 8;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Rela64 {
    pub r_offset: u64,
    pub r_info: u64,
    pub r_addend: i64,
}

#[cfg(target_pointer_width = "64")]
pub type Rela = Rela64;

impl Rela {
    pub fn r_sym(&self) -> u32 {
        (self.r_info >> 32) as u32
    }

    pub fn r_type(&self) -> u32 {
        self.r_info as u32
    }
}

#[cfg(target_arch = "x86_64")]
pub const R_X86_64_RELATIVE: u32 = 8;

#[derive(Debug)]
#[repr(C)]
pub struct Phdr64 {
    pub p_type: u32,
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
    InvalidEtype,
    InvalidMachine,
    InvalidEhdrSize,
    InvalidPhdrSize,
}

#[derive(Debug)]
pub struct Elf<'a> {
    pub ehdr: &'a Ehdr,
    pub phdrs: &'a [Phdr],
}

impl<'a> Elf<'a> {
    pub fn parse(buf: &[u8], expected_type: u16) -> Result<Elf<'a>, ParseError> {
        if buf.len() < size_of::<Ehdr>() {
            return Err(ParseError::BufferTooShort);
        }

        let ehdr = unsafe { &*(buf.as_ptr() as *const Ehdr) };
        if ehdr.e_ident[0..4] != [0x7f, b'E', b'L', b'F'] {
            return Err(ParseError::InvalidMagic);
        }

        if ehdr.e_type != expected_type {
            return Err(ParseError::InvalidEtype);
        }

        if ehdr.e_machine != EM_NATIVE {
            return Err(ParseError::InvalidMachine);
        }

        if ehdr.e_ehsize as usize != size_of::<Ehdr>() {
            return Err(ParseError::InvalidEhdrSize);
        }

        if ehdr.e_phentsize as usize != size_of::<Phdr>() {
            return Err(ParseError::InvalidPhdrSize);
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
