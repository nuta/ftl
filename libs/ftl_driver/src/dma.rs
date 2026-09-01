pub struct DmaBuf {
    vaddr: usize,
    paddr: usize,
    capacity: usize,
    len: usize,
}

impl DmaBuf {
    /// # Safety
    ///
    /// - `vaddr` and `paddr` must be valid. This object owns the space.
    /// - `capacity >= len`.
    pub unsafe fn new(vaddr: usize, paddr: usize, capacity: usize, len: usize) -> Self {
        Self {
            vaddr,
            paddr,
            capacity,
            len,
        }
    }

    /// # Safety
    ///
    /// `len <= self.capacity()`
    pub unsafe fn set_len(&mut self, len: usize) {
        debug_assert!(len <= self.capacity);
        self.len = len;
    }

    pub fn paddr(&self) -> usize {
        self.paddr
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.vaddr as *const u8, self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.vaddr as *mut u8, self.len) }
    }
}
