use core::fmt;

use ftl_types::channel::MessageInfo;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Message;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;

pub enum Event {
    Message {
        msginfo: MessageInfo,
        handle: HandleId,
        arg0: usize,
        arg1: usize,
    },
    Test,
}

pub struct Sink<C> {
    handle: OwnedHandle,
    contexts: HashMap<HandleId, C>,
}

impl<C> Sink<C> {
    pub fn add<H: Handleable>(&mut self, handle: H, ctx: C) -> Result<(), ErrorCode> {
        self.contexts.insert(handle.handle().id(), ctx);
        Ok(())
    }

    pub fn pop(&mut self) -> Result<(&mut C, Event), ErrorCode> {
        let handle_id = sys_sink_pop(self.handle.id())?;
        let ctx = self.contexts.get_mut(&handle_id).unwrap();
        Ok((ctx, Event::Test))
    }
}

fn sys_sink_pop(handle: HandleId) -> Result<HandleId, ErrorCode> {
    todo!()
}

impl<C> fmt::Debug for Sink<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Sink")
            .field(&self.handle.as_usize())
            .finish()
    }
}

impl<C> Handleable for Sink<C> {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
