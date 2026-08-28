#![cfg(target_arch = "x86_64")]
use core::mem::size_of;

use ftl_driver::env::Env;
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
    QueueSizeZero,
    TooHighPAddr,
    AllocFailed,
}

pub fn vring_size(queue_size: u16) -> usize {
    let n = queue_size as usize;
    align_up(size_of::<Desc>() * n + size_of::<u16>() * (3 + n), 4096)
        + align_up(size_of::<u16>() * 3 + size_of::<UsedElem>() * n, 4096)
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
    pub fn new(iobase: u16) -> Self {
        Self { iobase }
    }

    pub fn acknowledge(&self, env: &dyn Env) {
        // 1. Reset the device. This is not required on initial startup.
        // 2. The ACKNOWLEDGE status bit is set: we have noticed the device.
        unsafe {
            env.out8(self.iobase + PCI_IOPORT_STATUS, STATUS_ACKNOWLEDGE);
        }

        // 3. The DRIVER status bit is set: we know how to drive the device.
        unsafe {
            env.out8(
                self.iobase + PCI_IOPORT_STATUS,
                STATUS_ACKNOWLEDGE | STATUS_DRIVER,
            );
        }
    }

    pub fn read_device_features(&self, env: &dyn Env) -> u32 {
        unsafe { env.in32(self.iobase + PCI_IOPORT_DEVICE_FEATURES) }
    }

    pub fn write_guest_features(&self, env: &dyn Env, guest_features: u32) {
        // 5. The subset of Device Feature Bits understood by the driver is
        //    written to the device.
        unsafe {
            env.out32(self.iobase + PCI_IOPORT_GUEST_FEATURES, guest_features);
        }
    }

    pub fn driver_ok(&self, env: &dyn Env) {
        // 6. The DRIVER_OK status bit is set.
        unsafe {
            env.out8(
                self.iobase + PCI_IOPORT_STATUS,
                STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK,
            );
        }
    }

    /// Sets up a virtqueue. `Env::alloc_dma` must return a zeroed DMA buffer
    /// of at least `vring_size(queue_size)` bytes, 4096-byte aligned.
    pub fn setup_virtqueue<C>(
        &self,
        env: &dyn Env,
        queue_index: u16,
    ) -> Result<VirtQueue<C>, Error> {
        // 1. Write the virtqueue index (first queue is 0) to the Queue Select
        //    field.
        unsafe {
            env.out16(self.iobase + PCI_IOPORT_QUEUE_SEL, queue_index);
        }

        // 2. Read the virtqueue size from the Queue Size field, which is
        //    always a power of 2.
        let queue_size = unsafe { env.in16(self.iobase + PCI_IOPORT_QUEUE_SIZE) };
        if queue_size == 0 {
            // If this field is 0, the virtqueue does not exist.
            return Err(Error::QueueSizeZero);
        }

        let size = vring_size(queue_size);

        // 3. Allocate and zero virtqueue in contiguous physical memory, on a
        //    4096 byte alignment.
        let dmabuf = env.alloc_dma(size).map_err(|_| Error::AllocFailed)?;

        // Write the physical address, divided by 4096 to the Queue Address
        //    field.
        let pfn: u32 = (dmabuf.paddr() / 4096)
            .try_into()
            .map_err(|_| Error::TooHighPAddr)?;
        unsafe {
            env.out32(self.iobase + PCI_IOPORT_QUEUE_PFN, pfn);
        }

        Ok(VirtQueue::new(queue_index, queue_size, dmabuf))
    }

    pub fn read_device_config8(&self, env: &dyn Env, offset: u16) -> u8 {
        unsafe { env.in8(self.iobase + PCI_IOPORT_CONFIG + offset) }
    }

    /// Reads and clears the ISR status register.
    pub fn read_isr(&self, env: &dyn Env) -> IsrStatus {
        let raw = unsafe { env.in8(self.iobase + PCI_IOPORT_ISR) };
        IsrStatus(raw)
    }

    pub fn notify<C>(&self, env: &dyn Env, virtqueue: &VirtQueue<C>) {
        unsafe {
            env.out16(
                self.iobase + PCI_IOPORT_QUEUE_NOTIFY,
                virtqueue.queue_index(),
            );
        }
    }
}
