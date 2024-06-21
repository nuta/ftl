use ftl_types::error::FtlError;

use crate::handle::OwnedHandle;
use crate::syscall;

pub enum Event {
    // ChannelNewMessage(HandleId),
}

pub struct Poll(OwnedHandle);

impl Poll {
    pub fn new() -> Result<Poll, FtlError> {
        let handle = syscall::poll_create()?;
        Ok(Poll(OwnedHandle::from_raw(handle)))
    }

    pub fn wait(&self) -> Result<Event, FtlError> {
        let raw = syscall::poll_wait(self.0.id())?;
        todo!()
    }
}
