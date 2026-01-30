use ftl_types::error::ErrorCode;
use ftl_types::syscall::SYS_DMABUF_ALLOC;

use crate::syscall::syscall3;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct DmaAddr(usize);

impl DmaAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_usize(self) -> usize {
        self.0
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

pub struct Pool {}

impl Pool {
    pub fn new() -> Result<Self, ErrorCode> {
        Ok(Self {})
    }

    pub fn alloc(&mut self, size: usize) -> Result<(DmaAddr, &mut [u8]), ErrorCode> {
        todo!()
    }

    pub fn get_by_daddr(&self, daddr: DmaAddr) -> Option<&mut [u8]> {
        todo!()
    }
}
