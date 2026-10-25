use ftl_driver::dma::DmaBuf;
use ftl_driver::dma::DmaBufWithDrop;
use ftl_driver::env::Env;
use ftl_driver::info;
use ftl_driver::net::Driver;
use ftl_driver::net::Error;
use ftl_driver::warn;
use ftl_types::net::ETHTYPE_ARP;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::IPPROTO_TCP;
use ftl_types::net::IPPROTO_UDP;
use ftl_types::time::MonoTime;

use crate::PollNotifier;
use crate::mux::ExternalIpConfig;
use crate::mux::PhysicalState;
use crate::mux::RxNotify;
use crate::nic::Nic;
use crate::packet::arp::ARP_OP_REQUEST;
use crate::packet::arp::ArpInspector;
use crate::packet::dhcp::DHCP_CLIENT_PORT;
use crate::packet::dhcp::DHCP_SERVER_PORT;
use crate::packet::ethernet::ETHERNET_HEADER_LEN;
use crate::packet::ethernet::EthernetInspector;
use crate::packet::ipv4::Ipv4Inspector;
use crate::packet::tcp::TcpInspector;
use crate::packet::udp::UDP_HEADER_LEN;
use crate::packet::udp::UdpInspector;

impl<'a> PhysicalState<'a> {
    fn handle_eth_frame<N: RxNotify>(
        &mut self,
        pnic: &Nic<N>,
        buf: DmaBuf,
        headroom: usize,
        frame_len: usize,
        now: MonoTime,
    ) {
        let buf = DmaBufWithDrop::new(self.env, buf);

        // Device driver might return a bogus frame length.
        let Some(end) = headroom.checked_add(frame_len) else {
            ftl_driver::trace!(self.env, "ethernet frame length overflow");
            return;
        };

        // Make sure the slice is in bounds.
        let Some(frame) = buf.as_slice().get(headroom..end) else {
            ftl_driver::trace!(self.env, "ethernet frame out of bounds");
            return;
        };

        // Parse the Ethernet frame.
        let frame = match EthernetInspector::new(frame) {
            Ok(frame) => frame,
            Err(e) => {
                ftl_driver::trace!(self.env, "failed to inspect Ethernet frame: {:?}", e);
                return;
            }
        };

        // Forward to the upper layer.
        let eth_type = frame.eth_type();
        match eth_type {
            ETHTYPE_ARP => self.handle_arp(frame.payload()),
            ETHTYPE_IPV4 => {
                self.handle_ipv4(pnic, frame.src_mac(), buf, headroom, frame_len, now)
            }
            _ => {
                ftl_driver::trace!(self.env, "unsupported ethernet type: {:x}", eth_type);
            }
        }
    }

    fn handle_arp(&self, packet: &[u8]) {
        let arp = match ArpInspector::new(packet) {
            Ok(arp) => arp,
            Err(e) => {
                ftl_driver::trace!(self.env, "failed to inspect ARP packet: {:?}", e);
                return;
            }
        };

        let src_mac = arp.src_mac();
        let src_ip = arp.src_ip();

        // Learn the sender's MAC address.
        self.device.learn_arp(src_ip, src_mac);

        // Reply to the ARP request if it is for one of our route addresses.
        if arp.op() != ARP_OP_REQUEST {
            return;
        }

        let Some(external) = self.external else {
            return;
        };
        if arp.dst_ip() != external.address {
            return;
        }

        self.device
            .send_arp_reply(src_mac, src_ip, external.address);
    }

    fn handle_ipv4<N: RxNotify>(
        &mut self,
        pnic: &Nic<N>,
        src_mac: [u8; 6],
        buf: DmaBufWithDrop<'a>,
        headroom: usize,
        frame_len: usize,
        now: MonoTime,
    ) {
        let off = headroom + ETHERNET_HEADER_LEN;
        let len = frame_len - ETHERNET_HEADER_LEN;
        let packet = &buf.as_slice()[off..off + len];

        // Parse the IPv4 packet.
        let ipv4 = match Ipv4Inspector::new(packet) {
            Ok(ipv4) => ipv4,
            Err(e) => {
                ftl_driver::trace!(self.env, "failed to inspect IPv4 packet: {:?}", e);
                return;
            }
        };

        // Check if the IPv4 packet is valid.
        if let Err(e) = ipv4.validate() {
            ftl_driver::trace!(self.env, "failed to validate IPv4 packet: {:?}", e);
            return;
        }

        self.device.learn_arp(ipv4.src_ip(), src_mac);

        // Forward to the upper layer, and get the routing key.
        let ports = match ipv4.ip_proto() {
            IPPROTO_TCP => self.handle_tcp(&ipv4),
            IPPROTO_UDP => self.handle_udp(&ipv4, now),
            _ => {
                ftl_driver::trace!(self.env, "unsupported IP protocol: {:x}", ipv4.ip_proto());
                return;
            }
        };

        // Find the NIC to forward the packet to.
        let Some((local_port, remote_port, trans_header_len)) = ports else {
            return;
        };
        let Some(external) = self.external else {
            return;
        };
        if ipv4.dst_ip() != external.address {
            return;
        }
        let Some(nic) = pnic.lookup(local_port) else {
            return;
        };
        if !nic.routes_back_to(remote_port, pnic) {
            return;
        }

        // Build a RX packet.
        let total_len = ipv4.total_len();
        let header_len = ipv4.header_len() + trans_header_len;

        // Forward the packet to the NIC.
        nic.receive(buf.take(), off, total_len, header_len);
    }

