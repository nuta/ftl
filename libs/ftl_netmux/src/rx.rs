use ftl_driver::dma::DmaBuf;
use ftl_driver::dma::DmaBufWithDrop;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_driver::net::Error;
use ftl_types::error::ErrorCode;
use ftl_types::net::ETHTYPE_ARP;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::FiveTuple;
use ftl_types::net::IPPROTO_TCP;
use ftl_types::net::IPPROTO_UDP;
use ftl_types::net::Rule;
use ftl_utils::fxhash::FxHashMap;
use ftl_utils::fxhash::FxHashSet;

use crate::NetMux;
use crate::PollNotifier;
use crate::device::DeviceId;
use crate::dhcp::DhcpConfig;
use crate::mux::RxNotify;
use crate::nic::Nic;
use crate::nic::NicId;
use crate::nic::RxPacket;
use crate::packet::arp::ARP_OP_REQUEST;
use crate::packet::arp::ArpInspector;
use crate::packet::dhcp::DHCP_CLIENT_PORT;
use crate::packet::dhcp::DHCP_SERVER_PORT;
use crate::packet::ethernet::ETHERNET_HEADER_LEN;
use crate::packet::ethernet::EthernetInspector;
use crate::packet::ipv4::Ipv4Inspector;
use crate::packet::ipv4::NetMask;
use crate::packet::tcp::TcpInspector;
use crate::packet::udp::UdpInspector;
use crate::tx::Route;

struct PortBinding {
    nic_id: NicId,
    rules: FxHashSet<Rule>,
}

pub struct RxRouteTable<'a, N> {
    next_nic_id: u32,
    nics: FxHashMap<NicId, Nic<'a, N>>,
    bindings: FxHashMap<u16, PortBinding>,
}

impl<'a, N: RxNotify> RxRouteTable<'a, N> {
    pub const fn new() -> Self {
        Self {
            next_nic_id: 0,
            nics: FxHashMap::new(),
            bindings: FxHashMap::new(),
        }
    }

    fn alloc_nic_id(&mut self) -> Result<NicId, ErrorCode> {
        for _ in 0..64 {
            self.next_nic_id = self.next_nic_id.wrapping_add(1);
            let id = NicId::new(self.next_nic_id);
            if !self.nics.contains_key(&id) {
                return Ok(id);
            }
        }

        Err(ErrorCode::OutOfBounds)
    }

    pub fn add_nic(&mut self) -> Result<NicId, ErrorCode> {
        self.nics
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;

        let id = self.alloc_nic_id()?;
        self.nics.insert(id, Nic::new());
        Ok(id)
    }

    pub fn get_nic(&self, id: NicId) -> Option<&Nic<'a, N>> {
        self.nics.get(&id)
    }

    pub fn remove_nic(&mut self, id: NicId) -> Option<Nic<'a, N>> {
        self.bindings.retain(|_, binding| binding.nic_id != id);
        self.nics.remove(&id)
    }

    pub fn bind(&mut self, nic_id: NicId, rule: Rule) -> Result<(), ErrorCode> {
        let local_port = rule.local_port().ok_or(ErrorCode::InvalidArg)?.get();
        if !self.nics.contains_key(&nic_id) {
            return Err(ErrorCode::NotFound);
        }

        if let Some(binding) = self.bindings.get_mut(&local_port) {
            if binding.nic_id != nic_id {
                // The port is bound to a different network.
                return Err(ErrorCode::AlreadyExists);
            }

            // Reject if the same rule already exists.
            if binding.rules.contains(&rule) {
                return Err(ErrorCode::AlreadyExists);
            }

            // Add the rule to the binding.
            binding
                .rules
                .try_reserve(1)
                .map_err(|_| ErrorCode::OutOfMemory)?;
            binding.rules.insert(rule);
            return Ok(());
        }

        // Create a new binding for the port.
        let mut rules = FxHashSet::new();
        rules.try_reserve(1).map_err(|_| ErrorCode::OutOfMemory)?;
        rules.insert(rule);

        self.bindings
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;
        self.bindings
            .insert(local_port, PortBinding { nic_id, rules });
        Ok(())
    }

    pub fn unbind(&mut self, nic_id: NicId, rule: &Rule) -> Result<(), ErrorCode> {
        let port = rule.local_port().ok_or(ErrorCode::InvalidArg)?.get();
        let binding = self.bindings.get_mut(&port).ok_or(ErrorCode::NotFound)?;
        if binding.nic_id != nic_id {
            return Err(ErrorCode::NotFound);
        }

        if !binding.rules.remove(rule) {
            return Err(ErrorCode::NotFound);
        }

        if binding.rules.is_empty() {
            self.bindings.remove(&port);
        }
        Ok(())
    }

    pub fn find_nic_by_five_tuple(&self, five_tuple: FiveTuple) -> Option<NicId> {
        let binding = self.bindings.get(&five_tuple.local_port)?;
        for rule in five_tuple.matchers() {
            if binding.rules.contains(&rule) {
                return Some(binding.nic_id);
            }
        }
        None
    }
}

