use alloc::vec::Vec;
use core::ops::Deref;

use ftl_driver::dma::DmaBuf;
use ftl_driver::net::Error;
use ftl_types::error::ErrorCode;
use ftl_types::net::ETHTYPE_ARP;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::FiveTuple;

use super::device::Device;
use super::network::Network;
use super::packet::Ipv4Addr;
use super::packet::Ipv4Inspector;
use crate::net::GLOBAL_ENV;
use crate::shared_ref::SharedRef;

const ETHERNET_HEADER_LEN: usize = 14;
const ARP_PACKET_LEN: usize = 28;
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
        if frame_len < ETHERNET_HEADER_LEN {
            return;
        }

        let frame = &buf.as_slice()[headroom..headroom + frame_len];
        let src_mac = frame[6..12].try_into().unwrap();
        let eth_type = u16::from_be_bytes(frame[12..14].try_into().unwrap());

        match eth_type {
            ETHTYPE_ARP => self.handle_arp(frame, src_mac),
            ETHTYPE_IPV4 => self.handle_ipv4(src_mac, buf, headroom, frame_len),
            _ => {
                trace!("unsupported ethernet type: {:x}", eth_type);
            }
        }
    }

    fn handle_arp(&self, frame: &[u8], src_mac: [u8; 6]) {
        let expected_len = ETHERNET_HEADER_LEN + ARP_PACKET_LEN;
        if frame.len() < expected_len {
            return;
        }

        let operation = u16::from_be_bytes(frame[20..22].try_into().unwrap());
        let sender_ip = u32::from_be_bytes(frame[28..32].try_into().unwrap());
        let target_ip = u32::from_be_bytes(frame[38..42].try_into().unwrap());
        let sender_ip = Ipv4Addr::new(sender_ip);
        let target_ip = Ipv4Addr::new(target_ip);

        self.device.learn_arp(&GLOBAL_ENV, sender_ip, src_mac);
        if operation != 1 || target_ip != OUR_IP {
            return;
        }

        self.device
            .send_arp_reply(&GLOBAL_ENV, src_mac, sender_ip, OUR_IP);
    }

    fn handle_ipv4(&self, src_mac: [u8; 6], buf: RxDmaBuf<'_>, headroom: usize, frame_len: usize) {
        let packet_offset = headroom + ETHERNET_HEADER_LEN;
        let packet_len = frame_len - ETHERNET_HEADER_LEN;
        let packet = &buf.as_slice()[packet_offset..packet_offset + packet_len];
        let inspector = match Ipv4Inspector::new_tcp_packet(packet) {
            Ok(inspector) => inspector,
            Err(e) => {
                trace!("failed to inspect IPv4 packet: {:?}", e);
                return;
            }
        };

        let local_ip = inspector.dst_ip();
        let local_port = inspector.dst_port();
        let remote_ip = inspector.src_ip();
        let remote_port = inspector.src_port();
        let proto = inspector.ip_proto();
        let five_tuple = FiveTuple {
            eth_type: ETHTYPE_IPV4,
            ip_proto: proto,
            local_ip: local_ip.as_u32(),
            local_port,
            remote_ip: remote_ip.as_u32(),
            remote_port,
        };

        let Some((network, cookie)) = self.find_network(five_tuple) else {
            return;
        };

        self.device.learn_arp(&GLOBAL_ENV, remote_ip, src_mac);

        let packet_len = inspector.packet_len();
        let header_len = inspector.header_len();
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
