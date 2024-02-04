use alloc::collections::BTreeMap;

use crate::{handle::HandleId, result::Result};

use super::channel::Channel;

enum ObjectKind {
    Channel,
}

struct Object<State> {
    kind: ObjectKind,
    state: State,
}

pub enum Event<'a> {
    ChannelReceived { channel: &'a mut Channel },
}

pub struct Eventloop<State> {
    objects: BTreeMap<HandleId, Object<State>>,
}

impl<State> Eventloop<State> {
    pub fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
        }
    }

    pub fn add_channel(&mut self, ch: Channel) -> Result<()> {
        todo!()
    }

    pub fn run<F>(mut self, f: F)
    where
        F: Fn(&mut Eventloop<State>, &mut State, Event<'_>),
    {
        todo!()
    }
}
