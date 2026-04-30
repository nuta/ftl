use core::ops::Deref;
use core::ops::DerefMut;

use crate::device::Device;
use crate::tcp;

pub trait Mutex<T: ?Sized> {
    type Guard<'a>: Deref<Target = T> + DerefMut<Target = T> + 'a
    where
        Self: 'a;
    fn lock(&self) -> Self::Guard<'_>;
}

pub trait Io: 'static {
    type Device: Device;
    type TcpWrite: tcp::Write;
    type TcpRead: tcp::Read;
    type TcpAccept: tcp::Accept;
}
