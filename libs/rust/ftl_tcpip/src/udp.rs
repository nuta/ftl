use alloc::sync::Arc;
use core::marker::PhantomData;

use crate::TcpIp;
use crate::endian::Ne;
use crate::ethernet::EthernetHeader;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::ip::ipv4::Ipv4Header;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::packet::{self};
use crate::socket::Endpoint;
use crate::transport::Port;

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

    pub fn send(&self, pkt: &mut Packet, remote: Endpoint, data: &[u8]) -> Result<(), TxError> {
        let len: u16 = data
            .len()
            .try_into()
            .map_err(|_| TxError::DataTooLong(data.len()))?;
        let header = UdpHeader {
            src_port: self.local_port.into(),
            dst_port: remote.port.into(),
            len: len.into(),
            checksum: 0.into(),
        };

        let head_room =
            size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<UdpHeader>();
        let mut pkt = Packet::new(data.len(), head_room).map_err(TxError::PacketAlloc)?;

        pkt.write_front(header).map_err(TxError::PacketWrite)?;
        pkt.write_back_bytes(data).map_err(TxError::PacketWrite)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum TxError {
    DataTooLong(usize),
    PacketAlloc(packet::AllocError),
    PacketWrite(packet::ReserveError),
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
