use alloc::sync::Arc;
use core::marker::PhantomData;

use crate::TcpIp;
use crate::checksum::Checksum;
use crate::endian::Ne;
use crate::ethernet::EthernetHeader;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::ip::Ipv4Addr;
use crate::ip::ipv4::Ipv4Header;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::packet::{self};
use crate::socket::Endpoint;
use crate::transport::Port;
use crate::transport::Protocol;

pub struct UdpHandle<I: Io>(pub(crate) Arc<UdpSocket<I>>);

pub(crate) struct UdpSocket<I: Io> {
    local_port: Port,
    _pd: PhantomData<I>,
}

impl<I: Io> UdpSocket<I> {
    pub(crate) fn new(local_port: Port) -> Self {
        Self {
            local_port,
            _pd: PhantomData,
        }
    }

    pub fn send(
        &self,
        tcpip: &mut TcpIp<I>,
        pkt: &mut Packet,
        remote: Endpoint,
        data: &[u8],
    ) -> Result<(), TxError> {
        let udp_len: u16 = (data.len() + size_of::<UdpHeader>())
            .try_into()
            .map_err(|_| TxError::DataTooLong(data.len()))?;

        let mut header = UdpHeader {
            src_port: self.local_port.into(),
            dst_port: remote.port.into(),
            len: udp_len.into(),
            checksum: 0.into(),
        };

        let Some((iface, next_hop)) = tcpip.lookup_route(remote.addr) else {
            return Err(TxError::NoRoute);
        };

        match (remote.addr, next_hop) {
            (IpAddr::V4(remote_ipv4), IpAddr::V4(next_hop)) => {
                let Some(local_ipv4) = iface.ipv4_addr() else {
                    return Err(TxError::NoLocalIpv4);
                };

                let head_room =
                    size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<UdpHeader>();
                let mut pkt = Packet::new(data.len(), head_room).map_err(TxError::PacketAlloc)?;
                header.checksum =
                    compute_udp_checksum(local_ipv4, remote_ipv4, udp_len, pkt.slice()).into();
                pkt.write_front(header).map_err(TxError::PacketWrite)?;
                pkt.write_back_bytes(data).map_err(TxError::PacketWrite)?;

                crate::ip::ipv4::transmit::<I>(
                    iface,
                    &mut pkt,
                    remote_ipv4,
                    next_hop,
                    local_ipv4,
                    Protocol::Udp,
                )
                .map_err(TxError::Ipv4Tx)?;
            }
        }

        Ok(())
    }
}

fn compute_udp_checksum(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, udp_len: u16, payload: &[u8]) -> u16 {
    let mut checksum = Checksum::new();
    checksum.supply_u32(src_ip.as_u32());
    checksum.supply_u32(dst_ip.as_u32());
    checksum.supply_u16(Protocol::Udp as u16);
    checksum.supply_u16(udp_len as u16);
    checksum.supply_bytes(payload);
    checksum.finish()
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
    pkt: &mut Packet,
    remote: IpAddr,
    local: IpAddr,
) -> Result<(), RxError> {
    let header = pkt.read::<UdpHeader>().map_err(RxError::PacketRead)?;
    todo!()
}
