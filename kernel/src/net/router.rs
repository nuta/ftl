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
use super::dhcp;
use super::dhcp::DhcpConfig;
use super::network::Network;
use super::packet::arp::ARP_OP_REQUEST;
use super::packet::arp::ArpInspector;
use super::packet::dhcp::DHCP_CLIENT_PORT;
use super::packet::dhcp::DHCP_SERVER_PORT;
use super::packet::ethernet::ETHERNET_HEADER_LEN;
use super::packet::ethernet::EthernetInspector;
use super::packet::ipv4::Ipv4Inspector;
use super::packet::ipv4::NetMask;
use super::packet::tcp::TcpInspector;
use super::packet::udp::IPPROTO_UDP;
use super::packet::udp::UdpInspector;
use super::route_table::Route;
use super::route_table::RouteTable;
use crate::net::GLOBAL_ENV;
use crate::shared_ref::SharedRef;

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

    pub fn take(mut self) -> (SharedRef<Device>, DmaBuf) {
        (self.device.clone(), self.buf.take().unwrap())
    }
}

impl<'a> Deref for RxDmaBuf<'a> {
    type Target = DmaBuf;

    fn deref(&self) -> &Self::Target {
        self.buf.as_ref().unwrap()
    }
}

impl<'a> Drop for RxDmaBuf<'a> {
    fn drop(&mut self) {
        if let Some(buf) = self.buf.take() {
            self.device.recycle_rx_buffer(buf);
        }
    }
}

pub struct Router {
    route_table: SharedRef<RouteTable>,
    networks: Vec<SharedRef<Network>>,
}

impl Router {
    pub fn new(route_table: SharedRef<RouteTable>) -> Self {
        Self {
            route_table,
            networks: Vec::new(),
        }
    }

    pub fn add_network(&mut self, network: SharedRef<Network>) -> Result<(), ErrorCode> {
        self.networks
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;
        self.networks.push(network);
        Ok(())
    }

    fn find_network(&self, five_tuple: FiveTuple) -> Option<SharedRef<Network>> {
        // TODO: 5-tuple hash map to avoid scanning all networks.
        for network in &self.networks {
            if network.matches(five_tuple) {
                return Some(network.clone());
            }
        }

        None
    }

    pub fn route_table(&self) -> &SharedRef<RouteTable> {
        &self.route_table
    }

    fn handle_eth_frame(
        &self,
        device: &SharedRef<Device>,
        buf: DmaBuf,
        headroom: usize,
        frame_len: usize,
    ) {
        let buf = RxDmaBuf::new(device, buf);

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
        let eth_type = frame.eth_type();
        match eth_type {
            ETHTYPE_ARP => self.handle_arp(device, frame.payload()),
            ETHTYPE_IPV4 => self.handle_ipv4(device, frame.src_mac(), buf, headroom, frame_len),
            _ => {
                trace!("unsupported ethernet type: {:x}", eth_type);
            }
        }
    }

    fn handle_arp(&self, device: &SharedRef<Device>, packet: &[u8]) {
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
        device.learn_arp(&GLOBAL_ENV, src_ip, src_mac);

        // Reply to the ARP request if it is for one of our route addresses.
        if arp.op() == ARP_OP_REQUEST {
            let Some(route) = self.route_table.lookup_exact(arp.dst_ip()) else {
                return;
            };

            route
                .device()
                .send_arp_reply(&GLOBAL_ENV, src_mac, src_ip, route.our_ip());
        }
    }

    fn handle_ipv4(
        &self,
        device: &SharedRef<Device>,
        src_mac: [u8; 6],
        buf: RxDmaBuf<'_>,
        headroom: usize,
        frame_len: usize,
    ) {
        let off = headroom + ETHERNET_HEADER_LEN;
        let len = frame_len - ETHERNET_HEADER_LEN;
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

        device.learn_arp(&GLOBAL_ENV, ipv4.src_ip(), src_mac);

        // Forward to the upper layer, and get the routing key.
        let five_tuple = match ipv4.ip_proto() {
            IPPROTO_TCP => self.handle_tcp(&ipv4),
            IPPROTO_UDP => {
                self.handle_udp(buf.device, &ipv4);
                return;
            }
            _ => {
                trace!("unsupported IP protocol: {:x}", ipv4.ip_proto());
                return;
            }
        };

        if let Some((five_tuple, trans_header_len)) = five_tuple {
            // Find the network to forward the packet to.
            let Some(network) = self.find_network(five_tuple) else {
                return;
            };

            // Forward the packet to the network.
            let total_len = ipv4.total_len();
            let header_len = ipv4.header_len() + trans_header_len;
            let (device, buf) = buf.take();
            network.receive(device, buf, off, total_len, header_len);
        }
    }

    fn handle_tcp(&self, ipv4: &Ipv4Inspector<'_>) -> Option<(FiveTuple, usize)> {
        // Parse the TCP header.
        let tcp = match TcpInspector::new(ipv4.payload()) {
            Ok(tcp) => tcp,
            Err(e) => {
                trace!("failed to inspect TCP segment: {:?}", e);
                return None;
            }
        };

        // Check if the TCP header is valid.
        if let Err(e) = tcp.validate(ipv4) {
            trace!("failed to validate TCP segment: {:?}", e);
            return None;
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

        Some((five_tuple, tcp.header_len()))
    }

    fn handle_udp(&self, device: &SharedRef<Device>, ipv4: &Ipv4Inspector<'_>) {
        let udp = match UdpInspector::new(ipv4.payload()) {
            Ok(udp) => udp,
            Err(e) => {
                trace!("failed to inspect UDP datagram: {:?}", e);
                return;
            }
        };

        if let Err(e) = udp.validate(ipv4) {
            trace!("failed to validate UDP datagram: {:?}", e);
            return;
        }

        if udp.src_port() == DHCP_SERVER_PORT && udp.dst_port() == DHCP_CLIENT_PORT {
            if let Some(config) = dhcp::handle_rx(device, ipv4, &udp) {
                match self.add_dhcp_route(device, &config) {
                    Ok(()) => {
                        info!(
                            "DHCP configured: address {}, gateway {}, netmask {}",
                            config.address, config.gateway, config.netmask,
                        );
                    }
                    Err(error) => {
                        warn!("failed to add DHCP route: {:?}", error);
                    }
                }
            }

            return;
        }

        // TODO: Forward to a UDP socket.
    }

    fn add_dhcp_route(
        &self,
        device: &SharedRef<Device>,
        config: &DhcpConfig,
    ) -> Result<(), ErrorCode> {
        let route = SharedRef::new(Route::new(
            device.clone(),
            config.address,
            NetMask::new(0),
            config.gateway,
            config.gateway,
        ))?;

        self.route_table.add_route(route)?;
        Ok(())
    }

    pub fn handle_interrupt(&self, device: &SharedRef<Device>) {
        // Do driver's interrupt work.
        let driver = device.driver();
        driver.handle_interrupt(&GLOBAL_ENV);

        // Process pending RX packets.
        loop {
            match driver.try_receive(&GLOBAL_ENV) {
                Ok((buf, headroom, frame_len)) => {
                    self.handle_eth_frame(device, buf, headroom, frame_len);
                }
                Err((error, buf)) => {
                    if error != Error::RxEmpty {
                        // Something went wrong.
                        warn!("failed to receive packet: {:?}", error);
                        if let Some(buf) = buf {
                            device.recycle_rx_buffer(buf);
                        }
                    }

                    break;
                }
            };
        }
    }
}
