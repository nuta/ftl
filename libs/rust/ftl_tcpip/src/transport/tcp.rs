use alloc::collections::VecDeque;

use crate::Io;
use crate::OutOfMemoryError;
use crate::endian::Ne;
use crate::ip::ipv4::Ipv4Addr;
use crate::packet;
use crate::packet::Packet;
use crate::route::RouteTable;
use crate::socket::AnySocket;
use crate::socket::SocketMap;
use crate::transport::Port;
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

#[repr(C, packed)]
struct TcpHeader {
    src_port: Ne<u16>,
    dst_port: Ne<u16>,
    seq: Ne<u32>,
    ack: Ne<u32>,
    header_len: u8,
    flags: u8,
    window_size: Ne<u16>,
    checksum: Ne<u16>,
    urgent_pointer: Ne<u16>,
}

#[derive(Debug)]
pub(crate) enum RxError {
    PacketRead(packet::ReserveError),
}

pub(crate) fn handle_rx<I: Io>(
    routes: &mut RouteTable<I::Device>,
    sockets: &mut SocketMap,
    pkt: &mut Packet,
) -> Result<(), RxError> {
    let header = pkt.read::<TcpHeader>().map_err(RxError::PacketRead)?;
    let src_port = Port::from(header.src_port);
    let dst_port = Port::from(header.dst_port);

    trace!("TCP packet: src_port: {}, dst_port: {}", src_port, dst_port);

    Ok(())
}
