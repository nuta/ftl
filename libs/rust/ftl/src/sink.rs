use core::fmt;

use ftl_types::channel::MessageInfo;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;

use crate::channel::Message;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;

pub enum Event {
    Message { msginfo: MessageInfo, len: usize },
    Test,
}

pub struct Sink {
    handle: OwnedHandle,
}

impl Sink {
    pub fn add<H: Handleable>(&mut self, handle: H) -> Result<(), ErrorCode> {
        todo!()
    }

    pub fn pop(&mut self) -> Result<Event, ErrorCode> {
        todo!()
    }
}

fn sys_sink_pop(handle: HandleId) -> Result<HandleId, ErrorCode> {
    todo!()
}

impl fmt::Debug for Sink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Sink")
            .field(&self.handle.as_usize())
            .finish()
    }
}

impl Handleable for Sink {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
