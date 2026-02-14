use ftl_types::error::ErrorCode;

use crate::channel::Channel;
use crate::interrupt::Interrupt;
use crate::service::Service;
use crate::time::Timer;

enum Object {
    Channel(Channel),
    Interrupt(Interrupt),
    Timer(Timer),
    Service(Service),
}

pub struct EventLoop {}

impl EventLoop {
    pub fn new() -> Result<Self, ErrorCode> {
        Ok(Self {})
    }

    pub fn add_interrupt(&self, interrupt: Interrupt) -> Result<(), ErrorCode> {
        Ok(())
    }
}
