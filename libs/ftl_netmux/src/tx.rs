use alloc::vec::Vec;

use ftl_types::error::ErrorCode;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::FiveTuple;
use ftl_types::net::IPPROTO_TCP;
use ftl_utils::reserve_slot::ReserveSlot;

use crate::BufReader;
use crate::NetMux;
use crate::NicId;
use crate::RxNotify;
use crate::device::DeviceId;
use crate::device::Tx;
use crate::packet::ipv4::Ipv4Addr;
use crate::packet::ipv4::Ipv4Inspector;
use crate::packet::ipv4::Ipv4Rewriter;
use crate::packet::ipv4::NetMask;
use crate::packet::tcp::TcpInspector;
use crate::packet::tcp::TcpRewriter;

pub struct Route {
    device: DeviceId,
    our_ip: Ipv4Addr,
    netmask: NetMask,
    #[allow(dead_code)]
    gateway_ip: Ipv4Addr,
    next_hop_ip: Ipv4Addr,
}

impl Route {
    pub fn new(
        device: DeviceId,
        our_ip: Ipv4Addr,
        netmask: NetMask,
        gateway_ip: Ipv4Addr,
        next_hop_ip: Ipv4Addr,
    ) -> Self {
        Self {
            device,
            our_ip,
            netmask,
            gateway_ip,
            next_hop_ip,
        }
    }
}

pub struct TxRouteTable {
    routes: Vec<Route>,
}

impl TxRouteTable {
    pub const fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn add_route(&mut self, route: Route) -> Result<(), ErrorCode> {
        self.routes
            .reserve_slot()
            .map_err(|_| ErrorCode::OutOfMemory)?
            .push(route);
        Ok(())
    }

    pub fn lookup(&self, dst_ip: Ipv4Addr) -> Option<(DeviceId, Ipv4Addr, Ipv4Addr)> {
        for route in &self.routes {
            if route.netmask.contains(dst_ip) {
                return Some((route.device, route.our_ip, route.next_hop_ip));
            }
        }

        None
    }

    pub fn lookup_exact(&self, dst_ip: Ipv4Addr) -> Option<(DeviceId, Ipv4Addr)> {
        for route in &self.routes {
            if route.our_ip == dst_ip {
                return Some((route.device, route.our_ip));
            }
        }

        None
    }
}

impl<'a, N: RxNotify> NetMux<'a, N> {
    pub(crate) fn prepare_tx(
        &self,
        nic_id: NicId,
        tx: &mut Tx<'a>,
        header: &mut dyn BufReader,
        payload: Option<&mut dyn BufReader>,
    ) -> Result<(DeviceId, Ipv4Addr, Ipv4Addr), ErrorCode> {
        let (header_buf, mut payload_buf) = tx.header_and_payload_bytes();

        // Read the header from the user buffer.
        header.read(header_buf)?;

        // Read the payload from the user buffer if exists.
        if let Some(dst) = payload_buf.as_mut() {
            payload.ok_or(ErrorCode::InvalidArg)?.read(dst)?;
        }

        // Parse the IPv4 header.
        let ipv4 = match Ipv4Inspector::new(header_buf) {
            Ok(ipv4) => ipv4,
            Err(e) => {
                ftl_driver::trace!(self.env, "invalid IPv4 header: {:?}", e);
                return Err(ErrorCode::InvalidArg);
            }
        };

        // The IPv4 total length must match the header + payload we were given.
        // TODO: Should we overwrite instead?
        let payload_len = payload_buf.as_ref().map(|p| p.len()).unwrap_or(0);
        if ipv4.total_len() != header_buf.len() + payload_len {
            return Err(ErrorCode::InvalidArg);
        }

        // Only TCP is supported for now.
        // TODO: Should we overwrite instead?
        if ipv4.ip_proto() != IPPROTO_TCP {
            return Err(ErrorCode::InvalidArg);
        }

        // Select the route from the destination in the IPv4 header.
        let dst_ip = ipv4.dst_ip();
        let ipv4_header_len = ipv4.header_len();
        let (device_id, our_ip, next_hop_ip) =
            self.tx.lookup(dst_ip).ok_or(ErrorCode::InvalidArg)?;

        // Parse the TCP header.
        let tcp_bytes = &mut header_buf[ipv4_header_len..];
        let tcp = match TcpInspector::new(tcp_bytes) {
            Ok(tcp) => tcp,
            Err(e) => {
                ftl_driver::trace!(self.env, "invalid TCP header: {:?}", e);
                return Err(ErrorCode::InvalidArg);
            }
        };

        let five_tuple = FiveTuple {
            eth_type: ETHTYPE_IPV4,
            ip_proto: IPPROTO_TCP,
            local_ip: our_ip.as_u32(),
            local_port: tcp.src_port(),
            remote_ip: dst_ip.as_u32(),
            remote_port: tcp.dst_port(),
        };

        // Check if this network owns the five-tuple.
        if self.rx.find_nic_by_five_tuple(five_tuple) != Some(nic_id) {
            return Err(ErrorCode::NotAllowed);
        }

        let mut tcp = match TcpRewriter::new(tcp_bytes) {
            Ok(tcp) => tcp,
            Err(e) => {
                ftl_driver::trace!(self.env, "invalid TCP header: {:?}", e);
                return Err(ErrorCode::InvalidArg);
            }
        };

        // Update the TCP header.
        tcp.update_checksum(our_ip, dst_ip, payload_buf.as_deref());

        // Update the IPv4 header.
        let mut ipv4 = match Ipv4Rewriter::new(header_buf) {
            Ok(ipv4) => ipv4,
            Err(e) => {
                ftl_driver::trace!(self.env, "invalid IPv4 header: {:?}", e);
                return Err(ErrorCode::InvalidArg);
            }
        };
        ipv4.set_src_ip(our_ip);
        ipv4.update_checksum();

        Ok((device_id, our_ip, next_hop_ip))
    }
}
