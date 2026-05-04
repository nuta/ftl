mod buffer;
mod checksum;
mod connection;
mod header;
mod listener;
mod rx;
mod tx;

pub use buffer::TcpBuffer;
pub(crate) use connection::TcpConn;
pub(crate) use connection::TimeoutResult;
pub(crate) use listener::TcpListener;
pub(crate) use rx::RxError;
pub(crate) use rx::handle_rx;

#[derive(Debug)]
pub enum Error {
    Closed,
}

pub trait Read: Send + Sync {
    fn complete(self, rx_buffer: &mut TcpBuffer);
    fn abort(self, error: Error);
}

pub trait Write: Send + Sync {
    fn complete(self, tx_buffer: &mut TcpBuffer);
}

pub trait Accept: Send + Sync {
    fn complete(self, result: Result<(), Error>);
}
