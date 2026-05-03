use std::marker::PhantomData;
use std::sync::Arc;

use crate::TcpIp;
use crate::endian::Ne;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::packet;
use crate::packet::Packet;
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
}

#[derive(Debug)]
pub enum RxError {
    PacketRead(packet::ReserveError),
}

#[repr(C, packed)]
pub(super) struct UdpHeader {
    pub src_port: Ne<u16>,
    pub dst_port: Ne<u16>,
    pub length: Ne<u16>,
    pub checksum: Ne<u16>,
}

pub(crate) fn handle_rx<I: Io>(
    tcpip: &mut TcpIp<I>,
    pkt: &mut Packet,
    remote: IpAddr,
    local: IpAddr,
) -> Result<(), RxError> {
    let header = pkt.read::<UdpHeader>().map_err(RxError::PacketRead)?;
    todo!()
}
