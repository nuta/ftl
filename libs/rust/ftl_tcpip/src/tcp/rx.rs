use crate::Io;
use crate::device::DeviceMap;
use crate::ip::ipv4::Ipv4Addr;
use crate::route::RouteTable;
use crate::socket::{ActiveKey, Endpoint, ListenerKey, SocketMap};
use crate::packet;
use crate::packet::Packet;
use crate::ip::IpAddr;
use crate::tcp::header::TcpHeader;
use crate::tcp::{TcpConn, TcpListener};
use crate::transport::{Port, Protocol};

#[derive(Debug)]
pub(crate) enum RxError {
    PacketRead(packet::ReserveError),
}

pub(crate) fn handle_rx<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    sockets: &mut SocketMap,
    pkt: &mut Packet,
    remote_ip: IpAddr,
    local_ip: IpAddr,
) -> Result<(), RxError> {
    let header = pkt.read::<TcpHeader>().map_err(RxError::PacketRead)?;
    let src_port = Port::from(header.src_port);
    let dst_port = Port::from(header.dst_port);
    let flags = header.flags;
    let seq: u32 = header.seq.into();
    let ack: u32 = header.ack.into();
    let window_size: u16 = header.window_size.into();

    trace!(
        "TCP packet [flags: {:?}] src_port: {}, dst_port: {}, {:?}",
        flags,
        src_port,
        dst_port,
        core::str::from_utf8(pkt.slice()).unwrap_or("(invalid UTF-8)"),
    );

    let key = ActiveKey {
        local: Endpoint {
            addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: dst_port,
        },
        protocol: Protocol::Tcp,
        remote: Endpoint {
            addr: remote_ip,
            port: src_port,
        },
    };

    match sockets.get_active::<TcpConn<I>>(&key) {
        Some(conn) => {
            todo!()
        }
        None => {
            let key = ListenerKey {
                local: Endpoint {
                    addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    port: dst_port,
                },
                protocol: Protocol::Tcp,
            };

            match sockets.get_listener::<TcpListener<I>>(&key) {
                Some(listener) => {
                    todo!()
                }
                None => {
                    trace!("TCP: no connection or listener found");
                    // TODO: Send an RST packet.
                }
            }
        }
    }

    Ok(())
}
