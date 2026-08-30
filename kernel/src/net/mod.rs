use core::fmt;
use core::mem::size_of;
use core::slice;
use core::sync::atomic::AtomicU8;
use core::sync::atomic::Ordering;

use ftl_driver::dma::DmaBuf;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::net::NetRxInfo;
use ftl_types::thread::SyscallRegs;
use ftl_utils::alignment::align_up;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::address::USlice;
use crate::arch::paddr2vaddr;
use crate::handle::Handle;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;
use crate::net::device::Device;
use crate::net::device::PollNotifier;
use crate::net::network::Router;
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

use network::Network;

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

pub fn sys_net_create(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let poll_id = HandleId::new(ctx.a0);
    let handle_table = current.isolate().handles();
    let poll = handle_table
        .lock()
        .get::<Poll>(poll_id, HandleRight::WRITE)?;

    let device = GLOBAL_ROUTER
        .lock()
        .as_ref()
        .ok_or(ErrorCode::INVALID_STATE)?
        .device();
    let network = SharedRef::new(network::Network::new(device))?;
    let handle = Handle::new(network.clone(), HandleRight::READ | HandleRight::WRITE);
    let handle_id = handle_table.lock().insert(handle)?;

    GLOBAL_ROUTER
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
        .get::<Network>(network_id, HandleRight::WRITE)?;
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
        .get::<Network>(network_id, HandleRight::READ)?;

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
        .get::<Network>(network_id, HandleRight::READ)?;

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
        .get::<Network>(network_id, HandleRight::WRITE)?;

    network.send(header, payload)?;
    Ok(SyscallOutput::Done(0))
}

pub(super) static GLOBAL_ROUTER: SpinLock<Option<Router>> = SpinLock::new(None);
pub(super) static NET_IRQ: AtomicU8 = AtomicU8::new(0);

pub fn is_irq(irq: u8) -> bool {
    irq == NET_IRQ.load(Ordering::Relaxed)
}

pub fn handle_interrupt() {
    let router = GLOBAL_ROUTER.lock();
    let router = router.as_ref().expect("network router is not initialized");
    router.handle_interrupt();
}

pub fn init() {
    const RX_BUFFER_SIZE: usize = 2048;
    const RX_BUFFER_COUNT: usize = 64;

    let driver = virtio_net::VirtioNet::<PollNotifier>::init(&GLOBAL_ENV)
        .expect("failed to initialize virtio-net");
    let driver = SharedRef::new(driver).expect("failed to allocate virtio-net driver");
    let driver: SharedRef<dyn Driver<Notifier = PollNotifier>> = driver;

    for _ in 0..RX_BUFFER_COUNT {
        let buf = GLOBAL_ENV
            .alloc_dma(RX_BUFFER_SIZE)
            .expect("failed to allocate virtio-net RX buffer");
        if driver.provide(&GLOBAL_ENV, buf).is_err() {
            panic!("failed to supply virtio-net RX buffer");
        }
    }

    let device = Device::new(driver);
    let device = SharedRef::new(device).expect("failed to allocate network device");
    let pci_device =
        ftl_driver::pci::find_virtio_device(&GLOBAL_ENV, 1).expect("virtio-net disappeared");
    let irq = ftl_driver::pci::get_interrupt_line(&GLOBAL_ENV, &pci_device);

    *GLOBAL_ROUTER.lock() = Some(Router::new(device));
    crate::arch::interrupt_acquire(irq).expect("failed to enable virtio-net IRQ");
    NET_IRQ.store(irq, Ordering::Relaxed);
    info!("net: listening for virtio-net IRQ {}", irq);
}
