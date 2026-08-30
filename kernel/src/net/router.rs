use alloc::vec::Vec;

use ftl_driver::dma::DmaBuf;
use ftl_driver::net::Error;
use ftl_types::error::ErrorCode;

use super::device::Device;
use super::network::Network;
use super::network::Rx;
use super::packet::Ipv4Addr;
use super::packet::Ipv4Inspector;
use crate::net::GLOBAL_ENV;
use crate::shared_ref::SharedRef;

const ETHERNET_HEADER_LEN: usize = 14;
const ARP_PACKET_LEN: usize = 28;
const ETHTYPE_IPV4: u16 = 0x0800;
const ETHTYPE_ARP: u16 = 0x0806;
const OUR_IP: Ipv4Addr = Ipv4Addr::new(0x0a00_020f);

pub struct Router {
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

    fn find_network(
        &self,
        local_ip: Ipv4Addr,
        local_port: u16,
        remote_ip: Ipv4Addr,
        remote_port: u16,
    ) -> Option<(SharedRef<Network>, u64)> {
        let mut best = None;
        for network in &self.networks {
            let Some((specificity, cookie)) =
                network.match_packet(local_ip, local_port, remote_ip, remote_port)
            else {
                continue;
            };
            if best
                .as_ref()
                .is_none_or(|(best_specificity, _, _)| specificity > *best_specificity)
            {
                best = Some((specificity, network.clone(), cookie));
            }
        }
        best.map(|(_, network, cookie)| (network, cookie))
    }

    pub fn device(&self) -> SharedRef<Device> {
        self.device.clone()
    }

    fn recycle(&self, buf: DmaBuf) {
        let driver = self.device.driver();
        if driver.provide(&GLOBAL_ENV, buf).is_err() {
            warn!("net: failed to recycle an RX buffer");
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

    fn handle_frame(&self, buf: DmaBuf, headroom: usize, frame_len: usize) {
        if frame_len < ETHERNET_HEADER_LEN {
            self.recycle(buf);
            return;
        }

        let frame = &buf.as_slice()[headroom..headroom + frame_len];
        let src_mac = frame[6..12].try_into().unwrap();
        let eth_type = u16::from_be_bytes(frame[12..14].try_into().unwrap());
        if eth_type == ETHTYPE_ARP {
            self.handle_arp(frame, src_mac);
            self.recycle(buf);
            return;
        }
        if eth_type != ETHTYPE_IPV4 {
            self.recycle(buf);
            return;
        }

        let packet_offset = headroom + ETHERNET_HEADER_LEN;
        let packet_len = frame_len - ETHERNET_HEADER_LEN;
        let packet = &buf.as_slice()[packet_offset..packet_offset + packet_len];
        let inspector = match Ipv4Inspector::new_tcp_packet(packet) {
            Ok(inspector) => inspector,
            Err(_) => {
                self.recycle(buf);
                return;
            }
        };
        let local_ip = inspector.dst_ip();
        let local_port = inspector.dst_port();
        let remote_ip = inspector.src_ip();
        let remote_port = inspector.src_port();
        let Some((network, cookie)) =
            self.find_network(local_ip, local_port, remote_ip, remote_port)
        else {
            self.recycle(buf);
            return;
        };

        self.device.learn_arp(&GLOBAL_ENV, remote_ip, src_mac);

        let packet_len = inspector.packet_len();
        let header_len = inspector.header_len();
        let rx = Rx {
            buf,
            packet_offset,
            packet_len,
            header_len,
            cookie,
        };
        network.enqueue_rx(rx);
    }

    pub fn handle_interrupt(&self) {
        let driver = self.device.driver();
        driver.handle_interrupt(&GLOBAL_ENV);

        loop {
            let rx = match driver.try_receive() {
                Ok(rx) => rx,
                Err((Error::RxEmpty, _)) => break,
                Err((error, buf)) => {
                    warn!("net: failed to receive packet: {:?}", error);
                    if let Some(buf) = buf {
                        self.recycle(buf);
                    }
                    break;
                }
            };
            let (buf, headroom, frame_len) = rx;
            self.handle_frame(buf, headroom, frame_len);
        }
    }
}
