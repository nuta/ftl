use core::num::NonZeroU32;
use hashbrown::HashMap;

use crate::{
    channel::{Channel, Message},
    Handle,
};

pub struct Interest(NonZeroU32);

impl Interest {
    pub const MESSAGE: Interest = Interest(unsafe { NonZeroU32::new_unchecked(1 << 0) });
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    Message(Message),
}

pub struct EventQueue<C> {
    contexts: HashMap<Handle, C>,
}

impl<C> EventQueue<C> {
    pub fn new() -> EventQueue<C> {
        todo!()
    }

    pub fn register_channel(
        &mut self,
        channel: &Channel,
        interest: Interest,
        ctx: C,
    ) -> crate::Result<()> {
        todo!();
        Ok(())
    }

    pub fn next(&mut self) -> Option<(&mut C, Event)> {
        todo!()
    }

    pub fn iter(&mut self) -> Iter<'_, C> {
        todo!()
    }
}
