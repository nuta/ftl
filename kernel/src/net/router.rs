use alloc::vec::Vec;
use core::ops::Deref;

use ftl_driver::dma::DmaBuf;
use ftl_driver::net::Error;
use ftl_types::error::ErrorCode;
use ftl_types::net::ETHTYPE_ARP;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::FiveTuple;
use ftl_types::net::IPPROTO_TCP;

use super::device::Device;
use super::network::Network;
use super::packet::ARP_OP_REQUEST;
use super::packet::ArpInspector;
use super::packet::Error as PacketError;
use super::packet::EthernetInspector;
use super::packet::Ipv4Addr;
use super::packet::Ipv4Inspector;
use super::packet::TcpInspector;
use crate::net::GLOBAL_ENV;
use crate::shared_ref::SharedRef;

const OUR_IP: Ipv4Addr = Ipv4Addr::new(0x0a00_020f);

/// Pushes the RX buffer back to the driver when dropped, if not taken.
struct RxDmaBuf<'a> {
    device: &'a SharedRef<Device>,
    buf: Option<DmaBuf>,
}

impl<'a> RxDmaBuf<'a> {
    pub fn new(device: &'a SharedRef<Device>, buf: DmaBuf) -> Self {
        Self {
            device,
            buf: Some(buf),
        }
    }

    pub fn take(mut self) -> DmaBuf {
        self.buf.take().unwrap()
    }
}

impl<'a> Deref for RxDmaBuf<'a> {
    type Target = DmaBuf;

    fn deref(&self) -> &Self::Target {
        self.buf.as_ref().unwrap()
    }
}

/// Pushes the RX buffer back to the driver.
fn recycle_rx_buffer(device: &SharedRef<Device>, buf: DmaBuf) {
    if device.driver().provide(&GLOBAL_ENV, buf).is_err() {
        warn!("net: failed to recycle an RX buffer");
    }
}

impl<'a> Drop for RxDmaBuf<'a> {
    fn drop(&mut self) {
        if let Some(buf) = self.buf.take() {
            recycle_rx_buffer(self.device, buf);
        }
    }
}

pub struct Router {
    // TODO: Support multiple devices.
    device: SharedRef<Device>,
    networks: Vec<SharedRef<Network>>,
}

impl Router {
    pub fn new(device: SharedRef<Device>) -> Self {
        Self {
            device,
            networks: Vec::new(),
        }
    }

    pub fn add_network(&mut self, network: SharedRef<Network>) -> Result<(), ErrorCode> {
        self.networks
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        self.networks.push(network);
        Ok(())
    }

    fn find_network(&self, five_tuple: FiveTuple) -> Option<(SharedRef<Network>, usize)> {
        // TODO: 5-tuple hash map to avoid scanning all networks.
        for network in &self.networks {
            if let Some(cookie) = network.matches(five_tuple) {
                return Some((network.clone(), cookie));
            }
        }

        None
    }

    pub fn device(&self) -> &SharedRef<Device> {
        &self.device
    }

    fn handle_eth_frame(&self, buf: DmaBuf, headroom: usize, frame_len: usize) {
        let buf = RxDmaBuf::new(&self.device, buf);

        // Parse the Ethernet frame.
        let frame = &buf.as_slice()[headroom..headroom + frame_len];
        let frame = match EthernetInspector::new(frame) {
            Ok(frame) => frame,
            Err(e) => {
                trace!("failed to inspect Ethernet frame: {:?}", e);
                return;
            }
        };

        // Forward to the upper layer.
        let src_mac = frame.src_mac();
        let eth_type = frame.eth_type();
        match eth_type {
            ETHTYPE_ARP => self.handle_arp(frame.payload()),
            ETHTYPE_IPV4 => self.handle_ipv4(src_mac, buf, headroom, frame_len),
            _ => {
                trace!("unsupported ethernet type: {:x}", eth_type);
            }
        }
    }

