use alloc::vec::Vec;

use crate::Device;
use crate::Io;
use crate::device::DeviceId;
use crate::device::DeviceMap;
use crate::endian::Ne;
use crate::ethernet;
use crate::ethernet::EtherType;
use crate::ethernet::EthernetHeader;
use crate::ethernet::MacAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::packet::{self};
use crate::route::Route;
use crate::route::RouteTable;

enum ArpEntry {}

pub(crate) struct ArpTable {
    entries: Vec<ArpEntry>,
}

impl ArpTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

// Wire layout has no padding after (hw_len, proto_len); `repr(C)` alone would insert a byte
// before `opcode` and shift every field after it.
#[repr(C, packed)]
struct ArpPacket {
    hw_type: Ne<u16>,
    proto_type: Ne<u16>,
    hw_len: u8,
    proto_len: u8,
    opcode: Ne<u16>,
    sender_hw_addr: MacAddr,
    sender_proto_addr: Ne<u32>,
    target_hw_addr: MacAddr,
    target_proto_addr: Ne<u32>,
}

impl WriteableToPacket for ArpPacket {}

const HWTYPE_ETHERNET: u16 = 1;
const PROTOTYPE_IPV4: u16 = 0x0800;
const OPCODE_REQUEST: u16 = 1;
const OPCODE_REPLY: u16 = 2;

#[derive(Debug)]
pub enum TxError {
    PacketAlloc(packet::AllocError),
    PacketWrite(packet::ReserveError),
    EthernetTx(ethernet::TxError),
}

fn transmit_arp_reply<D: Device>(
    route: &Route,
    device: &mut D,
    remote_addr: Ipv4Addr,
    remote_mac: MacAddr,
) -> Result<(), TxError> {
    let arp_pkt = ArpPacket {
        hw_type: HWTYPE_ETHERNET.into(),
        proto_type: PROTOTYPE_IPV4.into(),
        hw_len: 6.into(),    // 6 bytes for Ethernet address
        proto_len: 4.into(), // 4 bytes for IPv4 address
        opcode: OPCODE_REPLY.into(),
        sender_hw_addr: *device.mac_addr(),
        sender_proto_addr: route.ipv4_addr().into(),
        target_hw_addr: remote_mac,
        target_proto_addr: remote_addr.into(),
    };

    let mut pkt = Packet::new(size_of::<ArpPacket>(), size_of::<EthernetHeader>())
        .map_err(TxError::PacketAlloc)?;
    pkt.write_back(arp_pkt).map_err(TxError::PacketWrite)?;

    ethernet::transmit(device, EtherType::Arp, remote_mac, &mut pkt)
        .map_err(TxError::EthernetTx)?;
    Ok(())
}

#[derive(Debug)]
pub enum RxError {
    PacketRead(packet::ReserveError),
    BadOpcode(u16),
    BadHardwareType(u16),
    BadProtocolType(u16),
    BadHardwareLength(u8),
    BadProtocolLength(u8),
    ReplyFailed(TxError),
    DeviceNotFound(DeviceId),
}

pub(crate) fn handle_rx<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    pkt: &mut Packet,
) -> Result<(), RxError> {
    let arp = pkt.read::<ArpPacket>().map_err(RxError::PacketRead)?;

    let hw_type = arp.hw_type.into();
    if hw_type != HWTYPE_ETHERNET {
        return Err(RxError::BadHardwareType(hw_type));
    }

    let proto_type = arp.proto_type.into();
    if proto_type != PROTOTYPE_IPV4 {
        return Err(RxError::BadProtocolType(proto_type));
    }

    if arp.hw_len != 6 {
        return Err(RxError::BadHardwareLength(arp.hw_len));
    }

    if arp.proto_len != 4 {
        return Err(RxError::BadProtocolLength(arp.proto_len));
    }

    let sender_addr = Ipv4Addr::from(arp.sender_proto_addr);
    let target_addr = Ipv4Addr::from(arp.target_proto_addr);

    match arp.opcode.into() {
        OPCODE_REQUEST => {
            // Request
            trace!(
                "ARP request: sender: {}, target: {}",
                sender_addr, target_addr
            );

            let route = routes.lookup_by_dest_exact(target_addr);
            if let Some(route) = route {
                let device_id = route.device_id();
                let device = devices.get_mut(device_id).ok_or(RxError::DeviceNotFound(device_id))?;
                transmit_arp_reply(route, device, sender_addr, arp.sender_hw_addr)
                    .map_err(RxError::ReplyFailed)?;
            }

        }
        OPCODE_REPLY => {
            // Reply
            trace!(
                "ARP reply: sender: {}, target: {}",
                sender_addr, target_addr
            );
        }
        opcode => {
            return Err(RxError::BadOpcode(opcode));
        }
    }

    Ok(())
}
