use core::arch::asm;
use core::mem::MaybeUninit;

use ftl::driver::DmaBuf;
use ftl::error::ErrorCode;
use ftl::interrupt::Interrupt;
use ftl::pci::PciEntry;
use ftl_utils::alignment::align_up;

use crate::virtqueue::Desc;
use crate::virtqueue::UsedElem;
use crate::virtqueue::VirtQueue;

const PCI_IOPORT_DEVICE_FEATURES: u16 = 0;
const PCI_IOPORT_GUEST_FEATURES: u16 = 4;
const PCI_IOPORT_QUEUE_PFN: u16 = 8;
const PCI_IOPORT_QUEUE_SIZE: u16 = 12;
const PCI_IOPORT_QUEUE_SEL: u16 = 14;
const PCI_IOPORT_QUEUE_NOTIFY: u16 = 16;
const PCI_IOPORT_STATUS: u16 = 18;
const PCI_IOPORT_ISR: u16 = 19;
const PCI_IOPORT_CONFIG: u16 = 20;

const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;

/// The type of virtio device to probe for.
///
/// The value must match the Subsystem Device ID of the device type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum DeviceType {
    Network = 1,
}

#[derive(Debug)]
pub enum Error {
    DmaBufAlloc(ErrorCode),
    QueueSizeZero,
    TooHighPAddr,
    VirtQueueFull,
}

fn get_vring_size(queue_size: u16) -> usize {
    let n = queue_size as usize;
    align_up(size_of::<Desc>() * n + size_of::<u16>() * (2 + n), 4096)
        + align_up(size_of::<UsedElem>() * n, 4096)
}

#[derive(Debug)]
pub enum ProbeError {
    PciLookup(ErrorCode),
    DeviceNotFound,
    PciSetBusmaster(ErrorCode),
    PciGetBar(ErrorCode),
    PciGetInterruptLine(ErrorCode),
    PciGetSubsystemId(ErrorCode),
    PciAcquireInterrupt(ErrorCode),
    PciNew(ErrorCode),
}

pub struct Prober {
    virtio: VirtioPci,
    interrupt: Interrupt,
}

fn find_pci_device(device_type: DeviceType) -> Result<PciEntry, ProbeError> {
    for device_id in 0x1000..=0x103f {
        let mut entries: MaybeUninit<[PciEntry; 10]> = MaybeUninit::uninit();
        let ptr = entries.as_mut_ptr() as *mut PciEntry;

        let n =
            ftl::pci::sys_pci_lookup(ptr, 10, 0x1af4, device_id).map_err(ProbeError::PciLookup)?;
        let devices =
            unsafe { core::slice::from_raw_parts(entries.as_ptr() as *const PciEntry, n) };

        if n == 0 {
            continue;
        }

        for i in 0..n {
            let entry = devices[i];
            let subsystem_id = ftl::pci::sys_pci_get_subsystem_id(entry.bus, entry.slot)
                .map_err(ProbeError::PciGetSubsystemId)?;
            if subsystem_id == device_type as u16 {
                return Ok(entry);
            }
        }
    }

    Err(ProbeError::DeviceNotFound)
}

impl Prober {
    pub fn probe(device_type: DeviceType) -> Result<Self, ProbeError> {
        // Look up virtio-net PCI device
        let entry = find_pci_device(device_type)?;

        // Enable bus mastering
        ftl::pci::sys_pci_set_busmaster(entry.bus, entry.slot, true)
            .map_err(ProbeError::PciSetBusmaster)?;

        // Get BAR0 (I/O port base for legacy virtio)
        let bar0 =
            ftl::pci::sys_pci_get_bar(entry.bus, entry.slot, 0).map_err(ProbeError::PciGetBar)?;
        let iobase = (bar0 & 0xfffffffc) as u16;

        // Get interrupt line and acquire it
        let irq = ftl::pci::sys_pci_get_interrupt_line(entry.bus, entry.slot)
            .map_err(ProbeError::PciGetInterruptLine)?;

        // Enable IOPL for direct I/O access
        ftl::syscall::sys_x64_iopl(true).map_err(ProbeError::PciAcquireInterrupt)?;

        // Acquire the interrupt for the virtio device.
        let interrupt = Interrupt::acquire(irq).map_err(ProbeError::PciAcquireInterrupt)?;

        let virtio = VirtioPci::new(iobase);
        virtio.acknowledge();

        Ok(Self { virtio, interrupt })
    }

    pub fn read_guest_features(&self) -> u32 {
        self.virtio.read_guest_features()
    }

