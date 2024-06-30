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

pub trait WritableReg<T: Copy> {
    fn write(&self, folio: &mut Folio, value: T) {
        todo!()
    }
}

pub trait ReadableReg<T: Copy> {
    /// Reads a value from the MMIO region.
    ///
    /// # Why is `&mut Folio` required?
    ///
    /// This is to ensure that the caller has exclusive access to the MMIO
    /// region. This is important because reads from MMIO may have side effects
    /// (e.g. clearing an interrupt) and concurrent access to the same MMIO
    /// region might lead to unexpected behavior.
    fn read(&self, folio: &mut Folio, value: T) {
        todo!()
    }
}

pub struct Folio;

pub trait Access {}

pub struct Read;
impl Access for Read {}

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
}

static A: MmioReg<LittleEndian, Read, u32> = MmioReg::new(0);

