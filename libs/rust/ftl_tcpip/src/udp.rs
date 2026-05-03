use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::TcpIp;
use crate::checksum::Checksum;
use crate::endian::Ne;
use crate::ethernet::EthernetHeader;
use crate::interface::Interface;
use crate::interface::InterfaceId;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::ip::Ipv4Addr;
use crate::ip::ipv4::Ipv4Header;
use crate::packet;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::socket::Endpoint;
use crate::transport::Port;
use crate::transport::Protocol;

pub struct UdpHandle<I: Io>(pub(crate) Arc<UdpSocket<I>>);

pub struct Datagram {
    pub remote: Endpoint,
    pub data: Vec<u8>,
}

pub(crate) struct UdpSocket<I: Io> {
    local_port: Port,
    rx: spin::Mutex<VecDeque<Datagram>>,
    _pd: PhantomData<I>,
}

impl<I: Io> UdpSocket<I> {
    pub(crate) fn new(local_port: Port) -> Self {
        Self {
            local_port,
            rx: spin::Mutex::new(VecDeque::new()),
            _pd: PhantomData,
        }
    }

    pub fn send(&self, tcpip: &mut TcpIp<I>, remote: Endpoint, data: &[u8]) -> Result<(), TxError> {
        let Some((iface, next_hop)) = tcpip.lookup_route(remote.addr) else {
            return Err(TxError::NoRoute);
        };

        match (remote.addr, next_hop) {
            (IpAddr::V4(remote_ipv4), IpAddr::V4(next_hop)) => {
                let Some(local_ipv4) = iface.ipv4_addr() else {
                    return Err(TxError::NoLocalIpv4);
                };

                self.send_from_v4(iface, local_ipv4, remote_ipv4, remote.port, next_hop, data)
            }
        }
    }

    pub(crate) fn send_from_v4(
        &self,
        iface: &mut Interface<I::Device>,
        local_ipv4: Ipv4Addr,
        remote_ipv4: Ipv4Addr,
        remote_port: Port,
        nexthop_ipv4: Ipv4Addr,
        data: &[u8],
    ) -> Result<(), TxError> {
        let udp_len: u16 = (data.len() + size_of::<UdpHeader>())
            .try_into()
            .map_err(|_| TxError::DataTooLong(data.len()))?;

        let mut header = UdpHeader {
            src_port: self.local_port.into(),
            dst_port: remote_port.into(),
            len: udp_len.into(),
            checksum: 0.into(),
        };

        let head_room =
            size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<UdpHeader>();
        let mut pkt = Packet::new(data.len(), head_room).map_err(TxError::PacketAlloc)?;
        header.checksum = compute_udp_checksum(&header, local_ipv4, remote_ipv4, data).into();
        pkt.write_front(header).map_err(TxError::PacketWrite)?;
        pkt.write_back_bytes(data).map_err(TxError::PacketWrite)?;

        crate::ip::ipv4::transmit::<I>(
            iface,
            &mut pkt,
            remote_ipv4,
            nexthop_ipv4,
            local_ipv4,
            Protocol::Udp,
        )
        .map_err(TxError::Ipv4Tx)?;

        Ok(())
    }

    pub fn try_recv(&self) -> Option<Datagram> {
        let mut rx = self.rx.lock();
        rx.pop_front()
    }

    fn handle_rx(&self, pkt: &mut Packet, remote: Endpoint) -> Result<(), RxError> {
        let mut rx = self.rx.lock();
        if rx.len() >= 128 {
            return Err(RxError::RxQueueFull);
        }

        rx.push_back(Datagram {
            remote,
            data: pkt.slice().to_vec(),
        });
        Ok(())
    }
}

fn compute_udp_checksum(
    header: &UdpHeader,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    payload: &[u8],
) -> u16 {
    let mut checksum = Checksum::new();
    let udp_len = size_of::<UdpHeader>() + payload.len();
    debug_assert!(udp_len <= u16::MAX as usize);

    checksum.supply_u32(src_ip.as_u32());
    checksum.supply_u32(dst_ip.as_u32());
    checksum.supply_u16(Protocol::Udp as u16);
    checksum.supply_u16(udp_len as u16);
    checksum.supply_u16(header.src_port.into());
    checksum.supply_u16(header.dst_port.into());
    checksum.supply_u16(header.len.into());
    checksum.supply_u16(0);
    checksum.supply_bytes(payload);
    match checksum.finish() {
        0 => 0xffff,
        checksum => checksum,
    }
}

#[derive(Debug)]
pub enum TxError {
    DataTooLong(usize),
    PacketAlloc(packet::AllocError),
    PacketWrite(packet::ReserveError),
    NoRoute,
    NoLocalIpv4,
    Ipv4Tx(crate::ip::ipv4::TxError),
}

#[derive(Debug)]
pub enum RxError {
    PacketRead(packet::ReserveError),
    RxQueueFull,
    DhcpRx(crate::dhcp::RxError),
    DhcpTx(crate::dhcp::TxError),
    DhcpTransmit(crate::udp::TxError),
}

#[repr(C, packed)]
pub(super) struct UdpHeader {
    pub src_port: Ne<u16>,
    pub dst_port: Ne<u16>,
    pub len: Ne<u16>,
    pub checksum: Ne<u16>,
}

impl WriteableToPacket for UdpHeader {}

pub(crate) fn handle_rx<I: Io>(
    tcpip: &mut TcpIp<I>,
    iface_id: InterfaceId,
    pkt: &mut Packet,
    remote: IpAddr,
    local: IpAddr,
) -> Result<(), RxError> {
    let header = pkt.read::<UdpHeader>().map_err(RxError::PacketRead)?;
    let local = Endpoint {
        addr: local,
        port: Port::from(header.dst_port),
    };
    let remote = Endpoint {
        addr: remote,
        port: Port::from(header.src_port),
    };

    let socket = match tcpip.sockets().get_udp_socket(&local) {
        Some(socket) => socket,
        None => {
            debug!("UDP: no socket found for local address: {:?}", local);
            return Ok(());
        }
    };

    socket.handle_rx(pkt, remote)?;

    if let Some(client) = tcpip.sockets_mut().get_dhcp_client_mut(iface_id, socket.local_port) {
        client.handle_rx(pkt.slice()).map_err(RxError::DhcpRx)?;
        if let Some(tx) = client.poll_tx().map_err(RxError::DhcpTx)? {
            let iface = tcpip.interfaces_mut().get_mut(iface_id).unwrap();
            socket
                .send_from_v4(
                    iface,
                    tx.local_ip,
                    tx.remote_ip,
                    tx.remote_port,
                    tx.remote_ip,
                    tx.pkt.slice(),
                )
                .map_err(RxError::DhcpTransmit)?;
        }
    }

    Ok(())
}
