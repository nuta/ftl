//!! Virtio MMIO transport.
//!
//! <https://docs.oasis-open.org/virtio/virtio/v1.3/csd01/virtio-v1.3-csd01.html#x1-1920004>
use core::ptr::read_volatile;
use core::ptr::write_volatile;

use ftl_driver::env::Env;

use crate::transport::Error;
use crate::transport::IsrStatus;
use crate::transport::VirtioTransport;
use crate::virtqueue::VirtQueue;
use crate::virtqueue::vring_size;

const MMIO_MAGIC_VALUE: usize = 0x000;
const MMIO_VERSION: usize = 0x004;
const MMIO_DEVICE_ID: usize = 0x008;
const MMIO_HOST_FEATURES: usize = 0x010;
const MMIO_HOST_FEATURES_SEL: usize = 0x014;
const MMIO_GUEST_FEATURES: usize = 0x020;
const MMIO_GUEST_FEATURES_SEL: usize = 0x024;
const MMIO_GUEST_PAGE_SIZE: usize = 0x028;
const MMIO_QUEUE_SEL: usize = 0x030;
const MMIO_QUEUE_NUM_MAX: usize = 0x034;
const MMIO_QUEUE_NUM: usize = 0x038;
const MMIO_QUEUE_ALIGN: usize = 0x03c;
const MMIO_QUEUE_PFN: usize = 0x040;
const MMIO_QUEUE_READY: usize = 0x044;
const MMIO_QUEUE_NOTIFY: usize = 0x050;
const MMIO_INTERRUPT_STATUS: usize = 0x060;
const MMIO_INTERRUPT_ACK: usize = 0x064;
const MMIO_STATUS: usize = 0x070;
const MMIO_QUEUE_DESC_LOW: usize = 0x080;
const MMIO_QUEUE_DESC_HIGH: usize = 0x084;
const MMIO_QUEUE_DRIVER_LOW: usize = 0x090;
const MMIO_QUEUE_DRIVER_HIGH: usize = 0x094;
const MMIO_QUEUE_DEVICE_LOW: usize = 0x0a0;
const MMIO_QUEUE_DEVICE_HIGH: usize = 0x0a4;
const MMIO_CONFIG: usize = 0x100;

const VIRTIO_MAGIC: u32 = 0x7472_6976;
const PAGE_SIZE: usize = 4096;

const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER: u32 = 2;
const STATUS_DRIVER_OK: u32 = 4;
const STATUS_FEATURES_OK: u32 = 8;

pub struct VirtioMmio {
    base: usize,
    modern: bool,
}

impl VirtioMmio {
    pub unsafe fn new(base: usize) -> Result<Self, MmioError> {
        let mut this = Self {
            base,
            modern: false,
        };

        if this.read32(MMIO_MAGIC_VALUE) != VIRTIO_MAGIC {
            return Err(MmioError::BadMagic);
        }

        this.modern = match this.read32(MMIO_VERSION) {
            1 => false,
            2 => true,
            _ => return Err(MmioError::UnsupportedVersion),
        };

        if this.read32(MMIO_DEVICE_ID) == 0 {
            return Err(MmioError::NoDevice);
        }

        Ok(this)
    }

    pub fn device_id(&self) -> u32 {
        self.read32(MMIO_DEVICE_ID)
    }

    fn read32(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    fn write32(&self, offset: usize, value: u32) {
        unsafe { write_volatile((self.base + offset) as *mut u32, value) }
    }

    fn write64(&self, low_offset: usize, high_offset: usize, value: u64) {
        self.write32(low_offset, value as u32);
        self.write32(high_offset, (value >> 32) as u32);
    }

    fn read8(&self, offset: usize) -> u8 {
        unsafe { read_volatile((self.base + offset) as *const u8) }
    }
}

#[derive(Debug)]
pub enum MmioError {
    BadMagic,
    UnsupportedVersion,
    NoDevice,
}

impl VirtioTransport for VirtioMmio {
    fn acknowledge(&self, _env: &dyn Env) {
        // Reset the device.
        self.write32(MMIO_STATUS, 0);

        self.write32(MMIO_STATUS, STATUS_ACKNOWLEDGE);
        self.write32(MMIO_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);
    }

