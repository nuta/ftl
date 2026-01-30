//! Virtio device driver (legacy).
//!
//! # References
//!
//! Latest but very long:
//! <https://docs.oasis-open.org/virtio/virtio/v1.3/csd01/virtio-v1.3-csd01.html>
//!
//! Old but covers legacy + PCI concisely:
//! <https://ozlabs.org/~rusty/virtio-spec/virtio-0.9.5.pdf>

use core::arch::asm;
use core::ptr::read_volatile;
use core::ptr::write_volatile;
use core::sync::atomic::Ordering;
use core::sync::atomic::fence;

use ftl::error::ErrorCode;
use ftl::prelude::*;
use ftl_utils::alignment::align_up;

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
const STATUS_FEATURES_OK: u8 = 8;
const STATUS_DRIVER_FAILED: u8 = 128;

const DESC_F_NEXT: u16 = 1;
const DESC_F_WRITE: u16 = 2;

#[repr(C, packed)]
struct Desc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C, packed)]
struct UsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
struct Avail {
    flags: u16,
    idx: u16,
    ring: [u16; 0],
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

pub enum ChainEntry {
    Write { paddr: u64, len: u32 },
    Read { paddr: u64, len: u32 },
}

pub struct UsedChain {
    pub descs: Vec<ChainEntry>,
    pub total_len: u32,
}

pub struct VirtQueue {
    queue_index: u16,
    queue_size: u16,
    descs: *mut Desc,
    avail: *mut Avail,
    used: *mut UsedElem,
    free_indicies: Vec<u16>,
}

impl VirtQueue {
    pub fn new(queue_index: u16, queue_size: u16, vaddr: usize) -> Self {
        let descs = vaddr as *mut Desc;
        let avail_offset = size_of::<Desc>() * queue_size as usize;
        let avail = (vaddr + avail_offset) as *mut Avail;
        let used_offset = align_up(
            avail_offset + size_of::<u16>() * (2 + queue_size as usize),
            4096,
        );
        let used = (vaddr + used_offset) as *mut UsedElem;
        let mut free_indicies = Vec::with_capacity(queue_size as usize);
        for index in 0..queue_size {
            free_indicies.push(index);
        }
        unsafe {
            (*avail).flags = 0;
            (*avail).idx = 0;
        }
        Self {
            queue_index,
            queue_size,
            descs,
            avail,
            used,
            free_indicies,
        }
    }

    pub fn is_full(&self, n: usize) -> bool {
        self.free_indicies.len() < n
    }

    pub fn push(&mut self, chain: &[ChainEntry]) -> Result<(), Error> {
        assert!(chain.len() > 0);

        if self.is_full(chain.len()) {
            return Err(Error::VirtQueueFull);
        }

        // Add descriptors to the chain.
        let mut next_index = None;
        let head_index = self.free_indicies.pop().unwrap();
        for (i, entry) in chain.iter().enumerate() {
            let desc_index = if let Some(index) = next_index {
                index
            } else {
                // The first descriptor in the chain.
                head_index
            };

            let (mut flags, paddr, len) = match entry {
                ChainEntry::Write { paddr, len } => (DESC_F_WRITE, *paddr, *len),
                ChainEntry::Read { paddr, len } => (0, *paddr, *len),
            };

            let next = if i < chain.len() - 1 {
                // More entries to come. Prepare the next index.
                flags |= DESC_F_NEXT;
                self.free_indicies.pop().unwrap()
            } else {
                // This is the last descriptor in the chain.
                0
            };

            let desc = Desc {
                addr: paddr,
                len,
                flags,
                next,
            };

            unsafe {
                self.descs.offset(desc_index as isize).write(desc);
            }

            next_index = Some(next);
        }

        // Write the head index to the avail ring.
        let avail_index = unsafe { read_volatile(&(*self.avail).idx) };
        let ring_index = (avail_index % self.queue_size) as usize;
        unsafe {
            write_volatile((*self.avail).ring.as_mut_ptr().add(ring_index), head_index);
        }
        fence(Ordering::Release);
        unsafe {
            write_volatile(&mut (*self.avail).idx, avail_index.wrapping_add(1));
        }

        Ok(())
    }

    pub fn pop(&mut self) -> Option<UsedChain> {
        todo!()
    }

    pub fn notify(&self, virtio: &VirtioPci) {
        virtio.out16(PCI_IOPORT_QUEUE_NOTIFY, self.queue_index);
    }
}

pub struct VirtioPci {
    bus: u8,
    slot: u8,
    iobase: u16,
}

impl VirtioPci {
    pub fn new(bus: u8, slot: u8, iobase: u16) -> Self {
        Self { bus, slot, iobase }
    }

    pub fn initialize1(&self) -> u32 {
        // 1. Reset the device. This is not required on initial start up.
        // 2. The ACKNOWLEDGE status bit is set: we have noticed the device.
        self.out8(PCI_IOPORT_STATUS, STATUS_ACKNOWLEDGE);

        // 3. The DRIVER status bit is set: we know how to drive the device.
        self.out8(PCI_IOPORT_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);

        // 4 Device-specific setup, including reading the Device Feature Bits,
        //   discovery of virtqueues for the device, ...
        self.in32(PCI_IOPORT_DEVICE_FEATURES)
    }

    pub fn write_guest_features(&self, guest_features: u32) {
        // 5. The subset of Device Feature Bits understood by the driver is
        //    written to the device.
        self.out32(PCI_IOPORT_GUEST_FEATURES, guest_features);
    }

    pub fn initialize2(&self) {
        // 6. The DRIVER_OK status bit is set.
        self.out8(
            PCI_IOPORT_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK,
        );
    }

    pub fn setup_virtqueue(&self, queue_index: u16) -> Result<VirtQueue, Error> {
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
        let mut paddr = 0;
        let mut vaddr = 0;
        ftl::dmabuf::sys_dmabuf_alloc(vring_size, &mut vaddr, &mut paddr)
            .map_err(Error::DmaBufAlloc)?;

        // Write the physical address, divided by 4096 to the Queue Address
        //    field.
        let pfn: u32 = (paddr / 4096).try_into().map_err(|_| Error::TooHighPAddr)?;
        self.out32(PCI_IOPORT_QUEUE_PFN, pfn);

        Ok(VirtQueue::new(queue_index, queue_size, vaddr))
    }

    pub fn handle_interrupt(&self) {
        todo!()
    }

    pub fn read_device_config8(&self, offset: u16) -> u8 {
        self.in8(PCI_IOPORT_CONFIG + offset)
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
