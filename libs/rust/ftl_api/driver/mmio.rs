use core::marker::PhantomData;

pub trait Endianess {
    fn into_host_u16(&self, n: u16) -> u16;
    fn into_host_u32(&self, n: u32) -> u32;
    fn into_host_u64(&self, n: u64) -> u64;
    fn from_host_u16(&self, n: u16) -> u16;
    fn from_host_u32(&self, n: u32) -> u32;
    fn from_host_u64(&self, n: u64) -> u64;
}

pub struct LittleEndian;

impl Endianess for LittleEndian {
    fn into_host_u16(&self, n: u16) -> u16 {
        u16::from_le(n)
    }
    fn into_host_u32(&self, n: u32) -> u32 {
        u32::from_le(n)
    }
    fn into_host_u64(&self, n: u64) -> u64 {
        u64::from_le(n)
    }
    fn from_host_u16(&self, n: u16) -> u16 {
        u16::to_le(n)
    }
    fn from_host_u32(&self, n: u32) -> u32 {
        u32::to_le(n)
    }
    fn from_host_u64(&self, n: u64) -> u64 {
        u64::to_le(n)
    }
}

pub struct Folio;

pub trait Access {}
pub struct ReadOnly;
pub struct WriteOnly;
pub struct ReadWrite;

impl Access for ReadOnly {}
impl Access for WriteOnly {}
impl Access for ReadWrite {}

pub struct MmioReg<E: Endianess, A: Access, T: Copy> {
    offset: usize,
    _pd1: PhantomData<E>,
    _pd2: PhantomData<A>,
    _pd3: PhantomData<T>,
}

impl<E: Endianess, A: Access, T: Copy> MmioReg<E, A, T> {
    pub const fn new(offset: usize) -> MmioReg<E, A, T> {
        MmioReg {
            offset,
            _pd1: PhantomData,
            _pd2: PhantomData,
            _pd3: PhantomData,
        }
    }

    /// Reads a value from the MMIO region.
    ///
    /// # Why is `&mut Folio` required?
    ///
    /// This is to ensure that the caller has exclusive access to the MMIO
    /// region. This is important because reads from MMIO may have side effects
    /// (e.g. clearing an interrupt) and concurrent access to the same MMIO
    /// region might lead to unexpected behavior.
    ///
    /// TODO: What about memory ordering?
    fn do_read(&self, folio: &mut Folio) -> T {
        todo!()
    }

    fn do_write(&self, folio: &mut Folio, value: T) {
        todo!()
    }
}

impl <E: Endianess, T: Copy> MmioReg<E, ReadOnly, T> {
    pub fn read(&self, folio: &mut Folio) -> T {
        self.do_read(folio)
    }
}

impl <E: Endianess, T: Copy> MmioReg<E, WriteOnly, T> {
    pub fn write(&self, folio: &mut Folio, value: T) {
        self.do_write(folio, value)
    }
}

impl <E: Endianess, T: Copy> MmioReg<E, ReadWrite, T> {
    pub fn read(&self, folio: &mut Folio) -> T {
        self.do_read(folio)
    }

    pub fn write(&self, folio: &mut Folio, value: T) {
        self.do_write(folio, value)
    }
}
