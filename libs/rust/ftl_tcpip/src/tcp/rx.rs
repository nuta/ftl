use crate::TcpIp;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::packet;
use crate::packet::Packet;
use crate::socket::Endpoint;
use crate::tcp::header::TcpFlags;
use crate::tcp::header::TcpHeader;
use crate::transport::Port;
use crate::transport::Protocol;

#[derive(Debug)]
pub enum RxError {
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
    tcpip: &mut TcpIp<I>,
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
        "TCP: RX: flags={:?}, src_port={}, dst_port={}, seq={}, ack={}",
        rx.flags,
        rx.src_port,
        rx.dst_port,
        Into::<u32>::into(rx.seq),
        Into::<u32>::into(rx.ack),
    );

    let local = Endpoint {
        addr: local_ip,
        port: rx.dst_port,
    };
    let remote = Endpoint {
        addr: remote_ip,
        port: rx.src_port,
    };

    match tcpip.sockets().get_tcp_conn(&local, &remote) {
        Some(conn) => {
            conn.handle_rx(tcpip, rx, pkt);
        }
        None => {
            match tcpip.sockets().get_tcp_listener(&local) {
                Some(listener) => {
                    listener.handle_rx(tcpip, rx, pkt);
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
