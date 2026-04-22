use alloc::vec::Vec;

use crate::endian::Ne;
use crate::ethernet::MacAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::packet::Packet;
use crate::packet::{self};
use crate::route::Route;
use crate::route::RouteTable;

enum ArpEntry {}

pub(crate) struct ArpTable {
    entries: Vec<ArpEntry>,
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

const OPCODE_REQUEST: u16 = 1;
const OPCODE_REPLY: u16 = 2;

#[derive(Debug)]
pub enum TxError {
    PacketAlloc(packet::AllocError),
}

fn transmit_tx(route: &Route, remote_addr: Ipv4Addr, remote_mac: MacAddr) -> Result<(), TxError> {
    let mut pkt = Packet::new(1024).map_err(TxError::PacketAlloc)?;
    let header = ArpPacket {
        hw_type: 1.into(),
        proto_type: 0x0800.into(),
        hw_len: 6.into(),
        proto_len: 4.into(),
        opcode: OPCODE_REPLY.into(),
        sender_hw_addr: route.mac_addr(),
        sender_proto_addr: remote_addr.into(),
        target_hw_addr: remote_mac,
        target_proto_addr: remote_addr.into(),
    };

    Ok(())
}

#[derive(Debug)]
pub enum RxError {
    PacketRead(packet::ReserveError),
    BadOpcode(u16),
}

pub(crate) fn handle_rx(routes: &mut RouteTable, pkt: &mut Packet) -> Result<(), RxError> {
    let arp = pkt.read::<ArpPacket>().map_err(RxError::PacketRead)?;
    let sender_addr = Ipv4Addr::from(arp.sender_proto_addr);
    let target_addr = Ipv4Addr::from(arp.target_proto_addr);

    match arp.opcode.into() {
        OPCODE_REQUEST => {
            // Request
            trace!(
                "ARP request: sender: {}, target: {}",
                sender_addr, target_addr
            );

            let route = routes.lookup_by_dest_exact(sender_addr);
            if let Some(route) = route {
                //
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
