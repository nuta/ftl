use crate::{endian::Ne, ethernet::MacAddr, packet::Packet};

#[derive(Debug)]
#[repr(C)]
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

pub(crate) fn handle_rx(pkt: &mut Packet) {
    let arp = pkt.read::<ArpPacket>().unwrap();
    info!("ARP packet: {:#?}", arp);
}
