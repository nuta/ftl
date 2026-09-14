use alloc::collections::VecDeque;
use core::fmt;
use core::sync::atomic::AtomicU8;
use core::sync::atomic::Ordering;

use ftl_driver::dma::DmaBuf;
use ftl_driver::net::Driver;
use ftl_netmux::DeviceId;
use ftl_netmux::PollNotifier;
use ftl_utils::alignment::align_up;
use ftl_utils::spinlock::SpinLock;
use virtio_net::VirtioNet;

use crate::address::PAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::arch::paddr2vaddr;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;
use crate::net::NET_MUX;

static DMA_FREE_LIST: SpinLock<VecDeque<DmaBuf>> = SpinLock::new(VecDeque::new());
const DMA_FREE_LIST_MAX: usize = 16;

pub static DRIVER_ENV: DriverEnv = DriverEnv::new();

// An environment for device drivers.
pub struct DriverEnv {
    _private: (),
}

impl DriverEnv {
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl ftl_driver::env::Env for DriverEnv {
    fn alloc_dma(&self, len: usize) -> Result<DmaBuf, ftl_driver::env::OutOfMemoryError> {
        let len = len.max(1);

        // Try reusing a buffer from the free list.
        {
            let mut free_list = DMA_FREE_LIST.lock();
            if let Some(buf) = free_list.back() {
                if buf.capacity() >= len {
                    let mut buf = free_list.pop_back().unwrap();

                    // SAFETY: We've checked the capacity is sufficient.
                    unsafe {
                        buf.set_len(len);
                    }
                    return Ok(buf);
                }
            }
        }

        // Allocate a new buffer.
        let capacity = align_up(len, MIN_PAGE_SIZE);
        let paddr = PAGE_ALLOCATOR
            .alloc(capacity, PageType::Zeroed)
            .ok_or(ftl_driver::env::OutOfMemoryError)?;
        let vaddr = paddr2vaddr(paddr).as_usize();

        // SAFETY: paddr/vaddr are valid, and capacity >= len.
        let buf = unsafe { DmaBuf::new(vaddr, paddr.as_usize(), capacity, len) };
        Ok(buf)
    }

    fn free_dma(&self, buf: DmaBuf) {
        let mut free_list = DMA_FREE_LIST.lock();
        if free_list.len() >= DMA_FREE_LIST_MAX {
            if let Some(buf) = free_list.pop_front() {
                let paddr = PAddr::new(buf.paddr());
                // SAFETY: This page is allocated by global PAGE_ALLOCATOR, and
                //         capacity is unchanged.
                unsafe {
                    PAGE_ALLOCATOR.free(paddr, buf.capacity());
                }
            }
        }

        // Try to reserve a space in the free list. If it fails, free it
        // immediately.
        if free_list.try_reserve(1).is_err() {
            // SAFETY: The buffer is allocated by global PAGE_ALLOCATOR, and
            //         capacity is unchanged.
            unsafe { PAGE_ALLOCATOR.free(PAddr::new(buf.paddr()), buf.capacity()) };
            return;
        }

        free_list.push_back(buf);
    }

    fn print(&self, args: fmt::Arguments<'_>) {
        // TODO: better logging
        println!("{}", args)
    }
}

static VIRTIO_NET_DRIVER: SpinLock<Option<VirtioNet<PollNotifier>>> = SpinLock::new(None);
static VIRTIO_NET_DEVICE_ID: SpinLock<Option<DeviceId>> = SpinLock::new(None);
static NET_IRQ: AtomicU8 = AtomicU8::new(0);

pub fn is_irq(irq: u8) -> bool {
    irq == NET_IRQ.load(Ordering::Relaxed)
}

pub fn net_device_id() -> DeviceId {
    VIRTIO_NET_DEVICE_ID
        .lock()
        .expect("virtio-net is not initialized")
}

pub fn handle_interrupt() {
    let mut net = NET_MUX.lock();
    let device_id = VIRTIO_NET_DEVICE_ID.lock().unwrap();
    net.handle_interrupt(device_id);
}

// FIXME: Move this into virtio_net?
fn virtio_net_init() -> (&'static dyn Driver<Notifier = PollNotifier>, u8) {
    use ftl_driver::pci::find_virtio_device;
    use ftl_driver::pci::get_interrupt_line;

    let driver = virtio_net::VirtioNet::<PollNotifier>::init(&DRIVER_ENV)
        .expect("failed to initialize virtio-net");
    *VIRTIO_NET_DRIVER.lock() = Some(driver);

    let driver: &'static dyn Driver<Notifier = PollNotifier> = {
        let guard = VIRTIO_NET_DRIVER.lock();
        let ptr: *const VirtioNet<PollNotifier> = guard.as_ref().unwrap();
        // SAFETY: The driver is never removed from VIRTIO_NET_DRIVER.
        unsafe { &*ptr }
    };

    let pci_device = find_virtio_device(&DRIVER_ENV, 1).expect("virtio-net disappeared");
    let irq = get_interrupt_line(&DRIVER_ENV, &pci_device);
    arch::interrupt_acquire(irq).expect("failed to enable virtio-net IRQ");

    (driver, irq)
}

pub fn init() {
    let (driver, irq) = virtio_net_init();
    let device_id = NET_MUX
        .lock()
        .add_device(driver)
        .expect("failed to add network device");

    *VIRTIO_NET_DEVICE_ID.lock() = Some(device_id);
    NET_IRQ.store(irq, Ordering::Relaxed);
}