    fn handle_arp(&self, packet: &[u8]) {
        let arp = match ArpInspector::new(packet) {
            Ok(arp) => arp,
            Err(e) => {
                trace!("failed to inspect ARP packet: {:?}", e);
                return;
            }
        };

        let src_mac = arp.src_mac();
        let src_ip = arp.src_ip();

        // Learn the sender's MAC address.
        self.device.learn_arp(&GLOBAL_ENV, src_ip, src_mac);

        // Reply to the ARP request if it's for our IP.
        if arp.operation() == ARP_OP_REQUEST && arp.dst_ip() == OUR_IP {
            self.device
                .send_arp_reply(&GLOBAL_ENV, src_mac, src_ip, OUR_IP);
        }
    }

    fn handle_ipv4(&self, buf: RxDmaBuf<'_>, headroom: usize, frame_len: usize) {
        let off = headroom + EthernetInspector::HEADER_LEN;
        let len = frame_len - EthernetInspector::HEADER_LEN;
        let packet = &buf.as_slice()[off..off + len];

        // Parse the IPv4 packet.
        let ipv4 = match Ipv4Inspector::new(packet) {
            Ok(ipv4) => ipv4,
            Err(e) => {
                trace!("failed to inspect IPv4 packet: {:?}", e);
                return;
            }
        };

        // Check if the IPv4 packet is valid.
        if let Err(e) = ipv4.validate() {
            trace!("failed to validate IPv4 packet: {:?}", e);
            return;
        }

        // Forward to the upper layer.
        match ipv4.ip_proto() {
            IPPROTO_TCP => self.handle_tcp(buf, &ipv4, , ),
            _ => {
                trace!("unsupported IP protocol: {:x}", ipv4.ip_proto());
            }
        }
    }

    fn handle_tcp(&self, buf: RxDmaBuf<'_>, ipv4: &Ipv4Inspector<'_>, off: usize, len: usize) {
        // Parse the TCP header.
        let packet = &buf.as_slice()[off..off + len];
        let tcp = match TcpInspector::new(packet) {
            Ok(tcp) => tcp,
            Err(e) => {
                trace!("failed to inspect TCP segment: {:?}", e);
                return;
            }
        };

        // Check if the TCP header is valid.
        if let Err(e) = tcp.validate(&ipv4) {
            trace!("failed to validate TCP segment: {:?}", e);
            return;
        }

        // Build the 5-tuple to look up the network.
        let local_ip = ipv4.dst_ip();
        let local_port = tcp.dst_port();
        let remote_ip = ipv4.src_ip();
        let remote_port = tcp.src_port();
        let five_tuple = FiveTuple {
            eth_type: ETHTYPE_IPV4,
            ip_proto: IPPROTO_TCP,
            local_ip: local_ip.as_u32(),
            local_port,
            remote_ip: remote_ip.as_u32(),
            remote_port,
        };

        // Find the network to forward the packet to.
        let Some((network, cookie)) = self.find_network(five_tuple) else {
            return;
        };

        // Forward the packet to the network.
        let packet_len = ipv4.packet_len();
        let header_len = ipv4.header_len();
        network.receive(buf.take(), packet_offset, packet_len, header_len, cookie);
    }

    pub fn handle_interrupt(&self) {
        // Do driver's interrupt work.
        let driver = self.device.driver();
        driver.handle_interrupt(&GLOBAL_ENV);

        // Process pending RX packets.
        loop {
            match driver.try_receive() {
                Ok((buf, headroom, frame_len)) => {
                    self.handle_eth_frame(buf, headroom, frame_len);
                }
                Err((Error::RxEmpty, _)) => {
                    // We've processed all pending RX packets.
                    break;
                }
                Err((error, buf)) => {
                    // Something went wrong. Abort.
                    warn!("net: failed to receive packet: {:?}", error);
                    if let Some(buf) = buf {
                        recycle_rx_buffer(&self.device, buf);
                    }

                    break;
                }
            };
        }
    }
}
