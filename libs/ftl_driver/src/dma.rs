use core::mem;
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::ops::DerefMut;

use crate::env::Env;

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

/// Frees the DMA buffer when dropped.
pub struct DmaBufWithDrop<'a> {
    env: &'a dyn Env,
    buf: ManuallyDrop<DmaBuf>,
}

impl<'a> DmaBufWithDrop<'a> {
    pub fn new(env: &'a dyn Env, buf: DmaBuf) -> Self {
        Self {
            env,
            buf: ManuallyDrop::new(buf),
        }
    }

    pub fn take(mut self) -> DmaBuf {
        // SAFETY: mem::forget skips Drop::drop.
        let buf = unsafe { ManuallyDrop::take(&mut self.buf) };
        mem::forget(self);
        buf
    }
}

impl Deref for DmaBufWithDrop<'_> {
    type Target = DmaBuf;

    fn deref(&self) -> &Self::Target {
        &self.buf
    }
}

impl DerefMut for DmaBufWithDrop<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buf
    }
}

impl Drop for DmaBufWithDrop<'_> {
    fn drop(&mut self) {
        // SAFETY: This is called only when Self::take is not called.
        let buf = unsafe { ManuallyDrop::take(&mut self.buf) };
        self.env.free_dma(buf);
    }
}
