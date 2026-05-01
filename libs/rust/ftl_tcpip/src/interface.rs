use alloc::vec::Vec;

use crate::OutOfMemoryError;
use crate::arp::ArpTable;
use crate::ethernet::MacAddr;
use crate::ip::Ipv4Addr;
use crate::ip::NetMask;
use crate::packet::Packet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InterfaceId(u8);

pub trait Device {
    fn mac_addr(&self) -> &MacAddr;
    fn transmit(&mut self, pkt: &mut Packet);
}

pub struct Interface<D> {
    id: InterfaceId,
    device: D,
    arp_table: ArpTable,
    ipv4_addr: Option<Ipv4Addr>,
    net_mask: NetMask,
}

impl<D: Device> Interface<D> {
    pub fn new(id: InterfaceId, device: D) -> Self {
        Self {
            id,
            device,
            arp_table: ArpTable::new(),
            ipv4_addr: None,
            net_mask: NetMask::new(0, 0, 0, 0),
        }
    }

    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }

    pub(crate) fn arp_table(&self) -> &ArpTable {
        &self.arp_table
    }

    pub(crate) fn arp_table_mut(&mut self) -> &mut ArpTable {
        &mut self.arp_table
    }

    pub fn set_ipv4_addr(&mut self, ipv4_addr: Ipv4Addr) {
        self.ipv4_addr = Some(ipv4_addr);
    }

    pub fn ipv4_addr(&self) -> Option<Ipv4Addr> {
        self.ipv4_addr
    }
}

pub(crate) struct InterfaceMap<D: Device> {
    next_id: u8,
    interfaces: Vec<Interface<D>>,
}

impl<D: Device> InterfaceMap<D> {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            interfaces: Vec::new(),
        }
    }

    pub fn add(&mut self, device: D) -> Result<InterfaceId, OutOfMemoryError> {
        self.interfaces
            .try_reserve(1)
            .map_err(|_| OutOfMemoryError)?;

        let id = InterfaceId(self.next_id);
        self.interfaces.push(Interface::new(id, device));
        self.next_id += 1;
        Ok(id)
    }

    pub fn get_mut(&mut self, id: InterfaceId) -> Option<&mut Interface<D>> {
        for iface in &mut self.interfaces {
            if iface.id == id {
                return Some(iface);
            }
        }

        None
    }

    pub fn get_mut_by_ipv4_addr(&mut self, addr: Ipv4Addr) -> Option<&mut Interface<D>> {
        for iface in &mut self.interfaces {
            if iface.ipv4_addr() == Some(addr) {
                return Some(iface);
            }
        }

        None
    }
}
