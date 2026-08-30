use core::fmt;
use core::sync::atomic::AtomicU8;
use core::sync::atomic::Ordering;

use ftl_driver::dma::DmaBuf;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_utils::alignment::align_up;
use ftl_utils::spinlock::SpinLock;

use crate::arch;
use crate::arch::paddr2vaddr;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;
use crate::net::device::Device;
use crate::net::device::PollNotifier;
use crate::net::router::Router;
use crate::shared_ref::SharedRef;

mod arp;
mod device;
mod network;
mod packet;
mod route_table;
mod router;

pub use network::sys_net_bind;
pub use network::sys_net_create;
pub use network::sys_net_drop;
pub use network::sys_net_peek;
pub use network::sys_net_recv;
pub use network::sys_net_send;
pub use network::sys_net_subscribe;
pub use network::sys_net_unbind;

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

pub(super) static GLOBAL_ROUTER: SpinLock<Option<Router>> = SpinLock::new(None);
pub(super) static NET_IRQ: AtomicU8 = AtomicU8::new(0);

pub fn is_irq(irq: u8) -> bool {
    irq == NET_IRQ.load(Ordering::Relaxed)
}

pub fn handle_interrupt() {
    let router = GLOBAL_ROUTER.lock();
    if let Some(router) = router.as_ref() {
        router.handle_interrupt();
    }
}

// FIXME: Move this into virtio_net?
fn virtio_net_init() -> (SharedRef<Device>, u8) {
    use ftl_driver::pci::find_virtio_device;
    use ftl_driver::pci::get_interrupt_line;

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
    let pci_device = find_virtio_device(&GLOBAL_ENV, 1).expect("virtio-net disappeared");
    let irq = get_interrupt_line(&GLOBAL_ENV, &pci_device);

    (device, irq)
}
pub fn init() {
    let (device, irq) = virtio_net_init();
    *GLOBAL_ROUTER.lock() = Some(Router::new(device));
    arch::interrupt_acquire(irq).expect("failed to enable virtio-net IRQ");
    NET_IRQ.store(irq, Ordering::Relaxed);
    info!("net: listening for virtio-net IRQ {}", irq);
}