impl<'a, N: RxNotify> NetMux<'a, N> {
    fn handle_eth_frame(
        &mut self,
        device_id: DeviceId,
        buf: DmaBuf,
        headroom: usize,
        frame_len: usize,
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
            ETHTYPE_ARP => self.handle_arp(device_id, frame.payload()),
            ETHTYPE_IPV4 => self.handle_ipv4(device_id, frame.src_mac(), buf, headroom, frame_len),
            _ => {
                ftl_driver::trace!(self.env, "unsupported ethernet type: {:x}", eth_type);
            }
        }
    }

    fn handle_arp(&mut self, device_id: DeviceId, packet: &[u8]) {
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
        let Some(device) = self.devices.get(&device_id) else {
            return;
        };
        device.learn_arp(src_ip, src_mac);

        // Reply to the ARP request if it is for one of our route addresses.
        if arp.op() != ARP_OP_REQUEST {
            return;
        }

        // Look up the route to send the ARP reply to.
        let Some((device_id, our_ip)) = self.tx.lookup_exact(arp.dst_ip()) else {
            return;
        };

        let Some(device) = self.devices.get(&device_id) else {
            return;
        };
        device.send_arp_reply(src_mac, src_ip, our_ip);
    }

    fn handle_ipv4(
        &mut self,
        device_id: DeviceId,
        src_mac: [u8; 6],
        buf: DmaBufWithDrop<'a>,
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
                ftl_driver::trace!(self.env, "failed to inspect IPv4 packet: {:?}", e);
                return;
            }
        };

        // Check if the IPv4 packet is valid.
        if let Err(e) = ipv4.validate() {
            ftl_driver::trace!(self.env, "failed to validate IPv4 packet: {:?}", e);
            return;
        }

        {
            let Some(device) = self.devices.get(&device_id) else {
                return;
            };
            device.learn_arp(ipv4.src_ip(), src_mac);
        }

        // Forward to the upper layer, and get the routing key.
        let five_tuple = match ipv4.ip_proto() {
            IPPROTO_TCP => self.handle_tcp(&ipv4),
            IPPROTO_UDP => {
                self.handle_udp(device_id, &ipv4);
                return;
            }
            _ => {
                ftl_driver::trace!(self.env, "unsupported IP protocol: {:x}", ipv4.ip_proto());
                return;
            }
        };

        // Find the NIC to forward the packet to.
        let Some((five_tuple, trans_header_len)) = five_tuple else {
            return;
        };
        let Some(nic_id) = self.rx.find_nic_by_five_tuple(five_tuple) else {
            return;
        };
        let Some(nic) = self.rx.get_nic(nic_id) else {
            return;
        };

        // Build a RX packet.
        let total_len = ipv4.total_len();
        let header_len = ipv4.header_len() + trans_header_len;

        // Forward the packet to the NIC.
        let packet = RxPacket::new(self.env, buf.take(), off, total_len, header_len);
        nic.receive(packet);
    }

    fn handle_udp(&mut self, device_id: DeviceId, ipv4: &Ipv4Inspector<'_>) {
        let udp = match UdpInspector::new(ipv4.payload()) {
            Ok(udp) => udp,
            Err(e) => {
                ftl_driver::trace!(self.env, "failed to inspect UDP datagram: {:?}", e);
                return;
            }
        };

        if let Err(e) = udp.validate(ipv4) {
            ftl_driver::trace!(self.env, "failed to validate UDP datagram: {:?}", e);
            return;
        }

        if udp.src_port() != DHCP_SERVER_PORT || udp.dst_port() != DHCP_CLIENT_PORT {
            return;
        }

        let Some(config) = self
            .devices
            .get_mut(&device_id)
            .and_then(|device| device.handle_dhcp_rx(ipv4, &udp))
        else {
            return;
        };

        if let Err(error) = self.add_dhcp_route(device_id, &config) {
            ftl_driver::warn!(self.env, "failed to add DHCP route: {:?}", error);
            return;
        }

        // TODO: Move this to kernel crate.
        ftl_driver::info!(
            self.env,
            "DHCP configured: address {}, gateway {}, netmask {}",
            config.address,
            config.gateway,
            config.netmask,
        );
    }

    fn handle_tcp(&self, ipv4: &Ipv4Inspector<'_>) -> Option<(FiveTuple, usize)> {
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

    fn add_dhcp_route(
        &mut self,
        device_id: DeviceId,
        config: &DhcpConfig,
    ) -> Result<(), ErrorCode> {
        let route = Route::new(
            device_id,
            config.address,
            NetMask::new(0),
            config.gateway,
            config.gateway,
        );

        self.tx.add_route(route)?;
        Ok(())
    }

    pub fn handle_interrupt(&mut self, device_id: DeviceId) {
        let Some(device) = self.devices.get(&device_id) else {
            return;
        };
        let driver = device.driver();
        let env = self.env;

        // Do driver's interrupt work.
        driver.handle_interrupt(env);

        // Process pending RX packets.
        let mut num_popped = 0;
        loop {
            match driver.try_receive(env) {
                Ok((buf, headroom, frame_len)) => {
                    num_popped += 1;
                    self.handle_eth_frame(device_id, buf, headroom, frame_len);
                }
                Err(error) => {
                    if error != Error::RxEmpty {
                        // Something went wrong.
                        ftl_driver::warn!(env, "failed to receive packet: {:?}", error);
                        num_popped += 1;
                    }

                    break;
                }
            }
        }

        self.provide_rx_buffers(env, driver, num_popped);
    }

    pub(crate) fn provide_rx_buffers(
        &mut self,
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
                            ftl_driver::warn!(env, "failed to provide RX buffer: {:?}", e);
                        }

                        env.free_dma(buf);
                        break;
                    }
                }
                Err(err) => {
                    ftl_driver::warn!(env, "failed to allocate RX buffer: {:?}", err);
                    break;
                }
            }
        }
    }
}
