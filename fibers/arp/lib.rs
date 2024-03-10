#![no_std]
use core::mem::size_of;

use ftl_api::channel::Channel;
use ftl_api::collections::HashMap;
use ftl_api::environ::Environ;
use ftl_api::prelude::*;
use ftl_api::types::message::Message;
use ftl_api::types::message::MessageOrSignal;
use ftl_autogen::fibers::arp::Deps;
use packetbuf::zerocopy::ArpHwType;
use packetbuf::zerocopy::ArpOp;
use packetbuf::zerocopy::ArpPacket;
use packetbuf::zerocopy::ArpProtoType;
use packetbuf::zerocopy::EtherType;
use packetbuf::zerocopy::Ipv4Addr;
use packetbuf::zerocopy::MacAddr;
use packetbuf::PacketBuf;

enum ArpEntry {
    Resolved { dst_mac: MacAddr },
    Resolving { pending: Vec<PacketBuf> },
}

struct Arp {
    table: HashMap<Ipv4Addr, ArpEntry>,
    mac: MacAddr,
}

impl Arp {
    fn send_ipv4_packet(
        &mut self,
        net_device: &mut Channel,
        dst_ip: Ipv4Addr,
        src_ip: Ipv4Addr,
        buf: PacketBuf,
    ) {
        match self.table.get_mut(&dst_ip) {
            Some(ArpEntry::Resolved { dst_mac }) => {
                // We already know the MAC address of the destination. Send immediately.
                let mut payload_buf = [0; 512]; // FIXME:
                payload_buf[..buf.len()].copy_from_slice(buf.as_bytes());
                net_device
                    .send(Message::NetworkTx {
                        dst_mac: dst_mac.as_bytes(),
                        ether_type: EtherType::Ipv4 as u16,
                        payload: payload_buf,
                        payload_len: buf.len(),
                    })
                    .unwrap();
            }
            Some(ArpEntry::Resolving { pending }) => {
                // We're waiting for ARP reply from the destination. Queue the packet.
                pending.push(buf);
            }
            None => {
                // We don't know the MAC address of the destination. Queue the packet
                // and initiate an ARP resolution.
                let pending = vec![buf];
                self.table.insert(dst_ip, ArpEntry::Resolving { pending });
                self.send_request(
                    net_device,
                    ArpOp::Request,
                    src_ip,
                    dst_ip,
                    MacAddr::BROADCAST,
                );
            }
        }
    }

    fn receive(&mut self, net_device: &mut Channel, mut buf: PacketBuf) {
        let pkt = buf.pop_front::<ArpPacket>().unwrap();
        let op: ArpOp = pkt.op.get().try_into().unwrap();
        let proto_type: ArpProtoType = pkt.proto_type.get().try_into().unwrap();
        let hw_type: ArpHwType = pkt.hw_type.get().try_into().unwrap();

        if proto_type != ArpProtoType::Ipv4
            || hw_type != ArpHwType::Ethernet
            || pkt.hw_addr_len != 6
            || pkt.proto_addr_len != 4
        {
            println!("arp: malformed packet, ignoring");
            return;
        }

        let src_ip = Ipv4Addr::from_ne_bytes(pkt.src_proto_addr);
        let src_mac = MacAddr::from_bytes(pkt.src_hw_addr);
        match op {
            ArpOp::Reply => {
                match self
                    .table
                    .insert(src_ip, ArpEntry::Resolved { dst_mac: src_mac })
                {
                    Some(ArpEntry::Resolving { mut pending }) => {
                        println!(
                            "arp: resolved {}, flusing {} pending packets",
                            src_ip,
                            pending.len()
                        );

                        for buf in pending.drain(..) {
                            let mut payload_buf = [0; 512]; // FIXME:
                            payload_buf[..buf.len()].copy_from_slice(buf.as_bytes());
                            net_device
                                .send(Message::NetworkTx {
                                    dst_mac: src_mac.as_bytes(),
                                    ether_type: EtherType::Ipv4 as u16,
                                    payload: payload_buf,
                                    payload_len: buf.len(),
                                })
                                .unwrap();
                        }
                    }
                    Some(ArpEntry::Resolved { .. }) | None => {
                        // We already know or we're not interested in this IP address. Ignore.
                    }
                }
            }
            ArpOp::Request => {
                let our_ip = todo!(); // TODO:
                let dst_ip = Ipv4Addr::from_ne_bytes(pkt.src_proto_addr);
                if dst_ip == our_ip {
                    // We received an ARP request for our own IP address. Send a reply.
                    self.send_request(net_device, ArpOp::Reply, our_ip, src_ip, self.mac);
                }
            }
        }
    }

    fn send_request(
        &mut self,
        net_device: &mut Channel,
        op: ArpOp,
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
        dst: MacAddr,
    ) {
        let mut buf = PacketBuf::new(PacketBuf::HEADROOM, size_of::<ArpPacket>());
        let mut pkt = buf.append::<ArpPacket>().unwrap();
        pkt.hw_type.set(ArpHwType::Ethernet as u16);
        pkt.proto_type.set(ArpProtoType::Ipv4 as u16);
        pkt.hw_addr_len = 6; // Ethernet
        pkt.proto_addr_len = 4; // IPv4 address
        pkt.op.set(op as u16);
        pkt.src_hw_addr = self.mac.as_bytes();
        pkt.src_proto_addr = src_ip.as_ne_bytes();
        pkt.dst_hw_addr = dst.as_bytes();
        pkt.dst_proto_addr = dst_ip.as_ne_bytes();

        let mut payload_buf = [0; 512]; // FIXME:
        payload_buf[..buf.len()].copy_from_slice(buf.as_bytes());
        net_device
            .send(Message::NetworkTx {
                dst_mac: dst.as_bytes(),
                ether_type: EtherType::Arp as u16,
                payload: payload_buf,
                payload_len: buf.len(),
            })
            .unwrap();
    }
}

pub fn main(mut env: Environ) {
    let mut deps: Deps = env.parse_deps().expect("failed to parse deps");

    let ret = deps.net_device.call(Message::GetMacAddr);
    let mac = match ret {
        Ok(MessageOrSignal::Message(Message::MacAddr(mac))) => MacAddr::from_bytes(mac),
        _ => panic!("failed to get MAC address"),
    };

    let mut arp = Arp {
        table: HashMap::new(),
        mac,
    };

    println!("sending dummy packet");
    let src_ip = Ipv4Addr::new(10, 0, 2, 15);
    let dst_ip = Ipv4Addr::new(10, 0, 2, 2);
    let dst = MacAddr::new(0xff, 0xff, 0xff, 0xff, 0xff, 0xff);
    arp.send_request(&mut deps.net_device, ArpOp::Request, src_ip, dst_ip, dst);
}
