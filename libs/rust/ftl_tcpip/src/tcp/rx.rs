use crate::TcpIp;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::packet;
use crate::packet::Packet;
use crate::socket::Endpoint;
use crate::tcp::header::TcpFlags;
use crate::tcp::header::TcpHeader;
use crate::transport::Port;

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

    if rx.flags.contains(TcpFlags::RST) && tcpip.sockets_mut().destroy_tcp_conn(&local, &remote) {
        return Ok(());
    }

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

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    use core::time::Duration;

    use super::*;
    use crate::ethernet::MacAddr;
    use crate::interface::Device;
    use crate::io::Instant;
    use crate::ip::Ipv4Addr;
    use crate::packet::Packet;
    use crate::tcp::Accept;
    use crate::tcp::Error;
    use crate::tcp::Read;
    use crate::tcp::TcpBuffer;
    use crate::tcp::TcpConn;
    use crate::tcp::TimeoutResult;
    use crate::tcp::Write;

    #[derive(Clone, Copy)]
    struct TestInstant;

    impl Instant for TestInstant {
        fn checked_add(&self, _duration: Duration) -> Option<Self> {
            Some(*self)
        }

        fn now(&self) -> Self {
            *self
        }

        fn is_before(&self, _other: &Self) -> bool {
            false
        }

        fn elapsed_since(&self, _other: &Self) -> Duration {
            Duration::ZERO
        }
    }

    struct TestDevice;

    impl Device for TestDevice {
        fn mac_addr(&self) -> &MacAddr {
            static MAC: MacAddr = MacAddr::new([0; 6]);
            &MAC
        }

        fn transmit(&mut self, _pkt: &mut Packet) {}
    }

    struct TestRead;
    impl Read for TestRead {
        fn complete(self, _rx_buffer: &mut TcpBuffer) {}
    }

    struct TestWrite;
    impl Write for TestWrite {
        fn complete(self, _tx_buffer: &mut TcpBuffer) {}
    }

    struct TestAccept;
    impl Accept for TestAccept {
        fn complete(self, _result: Result<(), Error>) {}
    }

    struct TestIo;

    impl Io for TestIo {
        type Device = TestDevice;
        type TcpWrite = TestWrite;
        type TcpRead = TestRead;
        type TcpAccept = TestAccept;
        type Instant = TestInstant;

        fn now(&self) -> Self::Instant {
            TestInstant
        }

        fn set_timer(&mut self, _at: Self::Instant) {}
    }

    fn endpoint(addr_last: u8, port: u16) -> Endpoint {
        Endpoint {
            addr: IpAddr::V4(Ipv4Addr::new(192, 0, 2, addr_last)),
            port: Port::new(port),
        }
    }

    fn tcp_packet(flags: TcpFlags, src_port: Port, dst_port: Port) -> Packet {
        let header = TcpHeader {
            src_port: src_port.into(),
            dst_port: dst_port.into(),
            seq: 1.into(),
            ack: 0.into(),
            window_size: 0.into(),
            flags,
            header_len: 5 << 4,
            checksum: 0.into(),
            urgent_pointer: 0.into(),
        };

        let mut pkt = Packet::new(size_of::<TcpHeader>(), 0).unwrap();
        pkt.write_back(header).unwrap();
        pkt
    }

    #[test]
    fn rst_destroys_active_socket() {
        let local = endpoint(1, 80);
        let remote = endpoint(2, 49152);
        let conn = Arc::new(TcpConn::new_listen(local.port));
        conn.open_passively(remote, 1000, 2000, 1024, TcpBuffer::new());

        let mut tcpip = TcpIp::<TestIo>::new(TestIo);
        tcpip
            .sockets_mut()
            .establish_tcp_conn(remote, local, conn.clone())
            .unwrap();

        let mut pkt = tcp_packet(TcpFlags::RST, remote.port, local.port);
        handle_rx(&mut tcpip, &mut pkt, remote.addr, local.addr).unwrap();

        assert!(tcpip.sockets().get_tcp_conn(&local, &remote).is_none());
        assert!(matches!(
            conn.handle_timeout(&TestInstant),
            TimeoutResult::Closed
        ));
    }
}
