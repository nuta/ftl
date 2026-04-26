use crate::Io;
use crate::device::DeviceMap;
use crate::ip::IpAddr;
use crate::packet::Packet;
use crate::packet::{self};
use crate::route::RouteTable;
use crate::socket::SocketMap;

mod checksum;
mod connection;
mod header;
mod listener;
mod ring_buffer;
mod rx;
mod tx;

pub use connection::TcpConn;
pub use listener::TcpListener;
pub use ring_buffer::RingBuffer;
pub(crate) use rx::RxError;
pub(crate) use rx::handle_rx;

#[derive(Debug)]
pub enum Error {}

pub trait Read: Send + Sync {
    fn complete(self, buffer: &mut RingBuffer);
}

pub trait Write: Send + Sync {
    fn complete(self, buffer: &mut RingBuffer);
}

pub trait Accept: Send + Sync {
    fn complete(self, result: Result<(), Error>);
}
