use crate::Io;
use crate::device::DeviceMap;
use crate::ip::IpAddr;
use crate::packet::Packet;
use crate::packet::{self};
use crate::route::RouteTable;
use crate::socket::SocketMap;

mod connection;
mod listener;
mod tx;
mod rx;
mod header;
mod checksum;

pub use connection::TcpConn;
pub use listener::TcpListener;
pub(crate) use rx::handle_rx;
pub(crate) use rx::RxError;

#[derive(Debug)]
pub enum Error {}

pub trait Read: Send + Sync {
    fn complete(self, result: Result<&[u8], Error>);
}

pub trait Write: Send + Sync {
    fn len(&self) -> usize;
    fn read(&mut self, buf: &mut [u8]) -> usize;
    fn complete(self, result: Result<usize, Error>);
}

pub trait Accept: Send + Sync {
    fn complete(self, result: Result<(), Error>);
}
