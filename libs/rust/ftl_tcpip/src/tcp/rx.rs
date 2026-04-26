use crate::Io;
use crate::device::DeviceMap;
use crate::ip::IpAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::packet;
use crate::packet::Packet;
use crate::route::RouteTable;
use crate::socket::ActiveKey;
use crate::socket::Endpoint;
use crate::socket::ListenerKey;
use crate::socket::SocketMap;
use crate::tcp::TcpConn;
use crate::tcp::TcpListener;
use crate::tcp::header::TcpFlags;
use crate::tcp::header::TcpHeader;
use crate::transport::Port;
use crate::transport::Protocol;

#[derive(Debug)]
pub(crate) enum RxError {
    PacketRead(packet::ReserveError),
}

pub(super) struct RxHeader {
    pub remote_ip: IpAddr,
    pub local_ip: IpAddr,
    pub src_port: Port,
    pub dst_port: Port,
    pub flags: TcpFlags,
    pub seq: u32,
    pub ack: u32,
    pub window_size: u16,
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
    let rx = RxHeader {
        remote_ip,
        local_ip,
        src_port: Port::from(header.src_port),
        dst_port: Port::from(header.dst_port),
        flags: header.flags,
        seq: header.seq.into(),
        ack: header.ack.into(),
        window_size: header.window_size.into(),
    };

    trace!(
        "TCP packet [flags: {:?}] src_port: {}, dst_port: {}, {:?}",
        rx.flags,
        rx.src_port,
        rx.dst_port,
        core::str::from_utf8(pkt.slice()).unwrap_or("(invalid UTF-8)"),
    );

    let key = ActiveKey {
        local: Endpoint {
            addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: rx.dst_port,
        },
        protocol: Protocol::Tcp,
        remote: Endpoint {
            addr: remote_ip,
            port: rx.src_port,
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
                    port: rx.src_port,
                },
                protocol: Protocol::Tcp,
            };

            match sockets.get_listener::<TcpListener<I>>(&key) {
                Some(listener) => {
                    listener.handle_rx(devices, routes, sockets, rx, pkt);
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