    fn read_device_features(&self, _env: &dyn Env) -> u32 {
        self.write32(MMIO_HOST_FEATURES_SEL, 0);
        self.read32(MMIO_HOST_FEATURES)
    }

    fn write_guest_features(&self, _env: &dyn Env, guest_features: u32) -> Result<(), Error> {
        self.write32(MMIO_GUEST_FEATURES_SEL, 0);
        self.write32(MMIO_GUEST_FEATURES, guest_features);

        // Modern devices need some more negotiation.
        if self.modern {
            self.write32(MMIO_GUEST_FEATURES_SEL, 1); // Higher bits
            self.write32(MMIO_GUEST_FEATURES, 1 << 0); // VERSION_1

            // Check if the device accepts the version 1 spec.
            let status = STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK;
            self.write32(MMIO_STATUS, status);
            if self.read32(MMIO_STATUS) & STATUS_FEATURES_OK == 0 {
                return Err(Error::FeaturesNotAccepted);
            }
        }

        Ok(())
    }

    fn is_modern(&self) -> bool {
        self.modern
    }

    fn driver_ok(&self, _env: &dyn Env) {
        let mut status = STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK;
        if self.modern {
            status |= STATUS_FEATURES_OK;
        }

        self.write32(MMIO_STATUS, status);
    }

    fn setup_virtqueue<C>(&self, env: &dyn Env, queue_index: u16) -> Result<VirtQueue<C>, Error> {
        self.write32(MMIO_QUEUE_SEL, queue_index as u32);

        let in_use = if self.modern {
            self.read32(MMIO_QUEUE_READY) != 0
        } else {
            self.read32(MMIO_QUEUE_PFN) != 0
        };

        if in_use {
            return Err(Error::QueueInUse);
        }

        let queue_size = self.read32(MMIO_QUEUE_NUM_MAX);
        if queue_size == 0 {
            return Err(Error::BadQueueSize(queue_size));
        }

        let queue_size: u16 = queue_size
            .try_into()
            .map_err(|_| Error::BadQueueSize(queue_size))?;
        let dma_size = vring_size(queue_size);

        let dmabuf = env.alloc_dma(dma_size).map_err(|_| Error::AllocFailed)?;
        let pfn = dmabuf.paddr() / PAGE_SIZE;
        if !self.modern && pfn > u32::MAX as usize {
            env.free_dma(dmabuf);
            return Err(Error::TooHighPAddr);
        }

        let queue = match VirtQueue::new(queue_index, queue_size, dmabuf) {
            Ok(queue) => queue,
            Err(dmabuf) => {
                env.free_dma(dmabuf);
                return Err(Error::AllocFailed);
            }
        };

        self.write32(MMIO_QUEUE_NUM, queue_size as u32);
        if self.modern {
            self.write64(
                MMIO_QUEUE_DESC_LOW,
                MMIO_QUEUE_DESC_HIGH,
                queue.desc_paddr(),
            );
            self.write64(
                MMIO_QUEUE_DRIVER_LOW,
                MMIO_QUEUE_DRIVER_HIGH,
                queue.avail_paddr(),
            );
            self.write64(
                MMIO_QUEUE_DEVICE_LOW,
                MMIO_QUEUE_DEVICE_HIGH,
                queue.used_paddr(),
            );
            self.write32(MMIO_QUEUE_READY, 1);
        } else {
            self.write32(MMIO_GUEST_PAGE_SIZE, PAGE_SIZE as u32);
            self.write32(MMIO_QUEUE_ALIGN, PAGE_SIZE as u32);
            self.write32(MMIO_QUEUE_PFN, pfn as u32);
        }

        Ok(queue)
    }

    fn read_device_config8(&self, _env: &dyn Env, offset: u16) -> u8 {
        self.read8(MMIO_CONFIG + offset as usize)
    }

    fn read_isr(&self, _env: &dyn Env) -> IsrStatus {
        let raw = self.read32(MMIO_INTERRUPT_STATUS);
        if raw != 0 {
            self.write32(MMIO_INTERRUPT_ACK, raw);
        }

        IsrStatus(raw as u8)
    }

    fn notify<C>(&self, _env: &dyn Env, virtqueue: &VirtQueue<C>) {
        self.write32(MMIO_QUEUE_NOTIFY, virtqueue.queue_index() as u32);
    }
}
