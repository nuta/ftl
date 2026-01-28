use ftl_types::error::ErrorCode;
use ftl_types::syscall::SYS_DMABUF_ALLOC;

use crate::syscall::syscall3;

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
