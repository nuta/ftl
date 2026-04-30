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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertError {
    OutOfMemory,
    AlreadyExists,
}

pub trait Map<K, V> {
    fn new() -> Self;
    fn insert(&mut self, key: K, value: V) -> Result<(), InsertError>;
    fn get(&self, key: &K) -> Option<&V>;
    fn remove(&mut self, key: &K) -> Option<V>;
}

pub trait Io: 'static {
    type Device: Device;
    type TcpWrite: tcp::Write;
    type TcpRead: tcp::Read;
    type TcpAccept: tcp::Accept;
    type Map<K, V>: Map<K, V>;
}
