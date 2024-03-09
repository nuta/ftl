use core::marker::PhantomData;

use crate::folio::Folio;

pub struct ReadOnly<T: Copy> {
    offset: usize,
    _pd: PhantomData<T>,
}

impl<T: Copy> ReadOnly<T> {
    pub const fn new(offset: usize) -> ReadOnly<T> {
        ReadOnly {
            offset,
            _pd: PhantomData,
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
    pub fn read(&self, folio: &mut Folio) -> T {
        let vaddr = folio.vaddr().as_usize() + self.offset;
        unsafe { core::ptr::read_volatile(vaddr as *const T) }
    }
}

pub struct WriteOnly<T: Copy> {
    offset: usize,
    _pd: PhantomData<T>,
}

impl<T: Copy> WriteOnly<T> {
    pub const fn new(offset: usize) -> WriteOnly<T> {
        WriteOnly {
            offset,
            _pd: PhantomData,
        }
    }

    pub fn write(&self, folio: &mut Folio, value: T) {
        let vaddr = folio.vaddr().as_usize() + self.offset;
        unsafe { core::ptr::write_volatile(vaddr as *mut T, value) }
    }
}

pub struct ReadWrite<T: Copy> {
    offset: usize,
    _pd: PhantomData<T>,
}

impl<T: Copy> ReadWrite<T> {
    pub const fn new(offset: usize) -> ReadWrite<T> {
        ReadWrite {
            offset,
            _pd: PhantomData,
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
    pub fn read(&self, folio: &mut Folio) -> T {
        self.read_with_offset(folio, 0)
    }

    pub fn read_with_offset(&self, folio: &mut Folio, offset: usize) -> T {
        let vaddr = folio.vaddr().as_usize() + self.offset + offset;
        unsafe { core::ptr::read_volatile(vaddr as *const T) }
    }

    pub fn write(&self, folio: &mut Folio, value: T) {
        let vaddr = folio.vaddr().as_usize() + self.offset;
        unsafe { core::ptr::write_volatile(vaddr as *mut T, value) }
    }
}

pub struct LittleEndianReadOnly<T: Copy> {
    inner: ReadOnly<T>,
}

impl<T: Copy> LittleEndianReadOnly<T> {
    pub const fn new(offset: usize) -> LittleEndianReadOnly<T> {
        LittleEndianReadOnly {
            inner: ReadOnly::new(offset),
        }
    }
}

impl LittleEndianReadOnly<u32> {
    pub fn read_u32(&self, folio: &mut Folio) -> u32 {
        let value = self.inner.read(folio);
        u32::from_le(value)
    }
}

impl LittleEndianReadOnly<u64> {
    pub fn read_u64(&self, folio: &mut Folio) -> u64 {
        let value = self.inner.read(folio);
        u64::from_le(value)
    }
}

pub struct LittleEndianWriteOnly<T: Copy> {
    inner: WriteOnly<T>,
}

impl<T: Copy> LittleEndianWriteOnly<T> {
    pub const fn new(offset: usize) -> LittleEndianWriteOnly<T> {
        LittleEndianWriteOnly {
            inner: WriteOnly::new(offset),
        }
    }
}

impl LittleEndianWriteOnly<u32> {
    pub fn write_u32(&self, folio: &mut Folio, value: u32) {
        self.inner.write(folio, value.to_le());
    }
}

impl LittleEndianWriteOnly<u64> {
    pub fn write_u64(&self, folio: &mut Folio, value: u64) {
        self.inner.write(folio, value.to_le());
    }
}

pub struct LittleEndianReadWrite<T: Copy> {
    inner: ReadWrite<T>,
}

impl<T: Copy> LittleEndianReadWrite<T> {
    pub const fn new(offset: usize) -> LittleEndianReadWrite<T> {
        LittleEndianReadWrite {
            inner: ReadWrite::new(offset),
        }
    }
}

impl LittleEndianReadWrite<u32> {
    pub fn read_u32(&self, folio: &mut Folio) -> u32 {
        let value = self.inner.read(folio);
        u32::from_le(value)
    }

    pub fn write_u32(&self, folio: &mut Folio, value: u32) {
        self.inner.write(folio, value.to_le());
    }
}

impl LittleEndianReadWrite<u64> {
    pub fn read_u64(&self, folio: &mut Folio) -> u64 {
        let value = self.inner.read(folio);
        u64::from_le(value)
    }

    pub fn write_u64(&self, folio: &mut Folio, value: u64) {
        self.inner.write(folio, value.to_le());
    }
}
