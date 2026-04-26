use crate::Io;
use crate::device::DeviceMap;
use crate::route::RouteTable;
use crate::socket::SocketMap;
use crate::packet;
use crate::packet::Packet;
use crate::ip::IpAddr;

#[derive(Debug)]
pub(crate) enum RxError {
    PacketRead(packet::ReserveError),
}

pub(crate) fn handle_rx<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    sockets: &mut SocketMap,
    pkt: &mut Packet,
    remote: IpAddr,
    dst: IpAddr,
) -> Result<(), RxError> {
    Ok(())
}
