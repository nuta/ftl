use crate::interface::Device;
use crate::tcp;

pub trait Io: 'static {
    type Device: Device;
    type TcpWrite: tcp::Write;
    type TcpRead: tcp::Read;
    type TcpAccept: tcp::Accept;
}
