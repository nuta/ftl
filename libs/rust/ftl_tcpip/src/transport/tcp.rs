use alloc::collections::VecDeque;
use core::fmt;
use core::ops::BitOr;
use core::ops::BitOrAssign;

use crate::Io;
use crate::OutOfMemoryError;
use crate::device::DeviceMap;
use crate::endian::Ne;
use crate::ip::IpAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::packet;
use crate::packet::Packet;
use crate::route::RouteTable;
use crate::socket::ActiveKey;
use crate::socket::AnySocket;
use crate::socket::Endpoint;
use crate::socket::ListenerKey;
use crate::socket::SocketMap;
use crate::transport::Port;
use crate::transport::Protocol;
use crate::utils::TryPushBack;

#[derive(Debug)]
pub enum Error {}

pub trait Read: Send + Sync {
    fn write(&mut self, buf: &[u8]) -> usize;
    fn complete(self, result: Result<usize, Error>);
}

pub trait Write: Send + Sync {
    fn read(&mut self, buf: &mut [u8]) -> usize;
    fn complete(self, result: Result<usize, Error>);
}

pub trait Accept: Send + Sync {
    fn complete(self, result: Result<(), Error>);
}

pub struct TcpConn<I: Io> {
    pending_writes: VecDeque<I::TcpWrite>,
    pending_reads: VecDeque<I::TcpRead>,
}

impl<I: Io> TcpConn<I> {
    pub(crate) fn new() -> Self {
        Self {
            pending_writes: VecDeque::new(),
            pending_reads: VecDeque::new(),
        }
    }
}

impl<I: Io> AnySocket for TcpConn<I> {}

pub struct TcpListener<I: Io> {
    pending_accepts: VecDeque<I::TcpAccept>,
}

impl<I: Io> TcpListener<I> {
    pub(crate) fn new() -> Self {
        Self {
            pending_accepts: VecDeque::new(),
        }
    }

    pub fn accept(&mut self, req: I::TcpAccept) -> Result<(), OutOfMemoryError> {
        self.pending_accepts.try_push_back(req)?;
        Ok(())
    }
}

impl<I: Io> AnySocket for TcpListener<I> {}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct TcpFlags(u8);

impl TcpFlags {
    pub const FIN: Self = Self(1 << 0);
    pub const SYN: Self = Self(1 << 1);
    pub const RST: Self = Self(1 << 2);
    pub const PSH: Self = Self(1 << 3);
    pub const ACK: Self = Self(1 << 4);

    pub fn contains(&self, flag: TcpFlags) -> bool {
        self.0 & flag.0 != 0
    }
}

impl BitOr<TcpFlags> for TcpFlags {
    type Output = TcpFlags;

    fn bitor(self, rhs: TcpFlags) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign<TcpFlags> for TcpFlags {
    fn bitor_assign(&mut self, rhs: TcpFlags) {
        self.0 |= rhs.0;
    }
}

impl fmt::Debug for TcpFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (value, name) in [
            (TcpFlags::FIN, "FIN"),
            (TcpFlags::SYN, "SYN"),
            (TcpFlags::RST, "RST"),
            (TcpFlags::PSH, "PSH"),
            (TcpFlags::ACK, "ACK"),
        ] {
            if self.0 & value.0 != 0 {
                if !first {
                    write!(f, "|")?;
                }

                write!(f, "{name}")?;
                first = false;
            }
        }

        Ok(())
    }
}

#[repr(C, packed)]
struct TcpHeader {
    src_port: Ne<u16>,
    dst_port: Ne<u16>,
    seq: Ne<u32>,
    ack: Ne<u32>,
    header_len: u8,
    flags: TcpFlags,
    window_size: Ne<u16>,
    checksum: Ne<u16>,
    urgent_pointer: Ne<u16>,
}

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
) -> Result<(), RxError> {
    let header = pkt.read::<TcpHeader>().map_err(RxError::PacketRead)?;
    let src_port = Port::from(header.src_port);
    let dst_port = Port::from(header.dst_port);

    trace!(
        "TCP packet [flags: {:?}] src_port: {}, dst_port: {}",
        header.flags, src_port, dst_port
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
            // TODO Handle the TCP connection.
            todo!("handle established connection");
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
                    // TODO Send an RST packet.
                    warn!("TCP packet: listener not found");
                }
                None => {
                    trace!("TCP: no connection or listener found");
                }
            }
        }
    }

    Ok(())
}
