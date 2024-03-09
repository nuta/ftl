use ftl_api::types::address::PAddr;

use super::DeviceType;

pub mod mmio;

#[repr(transparent)]
pub struct IsrStatus(pub u8);
const QUEUE_INTR: u8 = 1 << 0;
const DEVICE_CONFIG_INTR: u8 = 1 << 1;

pub trait VirtioTransport: Send + Sync {
    fn probe(&mut self) -> Option<DeviceType>;
    fn is_modern(&mut self) -> bool;
    fn read_device_config8(&mut self, offset: u16) -> u8;
    fn read_isr_status(&mut self) -> IsrStatus;
    fn read_device_status(&mut self) -> u8;
    fn write_device_status(&mut self, value: u8);
    fn read_device_features(&mut self) -> u64;
    fn write_driver_features(&mut self, value: u64);
    fn select_queue(&mut self, index: u16);
    fn queue_max_size(&mut self) -> u16;
    fn set_queue_size(&mut self, queue_size: u16);
    fn notify_queue(&mut self, index: u16);
    fn enable_queue(&mut self);
    fn set_queue_desc_paddr(&mut self, paddr: PAddr);
    fn set_queue_driver_paddr(&mut self, paddr: PAddr);
    fn set_queue_device_paddr(&mut self, paddr: PAddr);
}
