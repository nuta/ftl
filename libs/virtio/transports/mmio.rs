use ftl_api::device::mmio::LittleEndianReadOnly;
use ftl_api::device::mmio::LittleEndianReadWrite;
use ftl_api::device::mmio::LittleEndianWriteOnly;
use ftl_api::device::mmio::ReadWrite;
use ftl_api::folio::Folio;
use ftl_api::println;
use ftl_api::types::address::PAddr;
use ftl_api::types::address::VAddr;

use super::VirtioTransport;
use crate::transports::IsrStatus;
use crate::DeviceType;

// "All register values are organized as Little Endian."
// (4.2.2 MMIO Device Register Layout).
static MAGIC_VALUE_REG: LittleEndianReadOnly<u32> = LittleEndianReadOnly::new(0x00);
static DEVICE_VERSION_REG: LittleEndianReadOnly<u32> = LittleEndianReadOnly::new(0x04);
static DEVICE_ID_REG: LittleEndianReadOnly<u32> = LittleEndianReadOnly::new(0x08);
static DEVICE_FEATURES_REG: LittleEndianReadOnly<u32> = LittleEndianReadOnly::new(0x10);
static DEVICE_FEATURES_SEL_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x14);
static DRIVER_FEATURES_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x20);
static DRIVER_FEATURES_SEL_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x24);
static QUEUE_SEL_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x30);
static QUEUE_NUM_MAX_REG: LittleEndianReadOnly<u32> = LittleEndianReadOnly::new(0x34);
static QUEUE_NUM_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x38);
static QUEUE_READY_REG: LittleEndianReadWrite<u32> = LittleEndianReadWrite::new(0x44);
static QUEUE_NOTIFY_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x50);
static INTERRUPT_STATUS_REG: LittleEndianReadOnly<u32> = LittleEndianReadOnly::new(0x60);
static DEVICE_STATUS_REG: LittleEndianReadWrite<u32> = LittleEndianReadWrite::new(0x70);
static QUEUE_DESC_LOW_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x80);
static QUEUE_DESC_HIGH_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x84);
static QUEUE_DRIVER_LOW_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x90);
static QUEUE_DRIVER_HIGH_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0x94);
static QUEUE_DEVICE_LOW_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0xa0);
static QUEUE_DEVICE_HIGH_REG: LittleEndianWriteOnly<u32> = LittleEndianWriteOnly::new(0xa4);
static CONFIG_REG_BASE: ReadWrite<u8> = ReadWrite::new(0x100);

pub struct VirtioMmio {
    mmio: Folio,
}

impl VirtioMmio {
    pub fn new(mmio: Folio) -> VirtioMmio {
        VirtioMmio { mmio }
    }
}

impl VirtioTransport for VirtioMmio {
    fn probe(&mut self) -> Option<DeviceType> {
        // Check if the device is present by checking t he magic value
        // ("virt" in little-endian).
        if MAGIC_VALUE_REG.read_u32(&mut self.mmio) != 0x74726976 {
            return None;
        }

        let version = DEVICE_VERSION_REG.read_u32(&mut self.mmio);
        if version != 2 {
            println!("virtio-mmio: unsupported device version: {}", version);
            return None;
        }

        let device_type = DEVICE_ID_REG.read_u32(&mut self.mmio);
        Some(device_type)
    }

    fn is_modern(&mut self) -> bool {
        true
    }

    fn read_device_config8(&mut self, offset: u16) -> u8 {
        unsafe { CONFIG_REG_BASE.read_with_offset(&mut self.mmio, offset as usize) }
    }

    fn read_isr_status(&mut self) -> IsrStatus {
        IsrStatus(unsafe { INTERRUPT_STATUS_REG.read_u32(&mut self.mmio) as u8 })
    }

    fn read_device_status(&mut self) -> u8 {
        unsafe { DEVICE_STATUS_REG.read_u32(&mut self.mmio) as u8 }
    }

    fn write_device_status(&mut self, value: u8) {
        unsafe {
            DEVICE_STATUS_REG.write_u32(&mut self.mmio, value as u32);
        }
    }

    fn read_device_features(&mut self) -> u64 {
        unsafe {
            DEVICE_FEATURES_SEL_REG.write_u32(&mut self.mmio, 0);
            let low = DEVICE_FEATURES_REG.read_u32(&mut self.mmio);
            DEVICE_FEATURES_SEL_REG.write_u32(&mut self.mmio, 1);
            let high = DEVICE_FEATURES_REG.read_u32(&mut self.mmio);
            ((high as u64) << 32) | (low as u64)
        }
    }

    fn write_driver_features(&mut self, value: u64) {
        unsafe {
            DRIVER_FEATURES_SEL_REG.write_u32(&mut self.mmio, 0);
            DRIVER_FEATURES_REG.write_u32(&mut self.mmio, (value & 0xffff_ffff) as u32);
            DRIVER_FEATURES_SEL_REG.write_u32(&mut self.mmio, 1);
            DRIVER_FEATURES_REG.write_u32(&mut self.mmio, (value >> 32) as u32);
        }
    }

    fn select_queue(&mut self, index: u16) {
        unsafe {
            QUEUE_SEL_REG.write_u32(&mut self.mmio, index as u32);
        }
    }

    fn queue_max_size(&mut self) -> u16 {
        unsafe { QUEUE_NUM_MAX_REG.read_u32(&mut self.mmio) as u16 }
    }

    fn set_queue_size(&mut self, queue_size: u16) {
        unsafe {
            QUEUE_NUM_REG.write_u32(&mut self.mmio, queue_size as u32);
        }
    }

    fn notify_queue(&mut self, index: u16) {
        unsafe {
            QUEUE_NOTIFY_REG.write_u32(&mut self.mmio, index as u32);
        }
    }

    fn enable_queue(&mut self) {
        unsafe {
            QUEUE_READY_REG.write_u32(&mut self.mmio, 1);
        }
    }

    fn set_queue_desc_paddr(&mut self, paddr: PAddr) {
        unsafe {
            QUEUE_DESC_LOW_REG.write_u32(&mut self.mmio, (paddr.as_usize() & 0xffff_ffff) as u32);
            QUEUE_DESC_HIGH_REG.write_u32(&mut self.mmio, (paddr.as_usize() >> 32) as u32);
        }
    }

    fn set_queue_driver_paddr(&mut self, paddr: PAddr) {
        unsafe {
            QUEUE_DRIVER_LOW_REG.write_u32(&mut self.mmio, (paddr.as_usize() & 0xffff_ffff) as u32);
            QUEUE_DRIVER_HIGH_REG.write_u32(&mut self.mmio, (paddr.as_usize() >> 32) as u32);
        }
    }

    fn set_queue_device_paddr(&mut self, paddr: PAddr) {
        unsafe {
            QUEUE_DEVICE_LOW_REG.write_u32(&mut self.mmio, (paddr.as_usize() & 0xffff_ffff) as u32);
            QUEUE_DEVICE_HIGH_REG.write_u32(&mut self.mmio, (paddr.as_usize() >> 32) as u32);
        }
    }
}
