use alloc::vec::Vec;

use crate::OutOfMemoryError;
use crate::ethernet::MacAddr;
use crate::packet::Packet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(u8);

pub trait Device {
    fn mac_addr(&self) -> &MacAddr;
    fn transmit(&mut self, pkt: &mut Packet);
}

struct Entry<D> {
    id: DeviceId,
    device: D,
}

pub(crate) struct DeviceMap<D: Device> {
    next_id: u8,
    devices: Vec<Entry<D>>,
}

impl<D: Device> DeviceMap<D> {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            devices: Vec::new(),
        }
    }

    pub fn add(&mut self, device: D) -> Result<DeviceId, OutOfMemoryError> {
        self.devices.try_reserve(1).map_err(|_| OutOfMemoryError)?;

        let id = DeviceId(self.next_id);
        self.devices.push(Entry { id, device });
        self.next_id += 1;
        Ok(id)
    }

    pub fn get_mut(&mut self, id: DeviceId) -> Option<&mut D> {
        for entry in &mut self.devices {
            if entry.id == id {
                return Some(&mut entry.device);
            }
        }

        None
    }
}
