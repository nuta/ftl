use crate::tcp::checksum::compute_checksum;
use crate::transport::Protocol;
use crate::{ip::ipv4, packet};
use crate::Io;
use crate::device::DeviceMap;
use crate::route::RouteTable;
use crate::ip::IpAddr;
use crate::packet::Packet;
use crate::ethernet::EthernetHeader;
use crate::ip::ipv4::Ipv4Header;
use super::header::TcpHeader;

#[derive(Debug)]
enum TxError {
    PacketAlloc(packet::AllocError),
    PacketWrite(packet::ReserveError),
    Ipv4Tx(ipv4::TxError),
    NoRoute,
    NoDevice,
}

fn transmit_segment<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    mut header: TcpHeader,
    remote_ip: IpAddr,
    payload: &[u8],
) -> Result<(), TxError> {
    let head_room = size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<TcpHeader>();
    let mut pkt = Packet::new(payload.len(), head_room).map_err(TxError::PacketAlloc)?;
    pkt.write_back_bytes(payload)
        .map_err(TxError::PacketWrite)?;

        let Some(route) = routes.lookup_by_dest(remote_ip) else {
            return Err(TxError::NoRoute);
        };

        let Some(device) = devices.get_mut(route.device_id()) else {
            return Err(TxError::NoDevice);
        };

    match remote_ip {
        IpAddr::V4(remote_ipv4) => {
            header.checksum = 
                compute_checksum(&header, route.ipv4_addr(), remote_ipv4, pkt.slice())
                .into();

            pkt.write_front(header).map_err(TxError::PacketWrite)?;

            ipv4::transmit::<I>(
                device,
                route,
                &mut pkt,
                route.ipv4_addr(),
                remote_ipv4,
                Protocol::Tcp,
            )
            .map_err(TxError::Ipv4Tx)?;
        }
    }

    Ok(())
}
