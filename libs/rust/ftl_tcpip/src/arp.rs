use crate::endian::Ne;
use crate::ethernet::MacAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::packet::Packet;
use crate::route::RouteTable;

use alloc::vec::Vec;

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

pub(crate) fn handle_rx(routes: &mut RouteTable, pkt: &mut Packet) {
    let arp = pkt.read::<ArpPacket>().unwrap();
    let sender_addr = Ipv4Addr::from(arp.sender_proto_addr);
    let target_addr = Ipv4Addr::from(arp.target_proto_addr);
    info!(
        "ARP packet: sender: {}, target: {}",
        sender_addr, target_addr
    );
}
