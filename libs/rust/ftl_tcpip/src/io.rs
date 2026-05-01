use core::time::Duration;

use crate::interface::Device;
use crate::tcp;

pub trait Instant: Send + Sync + Clone + Copy {
    fn checked_add(&self, duration: Duration) -> Option<Self>;
    fn now(&self) -> Self;
    fn is_before(&self, other: &Self) -> bool;
    fn elapsed_since(&self, other: &Self) -> Duration;
}

pub trait Io: 'static {
    type Device: Device;
    type TcpWrite: tcp::Write;
    type TcpRead: tcp::Read;
    type TcpAccept: tcp::Accept;

    type Instant: Instant;
    fn now(&self) -> Self::Instant;
    fn set_timer(&mut self, at: Self::Instant);
}