    pub fn finish(self, guest_features: u32) -> (VirtioPci, Interrupt) {
        self.virtio.write_guest_features(guest_features);
        self.virtio.driver_ok();

        (self.virtio, self.interrupt)
    }
}

pub struct IsrStatus(u8);

impl IsrStatus {
    pub fn virtqueue_updated(&self) -> bool {
        self.0 & 1 != 0
    }
}

pub struct VirtioPci {
    iobase: u16,
}

impl VirtioPci {
    pub fn probe(device_type: DeviceType) -> Result<Prober, ProbeError> {
        Prober::probe(device_type)
    }

    fn new(iobase: u16) -> Self {
        Self { iobase }
    }

    fn acknowledge(&self) {
        // 1. Reset the device. This is not required on initial start up.
        // 2. The ACKNOWLEDGE status bit is set: we have noticed the device.
        self.out8(PCI_IOPORT_STATUS, STATUS_ACKNOWLEDGE);

        // 3. The DRIVER status bit is set: we know how to drive the device.
        self.out8(PCI_IOPORT_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);
    }

    fn read_guest_features(&self) -> u32 {
        self.in32(PCI_IOPORT_DEVICE_FEATURES)
    }

    pub fn write_guest_features(&self, guest_features: u32) {
        // 5. The subset of Device Feature Bits understood by the driver is
        //    written to the device.
        self.out32(PCI_IOPORT_GUEST_FEATURES, guest_features);
    }

    fn driver_ok(&self) {
        // 6. The DRIVER_OK status bit is set.
        self.out8(
            PCI_IOPORT_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK,
        );
    }

    pub fn setup_virtqueue<C>(&self, queue_index: u16) -> Result<VirtQueue<C>, Error> {
        // 1. Write the virtqueue index (first queue is 0) to the Queue Select
        //    field.
        self.out16(PCI_IOPORT_QUEUE_SEL, queue_index);

        // 2. Read the virtqueue size from the Queue Size field, which is
        //    always a power of 2.
        let queue_size = self.in16(PCI_IOPORT_QUEUE_SIZE);
        if queue_size == 0 {
            // If this field is 0, the virtqueue does not exist.
            return Err(Error::QueueSizeZero);
        }

        let vring_size = get_vring_size(queue_size);

        // 3. Allocate and zero virtqueue in contiguous physical memory, on a
        //    4096 byte alignment.
        let dmabuf = DmaBuf::alloc(vring_size).map_err(Error::DmaBufAlloc)?;

        // Write the physical address, divided by 4096 to the Queue Address
        //    field.
        let pfn: u32 = (dmabuf.paddr() / 4096)
            .try_into()
            .map_err(|_| Error::TooHighPAddr)?;
        self.out32(PCI_IOPORT_QUEUE_PFN, pfn);

        Ok(VirtQueue::new(queue_index, queue_size, dmabuf))
    }

    pub fn read_device_config8(&self, offset: u16) -> u8 {
        self.in8(PCI_IOPORT_CONFIG + offset)
    }

    /// Reads and clears the ISR status register.
    pub fn read_isr(&self) -> IsrStatus {
        let raw = self.in8(PCI_IOPORT_ISR);
        IsrStatus(raw)
    }

    pub fn notify<C>(&self, virtqueue: &VirtQueue<C>) {
        self.out16(PCI_IOPORT_QUEUE_NOTIFY, virtqueue.queue_index());
    }

    fn out32(&self, port: u16, value: u32) {
        unsafe {
            asm!("out dx, eax", in("dx") self.iobase + port, in("eax") value);
        };
    }

    fn out16(&self, port: u16, value: u16) {
        unsafe {
            asm!("out dx, ax", in("dx") self.iobase + port, in("ax") value);
        };
    }

    fn out8(&self, port: u16, value: u8) {
        unsafe {
            asm!("out dx, al", in("dx") self.iobase + port, in("al") value);
        };
    }

    fn in32(&self, port: u16) -> u32 {
        let value: u32;
        unsafe {
            asm!("in eax, dx", in("dx") self.iobase + port, out("eax") value);
        };
        value
    }

    fn in16(&self, port: u16) -> u16 {
        let value: u16;
        unsafe {
            asm!("in ax, dx", in("dx") self.iobase + port, out("ax") value);
        };
        value
    }

    fn in8(&self, port: u16) -> u8 {
        let value: u8;
        unsafe {
            asm!("in al, dx", in("dx") self.iobase + port, out("al") value);
        };
        value
    }
}
