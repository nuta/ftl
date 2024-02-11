use alloc::collections::BTreeMap;
use ftl_types::{error::FtlError, handle::HandleId};
use hashbrown::HashMap;

use crate::channel::Channel;

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
    objects: HashMap<HandleId, Object<State>>,
}

impl<State> Eventloop<State> {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
        }
    }

    pub fn add_channel(&mut self, ch: Channel, state: State) -> Result<(), FtlError> {
        self.objects.insert(
            ch.handle_id(),
            Object {
                kind: ObjectKind::Channel,
                state,
            },
        );

        todo!()
    }

    pub fn run<F>(mut self, f: F)
    where
        F: Fn(&mut Eventloop<State>, &mut State, Event<'_>),
    {
        todo!()
    }
}
