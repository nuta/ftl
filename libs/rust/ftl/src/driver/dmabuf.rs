use alloc::vec::Vec;
use core::slice;

use ftl_types::error::ErrorCode;
use ftl_types::syscall::SYS_DMABUF_ALLOC;
use ftl_utils::alignment::is_aligned;

use crate::arch::min_page_size;
use crate::syscall::syscall3;

pub struct DmaBuf {
    vaddr: usize,
    paddr: usize,
    len: usize,
}

impl DmaBuf {
    pub fn alloc(size: usize) -> Result<DmaBuf, ErrorCode> {
        let mut vaddr = 0;
        let mut paddr = 0;
        sys_dmabuf_alloc(size, &mut vaddr, &mut paddr)?;
        Ok(DmaBuf {
            vaddr,
            paddr,
            len: size,
        })
    }

    pub fn vaddr(&self) -> usize {
        self.vaddr
    }

    pub fn paddr(&self) -> usize {
        self.paddr
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.vaddr as *const u8, self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.vaddr as *mut u8, self.len) }
    }
}

impl Drop for DmaBuf {
    fn drop(&mut self) {
        todo!();
    }
}

pub struct DmaBufPool {
    size: usize,
    pool: Vec<DmaBuf>,
}

impl DmaBufPool {
    pub fn new(size: usize) -> Self {
        debug_assert!(
            is_aligned(size, min_page_size()),
            "{size} is not aligned to the minimum page size"
        );

        Self {
            size,
            pool: Vec::new(),
        }
    }

    pub fn alloc(&mut self) -> Result<DmaBuf, ErrorCode> {
        if let Some(dmabuf) = self.pool.pop() {
            return Ok(dmabuf);
        }

        DmaBuf::alloc(self.size)
    }

    pub fn free(&mut self, dmabuf: DmaBuf) {
        self.pool.push(dmabuf);
    }
}

pub fn sys_dmabuf_alloc(
    size: usize,
    vaddr: &mut usize,
    paddr: &mut usize,
) -> Result<(), ErrorCode> {
    syscall3(
        SYS_DMABUF_ALLOC,
        size,
        vaddr as *mut usize as usize,
        paddr as *mut usize as usize,
    )?;
    Ok(())
}
