use hashbrown::HashMap;

use crate::Device;
use crate::TcpIp;
use crate::device::DeviceId;
use crate::endian::Ne;
use crate::ethernet;
use crate::ethernet::EtherType;
use crate::ethernet::EthernetHeader;
use crate::ethernet::MacAddr;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::packet;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::route::Route;

enum ArpEntry {
    Resolved(MacAddr),
}

pub(crate) struct ArpTable {
    entries: HashMap<Ipv4Addr, ArpEntry>,
}

impl ArpTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn add(&mut self, addr: Ipv4Addr, mac: MacAddr) {
        self.entries.insert(addr, ArpEntry::Resolved(mac));
    }

    pub fn lookup(&self, addr: Ipv4Addr) -> Option<&MacAddr> {
        match self.entries.get(&addr) {
            Some(ArpEntry::Resolved(mac)) => Some(mac),
            _ => None,
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

    ethernet::transmit(
        device,
        route,
        EtherType::Arp,
        &mut pkt,
        IpAddr::V4(remote_addr),
    )
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

pub(crate) fn handle_rx<I: Io>(tcpip: &mut TcpIp<I>, pkt: &mut Packet) -> Result<(), RxError> {
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
            let route = tcpip
                .routes
                .lookup_by_dest_exact_mut(IpAddr::V4(target_addr));
            if let Some(route) = route {
                let device_id = route.device_id();
                let device = tcpip
                    .devices
                    .get_mut(device_id)
                    .ok_or(RxError::DeviceNotFound(device_id))?;

                // Register the sender's MAC address so that we can reply to it.
                route.arp_table_mut().add(sender_addr, arp.sender_hw_addr);

                trace!("replying to ARP request for {}", target_addr);
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
