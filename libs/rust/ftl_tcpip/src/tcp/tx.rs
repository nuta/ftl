use super::header::TcpHeader;
use crate::Io;
use crate::device::DeviceMap;
use crate::ethernet::EthernetHeader;
use crate::ip::IpAddr;
use crate::ip::ipv4;
use crate::ip::ipv4::Ipv4Header;
use crate::packet;
use crate::packet::Packet;
use crate::route::RouteTable;
use crate::tcp::checksum::compute_checksum;
use crate::transport::Protocol;

#[derive(Debug)]
pub(super) enum TxError {
    PacketAlloc(packet::AllocError),
    PacketWrite(packet::ReserveError),
    Ipv4Tx(ipv4::TxError),
    NoRoute,
    NoDevice,
}

fn encode_header_len(len: usize) -> u8 {
    debug_assert_eq!(len % 4, 0);
    debug_assert!(len / 4 <= 0x0f);
    ((len / 4) as u8) << 4
}

pub(super) fn transmit_segment<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    mut header: TcpHeader,
    remote_ip: IpAddr,
    payload: &[u8],
) -> Result<(), TxError> {
    // We'll fill the header length and checksum later.
    debug_assert_eq!(header.header_len, 0);
    debug_assert_eq!(Into::<u16>::into(header.checksum), 0);

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

    header.header_len = encode_header_len(size_of::<TcpHeader>());

    match remote_ip {
        IpAddr::V4(remote_ipv4) => {
            header.checksum =
                compute_checksum(&header, route.ipv4_addr(), remote_ipv4, pkt.slice()).into();

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
