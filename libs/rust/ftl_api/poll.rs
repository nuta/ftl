use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;

use crate::handle::OwnedHandle;
use crate::syscall;

pub enum Event {
    ChannelNewMessage,
}

pub struct Poll(OwnedHandle);

impl Poll {
    pub fn new() -> Result<Poll, FtlError> {
        let handle = syscall::poll_create()?;
        Ok(Poll(OwnedHandle::from_raw(handle)))
    }

    pub fn add(&self, handle: HandleId) -> Result<(), FtlError> {
        syscall::poll_add(self.0.id(), handle)?;
        Ok(())
    }

    pub fn wait(&self) -> Result<(Event, HandleId), FtlError> {
        let raw = syscall::poll_wait(self.0.id())?;
        todo!()
    }
}
