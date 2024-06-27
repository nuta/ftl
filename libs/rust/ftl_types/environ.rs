use core::slice;

use crate::error::FtlError;

const NUM_PAGES_BITS: usize = 12;
const NUM_PAGES_BITMASK: usize = (1 << NUM_PAGES_BITS) - 1;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvironPtr(usize);

impl EnvironPtr {
    pub const fn new(addr: usize, num_pages: usize) -> Result<Self, FtlError> {
        if addr & NUM_PAGES_BITMASK != 0 {
            return Err(FtlError::InvalidArg);
        }

        if num_pages >= 1 << NUM_PAGES_BITS {
            return Err(FtlError::TooLarge);
        }

        Ok(EnvironPtr(addr | num_pages))
    }

    fn num_pages(&self) -> usize {
        self.0 & NUM_PAGES_BITMASK
    }

    fn len(&self) -> usize {
        self.num_pages() << NUM_PAGES_BITS
    }

    fn addr(&self) -> usize {
        self.0 & !NUM_PAGES_BITMASK
    }

    pub fn envion_as_bytes(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self.addr() as *const u8, self.len())
        }
    }
}
