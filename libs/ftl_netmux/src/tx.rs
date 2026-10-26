use alloc::sync::Arc;

use ftl_types::error::ErrorCode;
use ftl_types::nic::IP_PROTO_TCP;
use ftl_types::nic::IP_PROTO_UDP;
use ftl_types::nic::PROTO_IPV4;
use ftl_types::nic::PROTO_TCP;
use ftl_types::nic::PROTO_UDP;

use crate::BufReader;
use crate::RxNotify;
use crate::device::Tx;
use crate::nic::Nic;
use crate::packet::ipv4::Ipv4Addr;
use crate::packet::ipv4::Ipv4Inspector;
use crate::packet::ipv4::Ipv4Rewriter;
use crate::packet::tcp::TcpInspector;
use crate::packet::tcp::TcpRewriter;
use crate::packet::udp::UDP_HEADER_LEN;
use crate::packet::udp::UdpRewriter;

const MAX_HEADER_LEN: usize = 128; // IPv4 header + TCP/UDP header
const MAX_PAYLOAD_LEN: usize = 2048;

/// The destination of a packet.
enum Via<N> {
    /// To an external network, via a physical NIC such as virtio-net.
    External {
        nic: Arc<Nic<N>>,
        our_ip: Ipv4Addr,
        next_hop_ip: Ipv4Addr,
    },
    /// To a local virtual NIC.
    Local(Arc<Nic<N>>),
}

impl<N: RxNotify> Nic<N> {
    /// Sends a packet to the network.
    pub fn send(
        &self,
        header: &mut dyn BufReader,
        payload: Option<&mut dyn BufReader>,
    ) -> Result<(), ErrorCode> {
        let payload_len = if let Some(payload) = payload.as_ref() {
            payload.len()
        } else {
            0
        };

        if header.len() > MAX_HEADER_LEN {
            return Err(ErrorCode::PacketTooLong);
        }

        if payload_len > MAX_PAYLOAD_LEN {
            return Err(ErrorCode::PacketTooLong);
        }

        let header_len = header.len();
        let mut tx = Tx::alloc(self.env, header_len, payload_len)?;

        match self.prepare_tx(&mut tx, header, payload)? {
            Via::External {
                nic: pnic,
                our_ip,
                next_hop_ip,
            } => {
                let Some(physical) = &pnic.physical else {
                    return Err(ErrorCode::NotFound);
                };

                // Send the packet through the route's next hop.
                physical.lock().device.send_ipv4(our_ip, next_hop_ip, tx)
            }
            Via::Local(dst) => {
                // Send the packet to the vNIC directly.
                let (header_buf, header_offset, payload_buf) = tx.into_bufs();
                dst.receive(
                    header_buf,
                    header_offset,
                    header_len + payload_len,
                    header_len,
                    payload_buf,
                );
                Ok(())
            }
        }
    }

