use alloc::sync::Arc;

use ftl_types::error::ErrorCode;
use ftl_types::net::IPPROTO_TCP;
use ftl_types::net::IPPROTO_UDP;

use crate::BufReader;
use crate::RxNotify;
use crate::device::Tx;
use crate::nic::Nic;
use crate::packet::ipv4::Ipv4Addr;
use crate::packet::ipv4::Ipv4Inspector;
use crate::packet::ipv4::Ipv4Rewriter;
use crate::packet::tcp::TcpInspector;
use crate::packet::tcp::TcpRewriter;
use crate::packet::udp::UdpRewriter;

enum Via<N> {
    Device {
        pnic: Arc<Nic<N>>,
        our_ip: Ipv4Addr,
        next_hop_ip: Ipv4Addr,
    },
    Nic(Arc<Nic<N>>),
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

        let header_len = header.len();
        let mut tx = Tx::alloc(self.env, header_len, payload_len)?;
        match self.prepare_tx(&mut tx, header, payload)? {
            Via::Device {
                pnic,
                our_ip,
                next_hop_ip,
            } => {
                let Some(physical) = &pnic.physical else {
                    return Err(ErrorCode::NotFound);
                };

                // Send the packet through the route's next hop.
                physical.lock().device.send_ipv4(our_ip, next_hop_ip, tx)
            }
            Via::Nic(dst) => self.deliver_locally(dst, tx, header_len, payload_len),
        }
    }

    fn deliver_locally(
        &self,
        dst: Arc<Nic<N>>,
        mut tx: Tx<'static>,
        header_len: usize,
        payload_len: usize,
    ) -> Result<(), ErrorCode> {
        let mut buf = self
            .env
            .alloc_dma(header_len + payload_len)
            .map_err(|_| ErrorCode::OutOfMemory)?;

        let (header, payload) = tx.header_and_payload_bytes();
        let bytes = buf.as_mut_slice();
        bytes[..header_len].copy_from_slice(&header[..header_len]);
        if let Some(payload) = payload {
            bytes[header_len..header_len + payload_len].copy_from_slice(&payload[..payload_len]);
        }

        dst.receive(buf, 0, header_len + payload_len, header_len);
        Ok(())
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
        let mut src_ip = ipv4.src_ip();
        let dst_ip = ipv4.dst_ip();
        let ipv4_header_len = ipv4.header_len();

        let trans_bytes = &mut header_buf[ipv4_header_len..];
        let (local_port, remote_port) = match ip_proto {
            IPPROTO_TCP => match TcpInspector::new(trans_bytes) {
                Ok(tcp) => (tcp.src_port(), tcp.dst_port()),
                Err(e) => {
                    ftl_driver::trace!(self.env, "invalid TCP header: {:?}", e);
                    return Err(ErrorCode::InvalidArg);
                }
            },
            IPPROTO_UDP => match UdpRewriter::new(trans_bytes) {
                Ok(udp) => (udp.src_port(), udp.dst_port()),
                Err(e) => {
                    ftl_driver::trace!(self.env, "invalid UDP header: {:?}", e);
                    return Err(ErrorCode::InvalidArg);
                }
            },
            _ => {
                // TODO: Should we overwrite instead?
                return Err(ErrorCode::InvalidArg);
            }
        };

        let Some(dst) = self.lookup(remote_port) else {
            return Err(ErrorCode::NotAllowed);
        };
        if !dst.routes_back_to(local_port, self) {
            return Err(ErrorCode::NotAllowed);
        }

        let via;
        if let Some(physical) = &dst.physical {
            if dst_ip.as_u32() >> 16 == 0xa9fe {
                return Err(ErrorCode::NotAllowed);
            }

            let Some(external) = physical.lock().external else {
                return Err(ErrorCode::InvalidArg);
            };

            let our_ip = external.address;
            let next_hop_ip = external.next_hop(dst_ip);

            src_ip = our_ip;
            if dst_ip == our_ip {
                let Some(hairpin) = dst.lookup(remote_port) else {
                    return Err(ErrorCode::NotAllowed);
                };
                via = Via::Nic(hairpin);
            } else {
                via = Via::Device {
                    pnic: dst.clone(),
                    our_ip,
                    next_hop_ip,
                };
            }
        } else {
            via = Via::Nic(dst);
        }

        let trans_bytes = &mut header_buf[ipv4_header_len..];
        if ip_proto == IPPROTO_TCP {
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
