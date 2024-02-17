use core::marker::PhantomData;

use crate::folio::{self, Folio};

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
        let vaddr = folio.vaddr().as_usize() + self.offset;
        unsafe { core::ptr::read_volatile(vaddr as *const T) }
    }

    pub fn write(&self, folio: &mut Folio, value: T) {
        let vaddr = folio.vaddr().as_usize() + self.offset;
        unsafe { core::ptr::write_volatile(vaddr as *mut T, value) }
    }
}