    fn handle_udp(
        &mut self,
        ipv4: &Ipv4Inspector<'_>,
        now: MonoTime,
    ) -> Option<(u16, u16, usize)> {
        let udp = match UdpInspector::new(ipv4.payload()) {
            Ok(udp) => udp,
            Err(e) => {
                ftl_driver::trace!(self.env, "failed to inspect UDP datagram: {:?}", e);
                return None;
            }
        };

        if let Err(e) = udp.validate(ipv4) {
            ftl_driver::trace!(self.env, "failed to validate UDP datagram: {:?}", e);
            return None;
        }

        if udp.src_port() == DHCP_SERVER_PORT && udp.dst_port() == DHCP_CLIENT_PORT {
            self.handle_dhcp(ipv4, &udp, now);
            return None;
        }

        Some((udp.dst_port(), udp.src_port(), UDP_HEADER_LEN))
    }

    fn handle_dhcp(
        &mut self,
        ipv4: &Ipv4Inspector<'_>,
        udp: &UdpInspector<'_>,
        now: MonoTime,
    ) {
        let Some(config) = self.device.handle_dhcp_rx(ipv4, udp, now) else {
            return;
        };

        self.external = Some(ExternalIpConfig {
            address: config.address,
            netmask: config.netmask,
            gateway: config.gateway,
        });

        // TODO: Move this to kernel crate.
        info!(
            self.env,
            "DHCP configured: address {}, gateway {}, netmask {}",
            config.address,
            config.gateway,
            config.netmask,
        );
    }

    fn handle_tcp(&self, ipv4: &Ipv4Inspector<'_>) -> Option<(u16, u16, usize)> {
        // Parse the TCP header.
        let tcp = match TcpInspector::new(ipv4.payload()) {
            Ok(tcp) => tcp,
            Err(e) => {
                ftl_driver::trace!(self.env, "failed to inspect TCP segment: {:?}", e);
                return None;
            }
        };

        // Check if the TCP header is valid.
        if let Err(e) = tcp.validate(ipv4) {
            ftl_driver::trace!(self.env, "failed to validate TCP segment: {:?}", e);
            return None;
        }

        Some((tcp.dst_port(), tcp.src_port(), tcp.header_len()))
    }

    pub fn poll<N: RxNotify>(&mut self, pnic: &Nic<N>, irq: Option<u8>, now: MonoTime) {
        if irq.is_some() && irq != Some(self.device.irq()) {
            return;
        }

        self.device.driver().handle_interrupt(self.env);
        while let Some((buf, headroom, frame_len)) = self.receive() {
            self.handle_eth_frame(pnic, buf, headroom, frame_len, now);
        }

        self.device.poll(now);
    }

    fn receive(&self) -> Option<(DmaBuf, usize, usize)> {
        loop {
            let driver = self.device.driver();
            match driver.try_receive(self.env) {
                Ok((buf, headroom, frame_len)) => {
                    self.provide_rx_buffers(self.env, driver, 1);
                    return Some((buf, headroom, frame_len));
                }
                Err(Error::RxEmpty) => return None,
                Err(error) => {
                    warn!(self.env, "failed to receive packet: {:?}", error);
                    self.provide_rx_buffers(self.env, driver, 1);
                }
            }
        }
    }

    pub(crate) fn provide_rx_buffers(
        &self,
        env: &dyn Env,
        driver: &dyn Driver<Notifier = PollNotifier>,
        max_count: usize,
    ) {
        for _ in 0..max_count {
            // TODO: What if the allocation fails? Should we call this function periodically?
            match env.alloc_dma(self.rx_buffer_size) {
                Ok(buf) => {
                    if let Err((e, buf)) = driver.provide(env, buf) {
                        if e != Error::RxFull {
                            warn!(env, "failed to provide RX buffer: {:?}", e);
                        }

                        env.free_dma(buf);
                        break;
                    }
                }
                Err(err) => {
                    warn!(env, "failed to allocate RX buffer: {:?}", err);
                    break;
                }
            }
        }
    }
}
