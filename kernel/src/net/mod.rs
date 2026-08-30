use core::fmt;
use core::mem::size_of;
use core::slice;

use ftl_driver::dma::DmaBuf;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::net::NetRxInfo;
use ftl_types::thread::SyscallRegs;
use ftl_utils::alignment::align_up;

use crate::address::UAddr;
use crate::address::USlice;
use crate::arch::paddr2vaddr;
use crate::handle::Handle;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;
use crate::poll::EventEmitter;
use crate::poll::Poll;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

mod arp;
mod device;
mod network;
mod packet;
mod route;

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

pub fn sys_net_create(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let poll_id = HandleId::new(ctx.a0);
    let handle_table = current.isolate().handles();
    let poll = handle_table
        .lock()
        .get::<Poll>(poll_id, HandleRight::WRITE)?;

    let device = network::ROUTER
        .lock()
        .as_ref()
        .ok_or(ErrorCode::INVALID_STATE)?
        .device();
    let network = SharedRef::new(network::Network::new(device))?;
    let handle = Handle::new(network.clone(), HandleRight::READ | HandleRight::WRITE);
    let handle_id = handle_table.lock().insert(handle)?;

    network::ROUTER
        .lock()
        .as_mut()
        .ok_or(ErrorCode::INVALID_STATE)?
        .add_network(network.clone())?;
    network.set_emitter(EventEmitter::new(poll, handle_id));
    Ok(SyscallOutput::Done(handle_id.as_usize()))
}

pub fn sys_net_bind(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let local_port = ctx.a3;
    if local_port > u16::MAX as usize {
        return Err(ErrorCode::INVALID_ARG);
    }

    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<network::Network>(network_id, HandleRight::WRITE)?;
    network.add_rule(ctx.a1, ctx.a2 as u32, local_port as u16)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_net_peek(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let info_addr = UAddr::new(ctx.a1);
    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<network::Network>(network_id, HandleRight::READ)?;

    let (token, info) = network.peek()?;
    let info_ptr = &raw const info;
    let info_len = size_of::<NetRxInfo>();
    let info_bytes = unsafe { slice::from_raw_parts(info_ptr.cast::<u8>(), info_len) };
    USlice::new(info_addr, info_len)?.write_bytes(info_bytes)?;
    Ok(SyscallOutput::Done(token))
}

pub fn sys_net_recv(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let token = ctx.a1;
    let payload = USlice::new(UAddr::new(ctx.a2), ctx.a3)?;
    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<network::Network>(network_id, HandleRight::READ)?;

    network.recv(token, payload)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_net_send(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let header = USlice::new(UAddr::new(ctx.a2), ctx.a3)?;
    let payload = USlice::new(UAddr::new(ctx.a4), ctx.a5)?;
    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<network::Network>(network_id, HandleRight::WRITE)?;

    network.send(header, payload)?;
    Ok(SyscallOutput::Done(0))
}
