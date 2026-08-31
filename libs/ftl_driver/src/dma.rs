pub struct DmaBuf {
    vaddr: usize,
    paddr: usize,
    len: usize,
}

impl DmaBuf {
    pub unsafe fn new(vaddr: usize, paddr: usize, len: usize) -> Self {
        Self { vaddr, paddr, len }
    }

    pub fn paddr(&self) -> usize {
        self.paddr
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
