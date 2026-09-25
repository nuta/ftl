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
const MMIO_QUEUE_NOTIFY: usize = 0x050;
const MMIO_INTERRUPT_STATUS: usize = 0x060;
const MMIO_INTERRUPT_ACK: usize = 0x064;
const MMIO_STATUS: usize = 0x070;
const MMIO_CONFIG: usize = 0x100;

const VIRTIO_MAGIC: u32 = 0x7472_6976;
const PAGE_SIZE: usize = 4096;

const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER: u32 = 2;
const STATUS_DRIVER_OK: u32 = 4;

pub struct VirtioMmio {
    base: usize,
}

impl VirtioMmio {
    pub unsafe fn new(base: usize) -> Result<Self, MmioError> {
        let this = Self { base };
        if this.read32(MMIO_MAGIC_VALUE) != VIRTIO_MAGIC {
            return Err(MmioError::BadMagic);
        }

        if this.read32(MMIO_VERSION) != 1 {
            return Err(MmioError::UnsupportedVersion);
        }

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

        // Configure the device.
        self.write32(MMIO_GUEST_PAGE_SIZE, PAGE_SIZE as u32);
        self.write32(MMIO_STATUS, STATUS_ACKNOWLEDGE);
        self.write32(MMIO_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);
    }

    fn read_device_features(&self, _env: &dyn Env) -> u32 {
        self.write32(MMIO_HOST_FEATURES_SEL, 0);
        self.read32(MMIO_HOST_FEATURES)
    }

    fn write_guest_features(&self, _env: &dyn Env, guest_features: u32) {
        self.write32(MMIO_GUEST_FEATURES_SEL, 0);
        self.write32(MMIO_GUEST_FEATURES, guest_features);
    }

    fn driver_ok(&self, _env: &dyn Env) {
        self.write32(
            MMIO_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK,
        );
    }

    fn setup_virtqueue<C>(&self, env: &dyn Env, queue_index: u16) -> Result<VirtQueue<C>, Error> {
        self.write32(MMIO_QUEUE_SEL, queue_index as u32);

        if self.read32(MMIO_QUEUE_PFN) != 0 {
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
        let pfn = match (dmabuf.paddr() / PAGE_SIZE).try_into() {
            Ok(pfn) => pfn,
            Err(_) => {
                env.free_dma(dmabuf);
                return Err(Error::TooHighPAddr);
            }
        };

        let queue = match VirtQueue::new(queue_index, queue_size, dmabuf) {
            Ok(queue) => queue,
            Err(dmabuf) => {
                env.free_dma(dmabuf);
                return Err(Error::AllocFailed);
            }
        };

        self.write32(MMIO_QUEUE_NUM, queue_size as u32);
        self.write32(MMIO_QUEUE_ALIGN, PAGE_SIZE as u32);
        self.write32(MMIO_QUEUE_PFN, pfn);
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
