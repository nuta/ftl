use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::fmt;
use core::str;

use ftl_arrayvec::ArrayVec;
use ftl_driver::dma::DmaBuf;
use ftl_driver::net::Driver;
use ftl_netmux::PollNotifier;
use ftl_utils::alignment::align_up;
use ftl_utils::cmdline::Parser;
use ftl_utils::reserve_slot::ReserveSlot;
use ftl_utils::spinlock::SpinLock;
use ftl_virtio::VirtioMmio;
#[cfg(target_arch = "x86_64")]
use ftl_virtio::VirtioPci;
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
        match free_list.reserve_slot() {
            Ok(slot) => slot.push_back(buf),
            Err(_) => {
                // SAFETY: The buffer is allocated by global PAGE_ALLOCATOR, and
                //         capacity is unchanged.
                unsafe { PAGE_ALLOCATOR.free(PAddr::new(buf.paddr()), buf.capacity()) };
            }
        }
    }

    fn print(&self, args: fmt::Arguments<'_>) {
        // TODO: better logging
        println!("{}", args)
    }
}

pub fn poll(irq: u8) {
    let mut net = NET_MUX.lock();
    net.poll(irq);
}

#[cfg(target_arch = "x86_64")]
fn init_virtio_net_over_pci(
    pci_device: &ftl_driver::pci::PciDevice,
) -> (Box<dyn Driver<Notifier = PollNotifier>>, u8) {
    use ftl_driver::pci::get_bar;
    use ftl_driver::pci::get_interrupt_line;
    use ftl_driver::pci::set_bus_master;

    set_bus_master(&DRIVER_ENV, &pci_device, true);

    let bar0 = get_bar(&DRIVER_ENV, &pci_device, 0);
    if bar0 & 1 == 0 {
        panic!("virtio-net BAR0 is not I/O space");
    }
    let iobase = (bar0 & 0xffff_fffc) as u16;
    trace!("PCI BAR0: iobase={iobase:#x}");

    let transport = VirtioPci::new(iobase);
    let driver = VirtioNet::<VirtioPci, PollNotifier>::init(&DRIVER_ENV, transport)
        .expect("failed to initialize virtio-net");
    let irq = get_interrupt_line(&DRIVER_ENV, &pci_device);

    (Box::new(driver), irq)
}

fn init_virtio_net_over_mmio(
    base: PAddr,
    size: usize,
    irq: u8,
) -> (Box<dyn Driver<Notifier = PollNotifier>>, u8) {
    assert!(size >= 0x200, "virtio-mmio region too small: {size:#x}");

    let mmio_base = paddr2vaddr(base).as_usize();
    let transport = unsafe { VirtioMmio::new(mmio_base) }.expect("failed to probe virtio-mmio");
    let driver = VirtioNet::<VirtioMmio, PollNotifier>::init(&DRIVER_ENV, transport)
        .expect("failed to initialize virtio-net");

    (Box::new(driver), irq)
}

enum FoundDevice {
    #[cfg(target_arch = "x86_64")]
    VirtioNetOverPci(ftl_driver::pci::PciDevice),
    VirtioMmio {
        base: PAddr,
        size: usize,
        irq: u8,
    },
}

#[cfg(target_arch = "x86_64")]
fn probe_pci(devices: &mut ArrayVec<FoundDevice, 8>) {
    use ftl_driver::pci::find_virtio_device;
    use ftl_virtio::virtio_pci::DeviceType;

    let Some(pci_device) = find_virtio_device(&DRIVER_ENV, DeviceType::Network as u16) else {
        return;
    };

    trace!(
        "found at {:02x}:{:02x} (device_id={:#x}, subsystem={})",
        pci_device.bus, pci_device.slot, pci_device.device, pci_device.subsystem_id
    );

    let vendor_id = pci_device.vendor;
    let device_id = pci_device.device;
    let device = FoundDevice::VirtioNetOverPci(pci_device);
    if devices.try_push(device).is_err() {
        warn!("too many devices found, ignoring this PCI device {vendor_id:04x}:{device_id:04x}");
    }
}

/// Parses `512@0xfeb00e00:12` into (base, size, irq).
fn parse_mmio_cmdline(value: &[u8]) -> (PAddr, usize, u8) {
    // TODO: Avoid parsing as a UTF-8 string once slice::split_once gets stabilized.
    let value = str::from_utf8(value).unwrap();

    // Split the string into 3 parts.
    let (size_str, rest) = value.split_once('@').unwrap();
    let (base_str, irq_str) = rest.split_once(':').unwrap();

    // Parse them as integers.
    let size = size_str.parse::<usize>().unwrap();
    let base = usize::from_str_radix(base_str.strip_prefix("0x").unwrap(), 16).unwrap();
    let base = PAddr::new(base);
    let irq = irq_str.parse::<u8>().unwrap();

    (base, size, irq)
}

fn probe_cmdline(devices: &mut ArrayVec<FoundDevice, 8>, cmdline: &[u8]) {
    let parser = Parser::new(cmdline);
    for param in parser {
        let param = param.expect("failed to parse cmdline");
        match param.key {
            // Example: virtio_mmio.device=512@0xfeb00e00:12
            b"virtio_mmio.device" => {
                // Parse the value.
                let (base, size, irq) = parse_mmio_cmdline(param.value);
                let device = FoundDevice::VirtioMmio { base, size, irq };
                if devices.try_push(device).is_err() {
                    warn!("too many devices found, ignoring this virtio-mmio device: base={base}");
                }
            }
            _ => {
                // Unknown keys. Ignore.
            }
        }
    }
}

fn discover_devices(cmdline: &[u8]) -> ArrayVec<FoundDevice, 8> {
    let mut devices = ArrayVec::new();
    #[cfg(target_arch = "x86_64")]
    probe_pci(&mut devices);
    probe_cmdline(&mut devices, cmdline);
    devices
}

fn init_net_driver(devices: &[FoundDevice]) -> (Box<dyn Driver<Notifier = PollNotifier>>, u8) {
    for device in devices {
        match device {
            #[cfg(target_arch = "x86_64")]
            FoundDevice::VirtioNetOverPci(pci_device) => {
                return init_virtio_net_over_pci(pci_device);
            }
            FoundDevice::VirtioMmio { base, size, irq } => {
                return init_virtio_net_over_mmio(*base, *size, *irq);
            }
        }
    }

    panic!("no supported network device found");
}

pub fn init(cmdline: &[u8]) {
    let devices = discover_devices(cmdline);
    trace!("discovered {} devices:", devices.len());
    for device in &devices {
        match device {
            #[cfg(target_arch = "x86_64")]
            FoundDevice::VirtioNetOverPci(pci_device) => {
                trace!(
                    "  virtio-net over PCI: bus={}, slot={}",
                    pci_device.bus, pci_device.slot
                );
            }
            FoundDevice::VirtioMmio { base, .. } => {
                trace!("  virtio-net over MMIO: base={base:}");
            }
        }
    }

    let (driver, irq) = init_net_driver(devices.as_slice());
    let mut net = NET_MUX.lock();
    let device_id = net
        .add_device(driver, irq)
        .expect("failed to add network device");

    // Enable the virtio-net IRQ. This must done before starting DHCP.
    // Otherwise, DHCP replies may arrive too early, before unmasking IRQ.
    arch::interrupt_acquire(irq).expect("failed to enable virtio-net IRQ");

    net.start_dhcp(device_id);
    drop(net);
}