    fn prepare_tx(
        &self,
        tx: &mut Tx<'static>,
        header: &mut dyn BufReader,
        payload: Option<&mut dyn BufReader>,
    ) -> Result<Via<N>, ErrorCode> {
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

        // Reject fragmented IPv4 packets since this packet may not contain the
        // complete TCP header and we can't validate it.
        if ipv4.fragment_offset() & 0x3fff != 0 {
            return Err(ErrorCode::InvalidArg);
        }

        // The IPv4 total length must match the header + payload we were given.
        // TODO: Should we overwrite instead?
        let payload_len = payload_buf.as_ref().map(|p| p.len()).unwrap_or(0);
        if ipv4.total_len() != header_buf.len() + payload_len {
            return Err(ErrorCode::InvalidArg);
        }

        let ip_proto = ipv4.ip_proto();
        let src_ip = ipv4.src_ip();
        let dst_ip = ipv4.dst_ip();
        let ipv4_header_len = ipv4.header_len();

        let trans_bytes = &mut header_buf[ipv4_header_len..];
        let (local_port, remote_port, protocols, trans_header_len) = match ip_proto {
            IP_PROTO_TCP => {
                match TcpInspector::new(trans_bytes) {
                    Ok(tcp) => (
                        tcp.src_port(),
                        tcp.dst_port(),
                        PROTO_IPV4 | PROTO_TCP,
                        tcp.header_len(),
                    ),
                    Err(e) => {
                        ftl_driver::trace!(self.env, "invalid TCP header: {:?}", e);
                        return Err(ErrorCode::InvalidArg);
                    }
                }
            }
            IP_PROTO_UDP => {
                match UdpRewriter::new(trans_bytes) {
                    Ok(udp) => (
                        udp.src_port(),
                        udp.dst_port(),
                        PROTO_IPV4 | PROTO_UDP,
                        UDP_HEADER_LEN,
                    ),
                    Err(e) => {
                        ftl_driver::trace!(self.env, "invalid UDP header: {:?}", e);
                        return Err(ErrorCode::InvalidArg);
                    }
                }
            }
            _ => {
                // TODO: Should we overwrite instead?
                return Err(ErrorCode::InvalidArg);
            }
        };

        if header_buf.len() != ipv4_header_len + trans_header_len {
            return Err(ErrorCode::InvalidArg);
        }

        let (dst_nic, route_src_ip) = self
            .lookup_route(protocols, dst_ip, remote_port)
            .ok_or(ErrorCode::NotAllowed)?;

        // Overwrite the source IP address if the route specifies one.
        let mut src_ip = route_src_ip.unwrap_or(src_ip);

        // Where should we send the packet?
        let via = if let Some(physical) = &dst_nic.physical {
            let (our_ip, next_hop_ip) = {
                let physical = physical.lock();
                let Some(external) = physical.external else {
                    return Err(ErrorCode::InvalidArg);
                };

                let our_ip = external.address;
                let next_hop_ip = external.next_hop_ip(dst_ip);
                (our_ip, next_hop_ip)
            };

            if dst_ip == our_ip {
                // Reject packets destined to our own IP address. Otherwise,
                // containers may access each other's public endpoints by
                // default (not via explicit vNIC routes).
                return Err(ErrorCode::NotAllowed);
            }

            src_ip = our_ip;
            Via::External {
                nic: dst_nic.clone(),
                our_ip,
                next_hop_ip,
            }
        } else {
            Via::Local(dst_nic.clone())
        };

        if !self.has_route_back(&dst_nic, protocols, src_ip, local_port) {
            return Err(ErrorCode::NotAllowed);
        }

        let trans_bytes = &mut header_buf[ipv4_header_len..];
        if ip_proto == IP_PROTO_TCP {
            let mut tcp = match TcpRewriter::new(trans_bytes) {
                Ok(tcp) => tcp,
                Err(e) => {
                    ftl_driver::trace!(self.env, "invalid TCP header: {:?}", e);
                    return Err(ErrorCode::InvalidArg);
                }
            };

            // Update the TCP header.
            tcp.update_checksum(src_ip, dst_ip, payload_buf.as_deref());
        } else {
            let udp_len = trans_bytes.len() + payload_len;
            let mut udp = match UdpRewriter::new(trans_bytes) {
                Ok(udp) => udp,
                Err(e) => {
                    ftl_driver::trace!(self.env, "invalid UDP header: {:?}", e);
                    return Err(ErrorCode::InvalidArg);
                }
            };

            udp.set_len(udp_len as u16);
            udp.update_checksum(src_ip, dst_ip, payload_buf.as_deref());
        }

        // Update the IPv4 header.
        let mut ipv4 = match Ipv4Rewriter::new(header_buf) {
            Ok(ipv4) => ipv4,
            Err(e) => {
                ftl_driver::trace!(self.env, "invalid IPv4 header: {:?}", e);
                return Err(ErrorCode::InvalidArg);
            }
        };
        ipv4.set_src_ip(src_ip);
        ipv4.update_checksum();

        Ok(via)
    }
}
