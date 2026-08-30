use core::fmt;

use ftl_driver::dma::DmaBuf;
use ftl_utils::alignment::align_up;

use crate::arch::paddr2vaddr;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;

mod arp;
mod device;
mod network;
mod packet;
mod router;

struct EnvImpl {}

impl EnvImpl {
    pub const fn new() -> Self {
        Self {}
    }
}

static GLOBAL_ENV: EnvImpl = EnvImpl::new();

impl ftl_driver::env::Env for EnvImpl {
    fn alloc_dma(&self, size: usize) -> Result<DmaBuf, ftl_driver::env::OutOfMemoryError> {
        let alloc_len = align_up(size.max(1), crate::arch::MIN_PAGE_SIZE);
        let paddr = PAGE_ALLOCATOR
            .alloc(alloc_len, PageType::Zeroed)
            .ok_or(ftl_driver::env::OutOfMemoryError)?;
        let vaddr = paddr2vaddr(paddr);

        Ok(unsafe { DmaBuf::new(vaddr.as_usize(), paddr.as_usize(), size) })
    }

    fn free_dma(&self, _buf: DmaBuf) {
        // TODO:
    }

    fn print(&self, args: fmt::Arguments<'_>) {
        // TODO: better logging
        info!("{}", args)
    }
}

pub use network::handle_interrupt;
pub use network::init;
pub use network::is_irq;
pub use network::sys_net_acquire;
pub use network::sys_net_peek;
pub use network::sys_net_recv;
pub use network::sys_net_send;
